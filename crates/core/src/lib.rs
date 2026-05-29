//! Core image model and pixel-format conversions for the `imx` image toolkit.
//!
//! `imx-core` provides the format-agnostic [`Image`] type plus the
//! deterministic, byte-identical pixel conversions that the `imx` CLI and the
//! per-format codec crates build on. Every conversion is differentially
//! verified against the real ImageMagick binary as an oracle, and all buffer
//! allocations are bounded by [`MAX_PIXEL_BYTES`] and performed through checked
//! reservation so malformed or hostile inputs cannot trigger uncontrolled
//! allocation.
//!
//! This crate is codec-free: it operates on already-decoded pixel buffers and
//! never reads or writes files. Decoding and encoding of concrete container
//! formats (PNG, JPEG, BMP, QOI, Netpbm, farbfeld) live in the separate `imx`
//! codec crates.
//!
//! # Example
//!
//! Construct an [`Image`] from raw RGB8 pixels and convert it to 8-bit
//! grayscale using the Rec.709 luma weights:
//!
//! ```
//! use imx_core::{Image, PixelFormat};
//!
//! // A 2x1 RGB8 image: one pure-red pixel, one pure-green pixel.
//! let rgb = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 255, 0])?;
//!
//! let gray = rgb.to_gray8()?;
//! assert_eq!(gray.pixel_format(), PixelFormat::Gray8);
//! assert_eq!(gray.width(), 2);
//! assert_eq!(gray.height(), 1);
//! // Deterministic Rec.709 luma: red -> 54, green -> 182.
//! assert_eq!(gray.pixels(), &[54, 182]);
//! # Ok::<(), imx_core::ImageError>(())
//! ```

use std::fmt;

pub const MAX_PIXEL_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Bmp,
    Farbfeld,
    Gif,
    Jpeg,
    Pbm,
    Pgm,
    Png,
    Ppm,
    Qoi,
    Tiff,
    Webp,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Self::Bmp => "BMP",
            Self::Farbfeld => "FARBFELD",
            Self::Gif => "GIF",
            Self::Jpeg => "JPEG",
            Self::Pbm => "PBM",
            Self::Pgm => "PGM",
            Self::Png => "PNG",
            Self::Ppm => "PPM",
            Self::Qoi => "QOI",
            Self::Tiff => "TIFF",
            Self::Webp => "WEBP",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bilevel,
    Gray8,
    Gray16Be,
    Rgb8,
    Rgb16Be,
    Rgba8,
    Rgba16Be,
}

impl PixelFormat {
    pub fn channels(self) -> &'static str {
        match self {
            Self::Bilevel => "GRAY",
            Self::Gray8 | Self::Gray16Be => "GRAY",
            Self::Rgb8 | Self::Rgb16Be => "RGB",
            Self::Rgba8 => "RGBA",
            Self::Rgba16Be => "RGBA",
        }
    }

    pub fn depth(self) -> u8 {
        match self {
            Self::Bilevel => 1,
            Self::Gray8 => 8,
            Self::Gray16Be => 16,
            Self::Rgb8 | Self::Rgba8 => 8,
            Self::Rgb16Be => 16,
            Self::Rgba16Be => 16,
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Bilevel => 1,
            Self::Gray8 => 1,
            Self::Gray16Be => 2,
            Self::Rgb8 => 3,
            Self::Rgb16Be => 6,
            Self::Rgba8 => 4,
            Self::Rgba16Be => 8,
        }
    }
}

/// A decoded raster image plus an optional embedded ICC color profile.
///
/// `icc_profile`, when present, is the raw bytes of the source container's
/// embedded ICC profile (PNG `iCCP`, JPEG `APP2 ICC_PROFILE`, TIFF tag 34675).
/// It describes how the stored pixel values map to color and is preserved
/// verbatim across geometry transforms (which never change the encoding), but
/// is intentionally dropped by the pixel-format conversions
/// (`to_rgba8`/`to_gray8`/...), since those re-encode the samples into a
/// different representation that the source profile no longer describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    pixels: Vec<u8>,
    icc_profile: Option<Vec<u8>>,
}

impl Image {
    pub fn new(
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        pixels: Vec<u8>,
    ) -> Result<Self, ImageError> {
        let expected = pixel_len(width, height, pixel_format.bytes_per_pixel())?;
        if pixels.len() != expected {
            return Err(ImageError::InvalidPixelBuffer {
                expected,
                actual: pixels.len(),
            });
        }

        Ok(Self {
            width,
            height,
            pixel_format,
            pixels,
            icc_profile: None,
        })
    }

    /// Attach (or clear) the embedded ICC color profile, consuming and
    /// returning `self` so it can chain after [`Image::new`].
    ///
    /// Passing `None` clears any existing profile. The bytes are stored
    /// verbatim and are never parsed or validated by `imx-core`.
    pub fn with_icc(mut self, icc: Option<Vec<u8>>) -> Self {
        self.icc_profile = icc;
        self
    }

    /// The raw embedded ICC color profile, if any.
    pub fn icc(&self) -> Option<&[u8]> {
        self.icc_profile.as_deref()
    }

    /// Rebuild a geometry-transformed image, carrying the source ICC profile
    /// forward. A geometry transform (resize/crop/rotate/flip) does not change
    /// the pixel encoding, so the embedded profile stays valid and is preserved;
    /// every geometry method routes through here instead of calling
    /// [`Image::new`] directly (which would drop the profile).
    fn rebuild(
        &self,
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        pixels: Vec<u8>,
    ) -> Result<Self, ImageError> {
        Ok(Self::new(width, height, pixel_format, pixels)?.with_icc(self.icc_profile.clone()))
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }

    pub fn identify(&self, format: Format) -> Identify {
        Identify {
            format,
            width: self.width,
            height: self.height,
            pixel_format: self.pixel_format,
        }
    }

    pub fn resize_nearest(&self, width: u32, height: u32) -> Result<Self, ImageError> {
        let bytes_per_pixel = self.pixel_format.bytes_per_pixel();
        let output_len = pixel_len(width, height, bytes_per_pixel)?;
        if width == self.width && height == self.height {
            return Ok(self.clone());
        }

        let mut out = try_vec_with_capacity(output_len)?;
        let source_width = u128::from(self.width);
        let source_height = u128::from(self.height);
        let target_width = u128::from(width);
        let target_height = u128::from(height);

        for y in 0..height {
            let source_y = (((u128::from(y) * 2 + 1) * source_height) / (target_height * 2))
                .min(source_height - 1) as usize;
            for x in 0..width {
                let source_x = (((u128::from(x) * 2 + 1) * source_width) / (target_width * 2))
                    .min(source_width - 1) as usize;
                let source_offset = (source_y * self.width as usize + source_x) * bytes_per_pixel;
                out.extend_from_slice(&self.pixels[source_offset..source_offset + bytes_per_pixel]);
            }
        }

        self.rebuild(width, height, self.pixel_format, out)
    }

    pub fn resize_nearest_fit(&self, width: u32, height: u32) -> Result<Self, ImageError> {
        let (width, height) = fit_dimensions(self.width, self.height, width, height)?;
        self.resize_nearest(width, height)
    }

    /// Resize to exact `width` x `height` using the given resampling `filter`.
    ///
    /// [`ResizeFilter::Point`] is byte-for-byte identical to
    /// [`Image::resize_nearest`]. Every other filter performs a separable
    /// two-pass weighted resample (horizontal then vertical) in a normalized
    /// floating-point working space, rounding half-up back to the source bit
    /// depth. The source pixel format and channel count are preserved, output
    /// is byte-deterministic, and all allocation is bounded by
    /// [`MAX_PIXEL_BYTES`] so this never panics on hostile dimensions.
    pub fn resize_filtered(
        &self,
        width: u32,
        height: u32,
        filter: ResizeFilter,
    ) -> Result<Self, ImageError> {
        if filter == ResizeFilter::Point {
            return self.resize_nearest(width, height);
        }
        // Validate and bound the output buffer up front.
        let _ = pixel_len(width, height, self.pixel_format.bytes_per_pixel())?;
        if width == self.width && height == self.height {
            return Ok(self.clone());
        }

        match self.pixel_format {
            PixelFormat::Bilevel => self.resize_filtered_bilevel(width, height, filter),
            PixelFormat::Gray8 | PixelFormat::Rgb8 | PixelFormat::Rgba8 => {
                self.resize_filtered_u8(width, height, filter)
            }
            PixelFormat::Gray16Be | PixelFormat::Rgb16Be | PixelFormat::Rgba16Be => {
                self.resize_filtered_u16be(width, height, filter)
            }
        }
    }

    /// Aspect-preserving filtered resize: fit within `width` x `height`, then
    /// resample with `filter`. Mirrors [`Image::resize_nearest_fit`].
    pub fn resize_filtered_fit(
        &self,
        width: u32,
        height: u32,
        filter: ResizeFilter,
    ) -> Result<Self, ImageError> {
        let (width, height) = fit_dimensions(self.width, self.height, width, height)?;
        self.resize_filtered(width, height, filter)
    }

    fn resize_filtered_u8(
        &self,
        width: u32,
        height: u32,
        filter: ResizeFilter,
    ) -> Result<Self, ImageError> {
        let channels = self.pixel_format.bytes_per_pixel();
        let samples = self
            .pixels
            .iter()
            .map(|&value| f64::from(value))
            .collect::<Vec<_>>();
        let resampled = resample_planes(
            &samples,
            self.width as usize,
            self.height as usize,
            width as usize,
            height as usize,
            channels,
            filter,
        )?;
        let mut out = try_vec_with_capacity(pixel_len(width, height, channels)?)?;
        for value in resampled {
            out.push(round_clamp_u8(value));
        }
        Self::new(width, height, self.pixel_format, out)
    }

    fn resize_filtered_u16be(
        &self,
        width: u32,
        height: u32,
        filter: ResizeFilter,
    ) -> Result<Self, ImageError> {
        let channels = self.pixel_format.bytes_per_pixel() / 2;
        let samples = self
            .pixels
            .chunks_exact(2)
            .map(|chunk| f64::from(u16::from_be_bytes([chunk[0], chunk[1]])))
            .collect::<Vec<_>>();
        let resampled = resample_planes(
            &samples,
            self.width as usize,
            self.height as usize,
            width as usize,
            height as usize,
            channels,
            filter,
        )?;
        let mut out = try_vec_with_capacity(pixel_len(width, height, channels * 2)?)?;
        for value in resampled {
            out.extend_from_slice(&round_clamp_u16(value).to_be_bytes());
        }
        Self::new(width, height, self.pixel_format, out)
    }

    fn resize_filtered_bilevel(
        &self,
        width: u32,
        height: u32,
        filter: ResizeFilter,
    ) -> Result<Self, ImageError> {
        // Bilevel pixels are stored as one byte per pixel valued 0 or 255.
        // Resample in that 0..=255 space, then re-threshold so the output is a
        // valid bilevel buffer.
        let samples = self
            .pixels
            .iter()
            .map(|&value| f64::from(value))
            .collect::<Vec<_>>();
        let resampled = resample_planes(
            &samples,
            self.width as usize,
            self.height as usize,
            width as usize,
            height as usize,
            1,
            filter,
        )?;
        let mut out = try_vec_with_capacity(pixel_len(width, height, 1)?)?;
        for value in resampled {
            out.push(threshold_u8(round_clamp_u8(value)));
        }
        Self::new(width, height, PixelFormat::Bilevel, out)
    }

    pub fn crop(&self, x: u32, y: u32, width: u32, height: u32) -> Result<Self, ImageError> {
        if width == 0 || height == 0 {
            return Err(ImageError::CropOutOfBounds {
                x,
                y,
                width,
                height,
                source_width: self.width,
                source_height: self.height,
            });
        }
        let right = u64::from(x)
            .checked_add(u64::from(width))
            .ok_or(ImageError::LengthOverflow)?;
        let bottom = u64::from(y)
            .checked_add(u64::from(height))
            .ok_or(ImageError::LengthOverflow)?;
        if right > u64::from(self.width) || bottom > u64::from(self.height) {
            return Err(ImageError::CropOutOfBounds {
                x,
                y,
                width,
                height,
                source_width: self.width,
                source_height: self.height,
            });
        }

        let bytes_per_pixel = self.pixel_format.bytes_per_pixel();
        let output_len = pixel_len(width, height, bytes_per_pixel)?;
        let mut out = try_vec_with_capacity(output_len)?;
        let source_stride = self.width as usize * bytes_per_pixel;
        let row_bytes = width as usize * bytes_per_pixel;
        let x_offset = x as usize * bytes_per_pixel;
        for row in 0..height {
            let row_start = (y as usize + row as usize) * source_stride + x_offset;
            out.extend_from_slice(&self.pixels[row_start..row_start + row_bytes]);
        }

        self.rebuild(width, height, self.pixel_format, out)
    }

    pub fn rotate_90(&self) -> Result<Self, ImageError> {
        let bytes_per_pixel = self.pixel_format.bytes_per_pixel();
        let output_len = pixel_len(self.height, self.width, bytes_per_pixel)?;
        let mut out = try_vec_with_capacity(output_len)?;
        let source_stride = self.width as usize * bytes_per_pixel;
        for x in 0..self.width {
            for y in (0..self.height).rev() {
                let source_offset = y as usize * source_stride + x as usize * bytes_per_pixel;
                out.extend_from_slice(&self.pixels[source_offset..source_offset + bytes_per_pixel]);
            }
        }
        self.rebuild(self.height, self.width, self.pixel_format, out)
    }

    pub fn rotate_180(&self) -> Result<Self, ImageError> {
        let bytes_per_pixel = self.pixel_format.bytes_per_pixel();
        let output_len = pixel_len(self.width, self.height, bytes_per_pixel)?;
        let mut out = try_vec_with_capacity(output_len)?;
        for pixel in self.pixels.chunks_exact(bytes_per_pixel).rev() {
            out.extend_from_slice(pixel);
        }
        self.rebuild(self.width, self.height, self.pixel_format, out)
    }

    pub fn rotate_270(&self) -> Result<Self, ImageError> {
        let bytes_per_pixel = self.pixel_format.bytes_per_pixel();
        let output_len = pixel_len(self.height, self.width, bytes_per_pixel)?;
        let mut out = try_vec_with_capacity(output_len)?;
        let source_stride = self.width as usize * bytes_per_pixel;
        for x in (0..self.width).rev() {
            for y in 0..self.height {
                let source_offset = y as usize * source_stride + x as usize * bytes_per_pixel;
                out.extend_from_slice(&self.pixels[source_offset..source_offset + bytes_per_pixel]);
            }
        }
        self.rebuild(self.height, self.width, self.pixel_format, out)
    }

    pub fn flip_vertical(&self) -> Result<Self, ImageError> {
        let bytes_per_pixel = self.pixel_format.bytes_per_pixel();
        let output_len = pixel_len(self.width, self.height, bytes_per_pixel)?;
        let mut out = try_vec_with_capacity(output_len)?;
        let source_stride = self.width as usize * bytes_per_pixel;
        for y in (0..self.height).rev() {
            let row_start = y as usize * source_stride;
            out.extend_from_slice(&self.pixels[row_start..row_start + source_stride]);
        }
        self.rebuild(self.width, self.height, self.pixel_format, out)
    }

    pub fn flop_horizontal(&self) -> Result<Self, ImageError> {
        let bytes_per_pixel = self.pixel_format.bytes_per_pixel();
        let output_len = pixel_len(self.width, self.height, bytes_per_pixel)?;
        let mut out = try_vec_with_capacity(output_len)?;
        let source_stride = self.width as usize * bytes_per_pixel;
        for y in 0..self.height {
            let row_start = y as usize * source_stride;
            let row = &self.pixels[row_start..row_start + source_stride];
            for pixel in row.chunks_exact(bytes_per_pixel).rev() {
                out.extend_from_slice(pixel);
            }
        }
        self.rebuild(self.width, self.height, self.pixel_format, out)
    }

    pub fn to_rgba16be(&self) -> Result<Self, ImageError> {
        match self.pixel_format {
            PixelFormat::Rgba16Be => Ok(self.clone()),
            PixelFormat::Bilevel => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 8)?)?;
                for value in &self.pixels {
                    for channel in [*value, *value, *value, 0xff] {
                        out.push(channel);
                        out.push(channel);
                    }
                }
                Self::new(self.width, self.height, PixelFormat::Rgba16Be, out)
            }
            PixelFormat::Gray8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 8)?)?;
                for gray in &self.pixels {
                    for channel in [*gray, *gray, *gray, 0xff] {
                        out.push(channel);
                        out.push(channel);
                    }
                }
                Self::new(self.width, self.height, PixelFormat::Rgba16Be, out)
            }
            PixelFormat::Gray16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 8)?)?;
                for gray in self.pixels.chunks_exact(2) {
                    out.extend_from_slice(gray);
                    out.extend_from_slice(gray);
                    out.extend_from_slice(gray);
                    out.extend_from_slice(&[0xff, 0xff]);
                }
                Self::new(self.width, self.height, PixelFormat::Rgba16Be, out)
            }
            PixelFormat::Rgb16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 8)?)?;
                for px in self.pixels.chunks_exact(6) {
                    out.extend_from_slice(&px[..6]);
                    out.extend_from_slice(&[0xff, 0xff]);
                }
                Self::new(self.width, self.height, PixelFormat::Rgba16Be, out)
            }
            PixelFormat::Rgb8 | PixelFormat::Rgba8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 8)?)?;
                for px in self
                    .pixels
                    .chunks_exact(self.pixel_format.bytes_per_pixel())
                {
                    let red = px[0];
                    let green = px[1];
                    let blue = px[2];
                    let alpha = if self.pixel_format == PixelFormat::Rgba8 {
                        px[3]
                    } else {
                        0xff
                    };
                    for channel in [red, green, blue, alpha] {
                        out.push(channel);
                        out.push(channel);
                    }
                }
                Self::new(self.width, self.height, PixelFormat::Rgba16Be, out)
            }
        }
    }

    pub fn to_rgba8(&self) -> Result<Self, ImageError> {
        match self.pixel_format {
            PixelFormat::Rgba8 => Ok(self.clone()),
            PixelFormat::Bilevel => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 4)?)?;
                for value in &self.pixels {
                    out.extend_from_slice(&[*value, *value, *value, 0xff]);
                }
                Self::new(self.width, self.height, PixelFormat::Rgba8, out)
            }
            PixelFormat::Gray8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 4)?)?;
                for gray in &self.pixels {
                    out.extend_from_slice(&[*gray, *gray, *gray, 0xff]);
                }
                Self::new(self.width, self.height, PixelFormat::Rgba8, out)
            }
            PixelFormat::Gray16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 4)?)?;
                for gray in self.pixels.chunks_exact(2) {
                    let value = scale_u16be_to_u8(gray[0], gray[1]);
                    out.extend_from_slice(&[value, value, value, 0xff]);
                }
                Self::new(self.width, self.height, PixelFormat::Rgba8, out)
            }
            PixelFormat::Rgb8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 4)?)?;
                for px in self.pixels.chunks_exact(3) {
                    out.extend_from_slice(px);
                    out.push(0xff);
                }
                Self::new(self.width, self.height, PixelFormat::Rgba8, out)
            }
            PixelFormat::Rgb16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 4)?)?;
                for px in self.pixels.chunks_exact(6) {
                    out.push(scale_u16be_to_u8(px[0], px[1]));
                    out.push(scale_u16be_to_u8(px[2], px[3]));
                    out.push(scale_u16be_to_u8(px[4], px[5]));
                    out.push(0xff);
                }
                Self::new(self.width, self.height, PixelFormat::Rgba8, out)
            }
            PixelFormat::Rgba16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 4)?)?;
                for px in self.pixels.chunks_exact(8) {
                    out.push(scale_u16be_to_u8(px[0], px[1]));
                    out.push(scale_u16be_to_u8(px[2], px[3]));
                    out.push(scale_u16be_to_u8(px[4], px[5]));
                    out.push(scale_u16be_to_u8(px[6], px[7]));
                }
                Self::new(self.width, self.height, PixelFormat::Rgba8, out)
            }
        }
    }

    pub fn to_rgb8(&self) -> Result<Self, ImageError> {
        match self.pixel_format {
            PixelFormat::Rgb8 => Ok(self.clone()),
            PixelFormat::Bilevel => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 3)?)?;
                for value in &self.pixels {
                    out.extend_from_slice(&[*value, *value, *value]);
                }
                Self::new(self.width, self.height, PixelFormat::Rgb8, out)
            }
            PixelFormat::Gray8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 3)?)?;
                for gray in &self.pixels {
                    out.extend_from_slice(&[*gray, *gray, *gray]);
                }
                Self::new(self.width, self.height, PixelFormat::Rgb8, out)
            }
            PixelFormat::Gray16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 3)?)?;
                for gray in self.pixels.chunks_exact(2) {
                    let value = scale_u16be_to_u8(gray[0], gray[1]);
                    out.extend_from_slice(&[value, value, value]);
                }
                Self::new(self.width, self.height, PixelFormat::Rgb8, out)
            }
            PixelFormat::Rgba8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 3)?)?;
                for px in self.pixels.chunks_exact(4) {
                    out.extend_from_slice(&px[..3]);
                }
                Self::new(self.width, self.height, PixelFormat::Rgb8, out)
            }
            PixelFormat::Rgb16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 3)?)?;
                for px in self.pixels.chunks_exact(6) {
                    out.push(scale_u16be_to_u8(px[0], px[1]));
                    out.push(scale_u16be_to_u8(px[2], px[3]));
                    out.push(scale_u16be_to_u8(px[4], px[5]));
                }
                Self::new(self.width, self.height, PixelFormat::Rgb8, out)
            }
            PixelFormat::Rgba16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 3)?)?;
                for px in self.pixels.chunks_exact(8) {
                    out.push(scale_u16be_to_u8(px[0], px[1]));
                    out.push(scale_u16be_to_u8(px[2], px[3]));
                    out.push(scale_u16be_to_u8(px[4], px[5]));
                }
                Self::new(self.width, self.height, PixelFormat::Rgb8, out)
            }
        }
    }

    pub fn to_rgb16be(&self) -> Result<Self, ImageError> {
        match self.pixel_format {
            PixelFormat::Rgb16Be => Ok(self.clone()),
            PixelFormat::Bilevel => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 6)?)?;
                for value in &self.pixels {
                    for channel in [*value, *value, *value] {
                        out.push(channel);
                        out.push(channel);
                    }
                }
                Self::new(self.width, self.height, PixelFormat::Rgb16Be, out)
            }
            PixelFormat::Gray8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 6)?)?;
                for gray in &self.pixels {
                    for channel in [*gray, *gray, *gray] {
                        out.push(channel);
                        out.push(channel);
                    }
                }
                Self::new(self.width, self.height, PixelFormat::Rgb16Be, out)
            }
            PixelFormat::Gray16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 6)?)?;
                for gray in self.pixels.chunks_exact(2) {
                    out.extend_from_slice(gray);
                    out.extend_from_slice(gray);
                    out.extend_from_slice(gray);
                }
                Self::new(self.width, self.height, PixelFormat::Rgb16Be, out)
            }
            PixelFormat::Rgb8 | PixelFormat::Rgba8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 6)?)?;
                for px in self
                    .pixels
                    .chunks_exact(self.pixel_format.bytes_per_pixel())
                {
                    for channel in [px[0], px[1], px[2]] {
                        out.push(channel);
                        out.push(channel);
                    }
                }
                Self::new(self.width, self.height, PixelFormat::Rgb16Be, out)
            }
            PixelFormat::Rgba16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 6)?)?;
                for px in self.pixels.chunks_exact(8) {
                    out.extend_from_slice(&px[..6]);
                }
                Self::new(self.width, self.height, PixelFormat::Rgb16Be, out)
            }
        }
    }

    pub fn to_gray8(&self) -> Result<Self, ImageError> {
        match self.pixel_format {
            PixelFormat::Bilevel => Self::new(
                self.width,
                self.height,
                PixelFormat::Gray8,
                self.pixels.clone(),
            ),
            PixelFormat::Gray8 => Ok(self.clone()),
            PixelFormat::Gray16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 1)?)?;
                for gray in self.pixels.chunks_exact(2) {
                    out.push(scale_u16be_to_u8(gray[0], gray[1]));
                }
                Self::new(self.width, self.height, PixelFormat::Gray8, out)
            }
            PixelFormat::Rgb8 | PixelFormat::Rgba8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 1)?)?;
                for px in self
                    .pixels
                    .chunks_exact(self.pixel_format.bytes_per_pixel())
                {
                    out.push(rec709_luma8(px[0], px[1], px[2]));
                }
                Self::new(self.width, self.height, PixelFormat::Gray8, out)
            }
            PixelFormat::Rgb16Be | PixelFormat::Rgba16Be => {
                let gray16 = self.to_gray16be()?;
                gray16.to_gray8()
            }
        }
    }

    pub fn to_gray16be(&self) -> Result<Self, ImageError> {
        match self.pixel_format {
            PixelFormat::Bilevel => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 2)?)?;
                for value in &self.pixels {
                    out.extend_from_slice(&[*value, *value]);
                }
                Self::new(self.width, self.height, PixelFormat::Gray16Be, out)
            }
            PixelFormat::Gray16Be => Ok(self.clone()),
            PixelFormat::Gray8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 2)?)?;
                for gray in &self.pixels {
                    out.extend_from_slice(&[*gray, *gray]);
                }
                Self::new(self.width, self.height, PixelFormat::Gray16Be, out)
            }
            PixelFormat::Rgb8 | PixelFormat::Rgba8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 2)?)?;
                for px in self
                    .pixels
                    .chunks_exact(self.pixel_format.bytes_per_pixel())
                {
                    let gray = rec709_luma8(px[0], px[1], px[2]);
                    out.extend_from_slice(&[gray, gray]);
                }
                Self::new(self.width, self.height, PixelFormat::Gray16Be, out)
            }
            PixelFormat::Rgb16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 2)?)?;
                for px in self.pixels.chunks_exact(6) {
                    let gray = rec709_luma16be([px[0], px[1]], [px[2], px[3]], [px[4], px[5]]);
                    out.extend_from_slice(&gray.to_be_bytes());
                }
                Self::new(self.width, self.height, PixelFormat::Gray16Be, out)
            }
            PixelFormat::Rgba16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 2)?)?;
                for px in self.pixels.chunks_exact(8) {
                    let gray = rec709_luma16be([px[0], px[1]], [px[2], px[3]], [px[4], px[5]]);
                    out.extend_from_slice(&gray.to_be_bytes());
                }
                Self::new(self.width, self.height, PixelFormat::Gray16Be, out)
            }
        }
    }

    pub fn to_bilevel(&self) -> Result<Self, ImageError> {
        match self.pixel_format {
            PixelFormat::Bilevel => Ok(self.clone()),
            PixelFormat::Gray8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 1)?)?;
                for gray in &self.pixels {
                    out.push(threshold_u8(*gray));
                }
                Self::new(self.width, self.height, PixelFormat::Bilevel, out)
            }
            PixelFormat::Gray16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 1)?)?;
                for gray in self.pixels.chunks_exact(2) {
                    let value = u16::from_be_bytes([gray[0], gray[1]]);
                    out.push(if value < 32768 { 0 } else { 255 });
                }
                Self::new(self.width, self.height, PixelFormat::Bilevel, out)
            }
            PixelFormat::Rgb8 | PixelFormat::Rgba8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 1)?)?;
                for px in self
                    .pixels
                    .chunks_exact(self.pixel_format.bytes_per_pixel())
                {
                    out.push(threshold_u8(rec709_luma8(px[0], px[1], px[2])));
                }
                Self::new(self.width, self.height, PixelFormat::Bilevel, out)
            }
            PixelFormat::Rgb16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 1)?)?;
                for px in self.pixels.chunks_exact(6) {
                    let gray = rec709_luma16be([px[0], px[1]], [px[2], px[3]], [px[4], px[5]]);
                    out.push(if gray < 32768 { 0 } else { 255 });
                }
                Self::new(self.width, self.height, PixelFormat::Bilevel, out)
            }
            PixelFormat::Rgba16Be => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 1)?)?;
                for px in self.pixels.chunks_exact(8) {
                    let gray = rec709_luma16be([px[0], px[1]], [px[2], px[3]], [px[4], px[5]]);
                    out.push(if gray < 32768 { 0 } else { 255 });
                }
                Self::new(self.width, self.height, PixelFormat::Bilevel, out)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identify {
    pub format: Format,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
}

impl Identify {
    pub fn stable_line(self) -> String {
        format!(
            "format={} width={} height={} channels={} depth={}",
            self.format.name(),
            self.width,
            self.height,
            self.pixel_format.channels(),
            self.pixel_format.depth()
        )
    }
}

/// Apply an EXIF Orientation transform so that the returned [`Image`] is
/// displayed upright.
///
/// `orientation` is the raw EXIF Orientation tag value (1..=8) as defined by
/// the TIFF/EXIF specification:
///
/// | Value | Transform                          |
/// |-------|------------------------------------|
/// | 1     | identity (no-op)                   |
/// | 2     | mirror horizontal (flop)           |
/// | 3     | rotate 180                         |
/// | 4     | mirror vertical (flip)             |
/// | 5     | transpose (rotate 90 CW + flop)    |
/// | 6     | rotate 90 CW                       |
/// | 7     | transverse (rotate 90 CW + flip)   |
/// | 8     | rotate 270 CW                      |
///
/// Values 5..=8 swap the image's width and height. Any value outside 1..=8 is
/// treated as `1` (identity), so callers may forward unknown or missing tags
/// without special-casing them. The transform is implemented entirely in terms
/// of the existing [`Image`] rotate/flip helpers, so it is bounded by
/// [`MAX_PIXEL_BYTES`] and never panics.
pub fn apply_exif_orientation(image: Image, orientation: u16) -> Result<Image, ImageError> {
    match orientation {
        2 => image.flop_horizontal(),
        3 => image.rotate_180(),
        4 => image.flip_vertical(),
        5 => image.rotate_90()?.flop_horizontal(),
        6 => image.rotate_90(),
        7 => image.rotate_90()?.flip_vertical(),
        8 => image.rotate_270(),
        // 1 and any out-of-range value are treated as identity.
        _ => Ok(image),
    }
}

/// Return the displayed `(width, height)` after applying an EXIF Orientation
/// transform, without touching pixels.
///
/// Orientation values 5..=8 swap the two axes; every other value (including
/// out-of-range values, which are treated as identity) returns the dimensions
/// unchanged. This mirrors [`apply_exif_orientation`] and lets `identify` report
/// upright dimensions without decoding the full pixel buffer.
pub fn exif_oriented_dimensions(orientation: u16, width: u32, height: u32) -> (u32, u32) {
    match orientation {
        5..=8 => (height, width),
        _ => (width, height),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    InvalidHeader(&'static str),
    InvalidDimensions,
    LengthOverflow,
    ImageTooLarge {
        required: usize,
        limit: usize,
    },
    UnexpectedEof {
        expected: usize,
        actual: usize,
    },
    InvalidPixelBuffer {
        expected: usize,
        actual: usize,
    },
    AllocationFailed {
        requested: usize,
    },
    InvalidChannels {
        channels: u8,
    },
    InvalidColorspace {
        colorspace: u8,
    },
    InvalidMaxValue {
        format: &'static str,
        max_value: u32,
        max_supported: u32,
    },
    InvalidSampleValue {
        format: &'static str,
        sample_value: u32,
        max_value: u32,
    },
    InvalidPbmSample {
        byte: u8,
    },
    CropOutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        source_width: u32,
        source_height: u32,
    },
    FrameIndexOutOfRange {
        index: u32,
        frame_count: u32,
    },
    UnsupportedFormat(String),
}

impl ImageError {
    pub fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::InvalidHeader(_) => "image.invalid_header",
            Self::InvalidDimensions => "image.invalid_dimensions",
            Self::LengthOverflow => "image.length_overflow",
            Self::ImageTooLarge { .. } => "image.too_large",
            Self::UnexpectedEof { .. } => "image.unexpected_eof",
            Self::InvalidPixelBuffer { .. } => "image.invalid_pixel_buffer",
            Self::AllocationFailed { .. } => "image.allocation_failed",
            Self::InvalidChannels { .. } => "qoi.invalid_channels",
            Self::InvalidColorspace { .. } => "qoi.invalid_colorspace",
            Self::InvalidMaxValue { .. } => "pnm.invalid_max_value",
            Self::InvalidSampleValue { .. } => "pnm.invalid_sample_value",
            Self::InvalidPbmSample { .. } => "pbm.invalid_sample",
            Self::CropOutOfBounds { .. } => "image.crop_out_of_bounds",
            Self::FrameIndexOutOfRange { .. } => "image.frame_index_out_of_range",
            Self::UnsupportedFormat(_) => "image.unsupported_format",
        }
    }
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader(format) => write!(f, "invalid {format} header"),
            Self::InvalidDimensions => write!(f, "image dimensions must be non-zero"),
            Self::LengthOverflow => write!(f, "image pixel byte length overflow"),
            Self::ImageTooLarge { required, limit } => {
                write!(
                    f,
                    "image pixel buffer too large: requires {required} bytes, limit is {limit}"
                )
            }
            Self::UnexpectedEof { expected, actual } => {
                write!(
                    f,
                    "unexpected end of file: expected {expected} bytes, got {actual}"
                )
            }
            Self::InvalidPixelBuffer { expected, actual } => {
                write!(
                    f,
                    "invalid pixel buffer: expected {expected} bytes, got {actual}"
                )
            }
            Self::AllocationFailed { requested } => {
                write!(f, "failed to reserve {requested} bytes for image buffer")
            }
            Self::InvalidChannels { channels } => {
                write!(f, "QOI channels must be 3 or 4, got {channels}")
            }
            Self::InvalidColorspace { colorspace } => {
                write!(f, "QOI colorspace must be 0 or 1, got {colorspace}")
            }
            Self::InvalidMaxValue {
                format,
                max_value,
                max_supported,
            } => {
                write!(
                    f,
                    "{format} max value must be 1..={max_supported}, got {max_value}"
                )
            }
            Self::InvalidSampleValue {
                format,
                sample_value,
                max_value,
            } => {
                write!(
                    f,
                    "{format} sample value must be <= {max_value}, got {sample_value}"
                )
            }
            Self::InvalidPbmSample { byte } => {
                write!(f, "PBM samples must be ASCII 0 or 1, got 0x{byte:02x}")
            }
            Self::CropOutOfBounds {
                x,
                y,
                width,
                height,
                source_width,
                source_height,
            } => {
                write!(
                    f,
                    "crop region {width}x{height}+{x}+{y} exceeds {source_width}x{source_height} source bounds"
                )
            }
            Self::FrameIndexOutOfRange { index, frame_count } => {
                write!(
                    f,
                    "frame index {index} out of range: image has {frame_count} frame(s)"
                )
            }
            Self::UnsupportedFormat(format) => write!(f, "unsupported format: {format}"),
        }
    }
}

impl std::error::Error for ImageError {}

pub fn pixel_count(width: u32, height: u32) -> Result<usize, ImageError> {
    if width == 0 || height == 0 {
        return Err(ImageError::InvalidDimensions);
    }
    let pixels = (width as u64)
        .checked_mul(height as u64)
        .ok_or(ImageError::LengthOverflow)?;
    usize::try_from(pixels).map_err(|_| ImageError::LengthOverflow)
}

pub fn pixel_len(width: u32, height: u32, bytes_per_pixel: usize) -> Result<usize, ImageError> {
    let bytes = pixel_count(width, height)?
        .checked_mul(bytes_per_pixel)
        .ok_or(ImageError::LengthOverflow)?;
    if bytes > MAX_PIXEL_BYTES {
        return Err(ImageError::ImageTooLarge {
            required: bytes,
            limit: MAX_PIXEL_BYTES,
        });
    }
    Ok(bytes)
}

pub fn fit_dimensions(
    source_width: u32,
    source_height: u32,
    max_width: u32,
    max_height: u32,
) -> Result<(u32, u32), ImageError> {
    if source_width == 0 || source_height == 0 || max_width == 0 || max_height == 0 {
        return Err(ImageError::InvalidDimensions);
    }

    let source_width = u128::from(source_width);
    let source_height = u128::from(source_height);
    let max_width = u128::from(max_width);
    let max_height = u128::from(max_height);

    if max_width * source_height <= max_height * source_width {
        let height = round_scaled_dimension(source_height * max_width, source_width)
            .max(1)
            .min(max_height as u32);
        Ok((max_width as u32, height))
    } else {
        let width = round_scaled_dimension(source_width * max_height, source_height)
            .max(1)
            .min(max_width as u32);
        Ok((width, max_height as u32))
    }
}

fn round_scaled_dimension(numerator: u128, denominator: u128) -> u32 {
    ((numerator * 2 + denominator) / (denominator * 2)) as u32
}

/// Pixel-level difference statistics between two images.
///
/// Produced by [`compare_rgba8`] after normalizing both operands to the
/// `Rgba8` pixel format. All fields are computed over the four RGBA channels of
/// every pixel, so a single-channel-only difference still counts the pixel as
/// differing and contributes to the mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comparison {
    /// Number of pixels whose RGBA bytes are not all identical.
    pub differing_pixels: u64,
    /// Total number of pixels compared (`width * height`).
    pub total_pixels: u64,
    /// Largest absolute per-channel difference (the "AE"-style peak), 0..=255.
    pub max_abs_diff: u8,
    /// Sum of absolute per-channel differences across every channel of every
    /// pixel. Divide by `4 * total_pixels` to obtain the mean absolute error.
    pub sum_abs_diff: u64,
    /// Sum of squared per-channel differences across every channel of every
    /// pixel. Divide by `4 * total_pixels` to obtain the mean squared error
    /// used by [`Comparison::psnr`].
    pub sum_squared_diff: u64,
}

impl Comparison {
    /// Returns `true` when the two normalized images are byte-identical.
    pub fn is_identical(&self) -> bool {
        self.differing_pixels == 0
    }

    /// Mean absolute error across all RGBA channels of all pixels, in the
    /// 0.0..=255.0 range. Returns 0.0 when there are no pixels.
    pub fn mae(&self) -> f64 {
        let channels = self.total_pixels.saturating_mul(4);
        if channels == 0 {
            return 0.0;
        }
        self.sum_abs_diff as f64 / channels as f64
    }

    /// Mean squared error across all RGBA channels of all pixels.
    fn mse(&self) -> f64 {
        let channels = self.total_pixels.saturating_mul(4);
        if channels == 0 {
            return 0.0;
        }
        self.sum_squared_diff as f64 / channels as f64
    }

    /// Peak signal-to-noise ratio in decibels over 8-bit channels.
    ///
    /// Returns `f64::INFINITY` when the images are identical (zero error),
    /// matching the conventional "infinite PSNR" reported by ImageMagick and
    /// other tools for an exact match.
    pub fn psnr(&self) -> f64 {
        let mse = self.mse();
        if mse == 0.0 {
            return f64::INFINITY;
        }
        let max = 255.0_f64;
        10.0 * (max * max / mse).log10()
    }
}

/// Compute pixel-level difference statistics between two images.
///
/// Both images are normalized to the `Rgba8` representation via
/// [`Image::to_rgba8`] before comparison, so differences in source pixel
/// format (e.g. RGB vs RGBA, 8-bit vs 16-bit) are resolved through that common
/// representation. The caller is responsible for ensuring the two images share
/// the same dimensions; a dimension mismatch returns
/// [`ImageError::InvalidDimensions`] rather than attempting a diff.
pub fn compare_rgba8(a: &Image, b: &Image) -> Result<Comparison, ImageError> {
    if a.width() != b.width() || a.height() != b.height() {
        return Err(ImageError::InvalidDimensions);
    }
    let a = a.to_rgba8()?;
    let b = b.to_rgba8()?;
    let total_pixels = pixel_count(a.width(), a.height())? as u64;

    let mut differing_pixels: u64 = 0;
    let mut max_abs_diff: u8 = 0;
    let mut sum_abs_diff: u64 = 0;
    let mut sum_squared_diff: u64 = 0;

    for (pa, pb) in a.pixels().chunks_exact(4).zip(b.pixels().chunks_exact(4)) {
        let mut pixel_differs = false;
        for (&ca, &cb) in pa.iter().zip(pb.iter()) {
            let diff = ca.abs_diff(cb);
            if diff != 0 {
                pixel_differs = true;
            }
            if diff > max_abs_diff {
                max_abs_diff = diff;
            }
            sum_abs_diff += u64::from(diff);
            sum_squared_diff += u64::from(diff) * u64::from(diff);
        }
        if pixel_differs {
            differing_pixels += 1;
        }
    }

    Ok(Comparison {
        differing_pixels,
        total_pixels,
        max_abs_diff,
        sum_abs_diff,
        sum_squared_diff,
    })
}

/// A parsed `imx resize` geometry specification.
///
/// This mirrors the subset of ImageMagick geometry syntax that `imx resize`
/// accepts. Concrete target dimensions are resolved against source dimensions
/// by [`ResizeGeometry::resolve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeGeometry {
    /// `<width>x<height>` — both axes given as exact pixel counts.
    Exact { width: u32, height: u32 },
    /// `<width>x` — width given, height derived from the source aspect ratio.
    Width(u32),
    /// `x<height>` — height given, width derived from the source aspect ratio.
    Height(u32),
    /// `<percent>%` — scale both axes uniformly by an integer percentage.
    Percent(u32),
}

/// Resampling filter used by [`Image::resize_filtered`].
///
/// `Point` reproduces the exact center-sampled nearest-neighbor output of
/// [`Image::resize_nearest`] byte-for-byte. The remaining filters perform a
/// separable two-pass (horizontal then vertical) weighted resample using the
/// named reconstruction kernel:
///
/// | Filter        | Kernel                    | Support |
/// |---------------|---------------------------|---------|
/// | `Point`       | nearest neighbor          | n/a     |
/// | `Box`         | box / averaging           | 0.5     |
/// | `Triangle`    | linear (bilinear)         | 1.0     |
/// | `CatmullRom`  | Catmull-Rom bicubic       | 2.0     |
/// | `Lanczos3`    | windowed sinc (a = 3)     | 3.0     |
///
/// All filtered paths operate in a normalized floating-point working space and
/// round half-up back to the source bit depth, so output is fully
/// byte-deterministic across platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeFilter {
    Point,
    Box,
    Triangle,
    CatmullRom,
    Lanczos3,
}

impl ResizeFilter {
    /// The radius (in source pixels, before scaling) beyond which the kernel is
    /// zero. `Point` has no meaningful support and returns `0.0`.
    fn support(self) -> f64 {
        match self {
            Self::Point => 0.0,
            Self::Box => 0.5,
            Self::Triangle => 1.0,
            Self::CatmullRom => 2.0,
            Self::Lanczos3 => 3.0,
        }
    }

    /// Evaluate the reconstruction kernel at distance `x` (in source pixels).
    fn weight(self, x: f64) -> f64 {
        let x = x.abs();
        match self {
            // Point is handled separately and never sampled as a kernel.
            Self::Point => {
                if x < 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Box => {
                if x <= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Triangle => {
                if x < 1.0 {
                    1.0 - x
                } else {
                    0.0
                }
            }
            Self::CatmullRom => {
                // Catmull-Rom cubic (B = 0, C = 0.5).
                if x < 1.0 {
                    1.5 * x * x * x - 2.5 * x * x + 1.0
                } else if x < 2.0 {
                    -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0
                } else {
                    0.0
                }
            }
            Self::Lanczos3 => {
                if x < 1e-9 {
                    1.0
                } else if x < 3.0 {
                    let px = std::f64::consts::PI * x;
                    let px3 = px / 3.0;
                    (px.sin() / px) * (px3.sin() / px3)
                } else {
                    0.0
                }
            }
        }
    }
}

/// Error returned when a resize geometry string cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryError {
    geometry: String,
}

impl GeometryError {
    pub fn diagnostic_code(&self) -> &'static str {
        "image.invalid_geometry"
    }
}

impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid resize geometry: {}; expected <width>x<height>, <width>x, x<height>, or <percent>%",
            self.geometry
        )
    }
}

impl std::error::Error for GeometryError {}

fn parse_geometry_uint(value: &str) -> Option<u32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse::<u32>().ok()
}

impl ResizeGeometry {
    /// Parse a resize geometry string into a [`ResizeGeometry`].
    ///
    /// Supported forms: `<width>x<height>`, `<width>x`, `x<height>`, and
    /// `<percent>%`. All accepted numbers are non-negative decimal integers;
    /// zero is rejected here so callers do not need to special-case it.
    pub fn parse(geometry: &str) -> Result<Self, GeometryError> {
        let err = || GeometryError {
            geometry: geometry.to_string(),
        };

        if let Some(percent) = geometry.strip_suffix('%') {
            let percent = parse_geometry_uint(percent).ok_or_else(err)?;
            if percent == 0 {
                return Err(err());
            }
            return Ok(Self::Percent(percent));
        }

        let Some((width, height)) = geometry.split_once('x') else {
            return Err(err());
        };

        match (width.is_empty(), height.is_empty()) {
            (false, false) => {
                let width = parse_geometry_uint(width).ok_or_else(err)?;
                let height = parse_geometry_uint(height).ok_or_else(err)?;
                if width == 0 || height == 0 {
                    return Err(err());
                }
                Ok(Self::Exact { width, height })
            }
            (false, true) => {
                let width = parse_geometry_uint(width).ok_or_else(err)?;
                if width == 0 {
                    return Err(err());
                }
                Ok(Self::Width(width))
            }
            (true, false) => {
                let height = parse_geometry_uint(height).ok_or_else(err)?;
                if height == 0 {
                    return Err(err());
                }
                Ok(Self::Height(height))
            }
            (true, true) => Err(err()),
        }
    }

    /// Resolve this geometry against the source dimensions into exact target
    /// `(width, height)` pixel counts.
    ///
    /// Derived axes use ImageMagick's round-half-up integer rule and are
    /// clamped to a minimum of one pixel, matching `magick -resize`.
    pub fn resolve(self, source_width: u32, source_height: u32) -> Result<(u32, u32), ImageError> {
        if source_width == 0 || source_height == 0 {
            return Err(ImageError::InvalidDimensions);
        }
        match self {
            Self::Exact { width, height } => Ok((width, height)),
            Self::Width(width) => {
                let height = scale_axis(source_height, width, source_width)?;
                Ok((width, height))
            }
            Self::Height(height) => {
                let width = scale_axis(source_width, height, source_height)?;
                Ok((width, height))
            }
            Self::Percent(percent) => {
                let width = scale_percent(source_width, percent)?;
                let height = scale_percent(source_height, percent)?;
                Ok((width, height))
            }
        }
    }
}

/// Scale `value` by `percent` using ImageMagick's round-half-up rule, with a
/// minimum result of one. Returns [`ImageError::LengthOverflow`] if the scaled
/// dimension would not fit in a `u32`.
pub fn scale_percent(value: u32, percent: u32) -> Result<u32, ImageError> {
    scaled_dimension(u128::from(value) * u128::from(percent), 100)
}

/// Compute a dimension that preserves the source aspect ratio for a fixed
/// opposite axis, using ImageMagick's round-half-up rule with a minimum of one.
fn scale_axis(source_value: u32, target_other: u32, source_other: u32) -> Result<u32, ImageError> {
    scaled_dimension(
        u128::from(source_value) * u128::from(target_other),
        u128::from(source_other),
    )
}

fn scaled_dimension(numerator: u128, denominator: u128) -> Result<u32, ImageError> {
    let scaled = (numerator * 2 + denominator) / (denominator * 2);
    let scaled = u32::try_from(scaled).map_err(|_| ImageError::LengthOverflow)?;
    Ok(scaled.max(1))
}

pub fn try_vec_with_capacity(capacity: usize) -> Result<Vec<u8>, ImageError> {
    let mut out = Vec::new();
    out.try_reserve_exact(capacity)
        .map_err(|_| ImageError::AllocationFailed {
            requested: capacity,
        })?;
    Ok(out)
}

fn scale_u16be_to_u8(msb: u8, lsb: u8) -> u8 {
    let value = u16::from_be_bytes([msb, lsb]) as u32;
    ((value * 255 + 32767) / 65535) as u8
}

fn rec709_luma8(red: u8, green: u8, blue: u8) -> u8 {
    let value = u32::from(red) * 212_656 + u32::from(green) * 715_158 + u32::from(blue) * 72_186;
    ((value + 500_000) / 1_000_000) as u8
}

fn rec709_luma16be(red: [u8; 2], green: [u8; 2], blue: [u8; 2]) -> u16 {
    let red = u64::from(u16::from_be_bytes(red));
    let green = u64::from(u16::from_be_bytes(green));
    let blue = u64::from(u16::from_be_bytes(blue));
    let value = red * 212_656 + green * 715_158 + blue * 72_186;
    ((value + 500_000) / 1_000_000) as u16
}

fn threshold_u8(value: u8) -> u8 {
    if value < 128 {
        0
    } else {
        255
    }
}

/// Round half-up to the nearest integer and clamp to `0..=255`.
fn round_clamp_u8(value: f64) -> u8 {
    let rounded = (value + 0.5).floor();
    if rounded <= 0.0 {
        0
    } else if rounded >= 255.0 {
        255
    } else {
        rounded as u8
    }
}

/// Round half-up to the nearest integer and clamp to `0..=65535`.
fn round_clamp_u16(value: f64) -> u16 {
    let rounded = (value + 0.5).floor();
    if rounded <= 0.0 {
        0
    } else if rounded >= 65535.0 {
        65535
    } else {
        rounded as u16
    }
}

/// One output sample's contributing source indices and their normalized
/// weights along a single axis.
struct AxisContribution {
    start: usize,
    weights: Vec<f64>,
}

/// Precompute, for every output position along one axis, the contributing
/// source pixel indices and their normalized kernel weights.
///
/// Follows the standard separable-resampling convention: when downscaling
/// (`scale < 1`) the kernel support is widened by `1/scale` so the filter
/// averages the correct source neighborhood; when upscaling the unit-support
/// kernel is used directly. Weights are normalized to sum to 1 so flat regions
/// are preserved exactly.
fn axis_contributions(
    source_len: usize,
    target_len: usize,
    filter: ResizeFilter,
) -> Vec<AxisContribution> {
    let scale = target_len as f64 / source_len as f64;
    let filter_scale = if scale < 1.0 { 1.0 / scale } else { 1.0 };
    let support = filter.support() * filter_scale;
    let mut contributions = Vec::with_capacity(target_len);

    for out in 0..target_len {
        // Center of this output pixel mapped back into source coordinates.
        let center = (out as f64 + 0.5) / scale - 0.5;
        let left = (center - support).ceil();
        let right = (center + support).floor();
        let start = left.max(0.0) as usize;
        let end = (right.min((source_len - 1) as f64)).max(0.0) as usize;

        let mut weights = Vec::with_capacity(end - start + 1);
        let mut total = 0.0;
        for src in start..=end {
            let weight = filter.weight((src as f64 - center) / filter_scale);
            weights.push(weight);
            total += weight;
        }
        if total != 0.0 {
            for weight in &mut weights {
                *weight /= total;
            }
        }
        contributions.push(AxisContribution { start, weights });
    }
    contributions
}

/// Separable two-pass resample of an interleaved plane of `channels` samples.
///
/// `samples` is row-major and `channels`-interleaved, with length
/// `source_width * source_height * channels`. Returns the resampled buffer at
/// the target dimensions in the same interleaved layout, in unrounded
/// floating-point form.
fn resample_planes(
    samples: &[f64],
    source_width: usize,
    source_height: usize,
    target_width: usize,
    target_height: usize,
    channels: usize,
    filter: ResizeFilter,
) -> Result<Vec<f64>, ImageError> {
    // Horizontal pass: source_width -> target_width, height unchanged.
    let horizontal = axis_contributions(source_width, target_width, filter);
    let intermediate_len = target_width
        .checked_mul(source_height)
        .and_then(|value| value.checked_mul(channels))
        .ok_or(ImageError::LengthOverflow)?;
    let mut intermediate = try_vec_with_capacity_f64(intermediate_len)?;
    for y in 0..source_height {
        let row_offset = y * source_width * channels;
        for contribution in &horizontal {
            for channel in 0..channels {
                let mut acc = 0.0;
                for (index, weight) in contribution.weights.iter().enumerate() {
                    let src = row_offset + (contribution.start + index) * channels + channel;
                    acc += samples[src] * weight;
                }
                intermediate.push(acc);
            }
        }
    }

    // Vertical pass: source_height -> target_height, width unchanged.
    let vertical = axis_contributions(source_height, target_height, filter);
    let output_len = target_width
        .checked_mul(target_height)
        .and_then(|value| value.checked_mul(channels))
        .ok_or(ImageError::LengthOverflow)?;
    let mut output = try_vec_with_capacity_f64(output_len)?;
    for contribution in &vertical {
        for x in 0..target_width {
            for channel in 0..channels {
                let mut acc = 0.0;
                for (index, weight) in contribution.weights.iter().enumerate() {
                    let src =
                        ((contribution.start + index) * target_width + x) * channels + channel;
                    acc += intermediate[src] * weight;
                }
                output.push(acc);
            }
        }
    }
    Ok(output)
}

/// Bounded, fallible `Vec<f64>` reservation mirroring [`try_vec_with_capacity`].
fn try_vec_with_capacity_f64(capacity: usize) -> Result<Vec<f64>, ImageError> {
    // Guard the working-space allocation against the same byte budget as pixel
    // buffers so hostile dimensions cannot drive unbounded memory use.
    let bytes = capacity
        .checked_mul(std::mem::size_of::<f64>())
        .ok_or(ImageError::LengthOverflow)?;
    if bytes > MAX_PIXEL_BYTES {
        return Err(ImageError::ImageTooLarge {
            required: bytes,
            limit: MAX_PIXEL_BYTES,
        });
    }
    let mut out = Vec::new();
    out.try_reserve_exact(capacity)
        .map_err(|_| ImageError::AllocationFailed { requested: bytes })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults_icc_to_none_and_with_icc_round_trips() {
        let image = Image::new(1, 1, PixelFormat::Gray8, vec![0x42]).unwrap();
        assert_eq!(image.icc(), None);

        let profile = vec![1, 2, 3, 4];
        let tagged = image.clone().with_icc(Some(profile.clone()));
        assert_eq!(tagged.icc(), Some(profile.as_slice()));

        // Clearing the profile yields None again.
        assert_eq!(tagged.with_icc(None).icc(), None);
    }

    #[test]
    fn icc_profile_participates_in_equality() {
        let base = Image::new(1, 1, PixelFormat::Gray8, vec![0x42]).unwrap();
        let with_a = base.clone().with_icc(Some(vec![1, 2, 3]));
        let with_b = base.clone().with_icc(Some(vec![4, 5, 6]));
        assert_ne!(base, with_a);
        assert_ne!(with_a, with_b);
        assert_eq!(with_a, base.with_icc(Some(vec![1, 2, 3])));
    }

    #[test]
    fn geometry_transforms_preserve_icc_profile() {
        let profile = vec![0xde, 0xad, 0xbe, 0xef];
        let image = Image::new(2, 1, PixelFormat::Rgb8, vec![1, 2, 3, 4, 5, 6])
            .unwrap()
            .with_icc(Some(profile.clone()));

        assert_eq!(
            image.resize_nearest(4, 2).unwrap().icc(),
            Some(profile.as_slice())
        );
        assert_eq!(
            image.resize_nearest_fit(4, 4).unwrap().icc(),
            Some(profile.as_slice())
        );
        assert_eq!(
            image.crop(0, 0, 1, 1).unwrap().icc(),
            Some(profile.as_slice())
        );
        assert_eq!(image.rotate_90().unwrap().icc(), Some(profile.as_slice()));
        assert_eq!(image.rotate_180().unwrap().icc(), Some(profile.as_slice()));
        assert_eq!(image.rotate_270().unwrap().icc(), Some(profile.as_slice()));
        assert_eq!(
            image.flip_vertical().unwrap().icc(),
            Some(profile.as_slice())
        );
        assert_eq!(
            image.flop_horizontal().unwrap().icc(),
            Some(profile.as_slice())
        );

        // A resize that is a no-op still preserves the profile.
        assert_eq!(
            image.resize_nearest(2, 1).unwrap().icc(),
            Some(profile.as_slice())
        );
    }

    #[test]
    fn pixel_format_conversions_drop_icc_profile() {
        let image = Image::new(1, 1, PixelFormat::Rgb8, vec![10, 20, 30])
            .unwrap()
            .with_icc(Some(vec![1, 2, 3, 4]));
        // A real conversion re-encodes the samples, so the source profile is
        // no longer valid and must be dropped.
        assert_eq!(image.to_rgba8().unwrap().icc(), None);
        assert_eq!(image.to_gray8().unwrap().icc(), None);
        assert_eq!(image.to_rgba16be().unwrap().icc(), None);
        assert_eq!(image.to_bilevel().unwrap().icc(), None);
    }

    #[test]
    fn rgb8_expands_to_opaque_farbfeld_quantum_values() {
        let image = Image::new(1, 1, PixelFormat::Rgb8, vec![0x12, 0x80, 0xff]).unwrap();
        assert_eq!(
            image.to_rgba16be().unwrap().pixels(),
            &[0x12, 0x12, 0x80, 0x80, 0xff, 0xff, 0xff, 0xff]
        );
    }

    #[test]
    fn rgba16be_scales_to_8_bit() {
        let image = Image::new(
            1,
            1,
            PixelFormat::Rgba16Be,
            vec![0, 0, 0x80, 0x80, 0xff, 0xff, 0x12, 0x12],
        )
        .unwrap();
        assert_eq!(image.to_rgba8().unwrap().pixels(), &[0, 128, 255, 18]);
    }

    #[test]
    fn rgba_formats_convert_to_rgb8_by_dropping_alpha() {
        let image = Image::new(1, 2, PixelFormat::Rgba8, vec![1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        assert_eq!(image.to_rgb8().unwrap().pixels(), &[1, 2, 3, 5, 6, 7]);
    }

    #[test]
    fn rgb16_converts_to_8_bit_and_farbfeld() {
        let image = Image::new(
            1,
            2,
            PixelFormat::Rgb16Be,
            vec![
                0x00, 0x00, 0x80, 0x00, 0xff, 0xff, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
            ],
        )
        .unwrap();
        assert_eq!(
            image.to_rgb8().unwrap().pixels(),
            &[0, 128, 255, 18, 86, 154]
        );
        assert_eq!(
            image.to_rgba16be().unwrap().pixels(),
            &[
                0x00, 0x00, 0x80, 0x00, 0xff, 0xff, 0xff, 0xff, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
                0xff, 0xff,
            ]
        );
    }

    #[test]
    fn rgba16_and_gray16_convert_to_rgb16() {
        let rgba = Image::new(
            1,
            1,
            PixelFormat::Rgba16Be,
            vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0],
        )
        .unwrap();
        assert_eq!(
            rgba.to_rgb16be().unwrap().pixels(),
            &[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]
        );

        let gray = Image::new(1, 1, PixelFormat::Gray16Be, vec![0x80, 0x00]).unwrap();
        assert_eq!(
            gray.to_rgb16be().unwrap().pixels(),
            &[0x80, 0x00, 0x80, 0x00, 0x80, 0x00]
        );
    }

    #[test]
    fn gray_formats_expand_to_rgb_and_farbfeld() {
        let gray = Image::new(1, 2, PixelFormat::Gray8, vec![0x12, 0x80]).unwrap();
        assert_eq!(
            gray.to_rgb8().unwrap().pixels(),
            &[0x12, 0x12, 0x12, 0x80, 0x80, 0x80]
        );
        assert_eq!(
            gray.to_rgba16be().unwrap().pixels(),
            &[
                0x12, 0x12, 0x12, 0x12, 0x12, 0x12, 0xff, 0xff, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
                0xff, 0xff
            ]
        );
    }

    #[test]
    fn bilevel_expands_to_gray_and_color_formats() {
        let bilevel = Image::new(1, 2, PixelFormat::Bilevel, vec![0, 255]).unwrap();
        assert_eq!(bilevel.to_gray8().unwrap().pixels(), &[0, 255]);
        assert_eq!(
            bilevel.to_rgb8().unwrap().pixels(),
            &[0, 0, 0, 255, 255, 255]
        );
        assert_eq!(
            bilevel.to_rgba16be().unwrap().pixels(),
            &[0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
        );
    }

    #[test]
    fn rgb_and_farbfeld_convert_to_rec709_gray() {
        let rgb = Image::new(
            4,
            1,
            PixelFormat::Rgb8,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        )
        .unwrap();
        assert_eq!(rgb.to_gray8().unwrap().pixels(), &[54, 182, 18, 255]);

        let rgba16 = Image::new(
            1,
            1,
            PixelFormat::Rgba16Be,
            vec![0x80, 0x00, 0x40, 0x00, 0x20, 0x00, 0xff, 0xff],
        )
        .unwrap();
        assert_eq!(rgba16.to_gray16be().unwrap().pixels(), &[0x4b, 0x4d]);

        let rgb16 = Image::new(
            1,
            1,
            PixelFormat::Rgb16Be,
            vec![0x80, 0x00, 0x40, 0x00, 0x20, 0x00],
        )
        .unwrap();
        assert_eq!(rgb16.to_gray16be().unwrap().pixels(), &[0x4b, 0x4d]);
    }

    #[test]
    fn grayscale_and_color_convert_to_bilevel_threshold() {
        let gray = Image::new(1, 4, PixelFormat::Gray8, vec![0, 127, 128, 255]).unwrap();
        assert_eq!(gray.to_bilevel().unwrap().pixels(), &[0, 0, 255, 255]);

        let rgba16 = Image::new(
            1,
            2,
            PixelFormat::Rgba16Be,
            vec![
                0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00,
                0xff, 0xff,
            ],
        )
        .unwrap();
        assert_eq!(rgba16.to_bilevel().unwrap().pixels(), &[0, 255]);

        let rgb16 = Image::new(
            1,
            2,
            PixelFormat::Rgb16Be,
            vec![
                0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00,
            ],
        )
        .unwrap();
        assert_eq!(rgb16.to_bilevel().unwrap().pixels(), &[0, 255]);
    }

    #[test]
    fn resize_nearest_preserves_format_and_samples() {
        let image = Image::new(
            2,
            2,
            PixelFormat::Rgb8,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        )
        .unwrap();
        let resized = image.resize_nearest(4, 2).unwrap();
        assert_eq!(resized.width(), 4);
        assert_eq!(resized.height(), 2);
        assert_eq!(resized.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(
            resized.pixels(),
            &[1, 2, 3, 1, 2, 3, 4, 5, 6, 4, 5, 6, 7, 8, 9, 7, 8, 9, 10, 11, 12, 10, 11, 12]
        );
    }

    #[test]
    fn resize_nearest_rejects_invalid_and_oversized_dimensions() {
        let image = Image::new(1, 1, PixelFormat::Rgba16Be, vec![0; 8]).unwrap();
        assert_eq!(
            image.resize_nearest(0, 1),
            Err(ImageError::InvalidDimensions)
        );
        assert!(matches!(
            image.resize_nearest(u32::MAX, u32::MAX),
            Err(ImageError::LengthOverflow | ImageError::ImageTooLarge { .. })
        ));
    }

    #[test]
    fn resize_nearest_uses_center_sampled_coordinates() {
        let image = Image::new(3, 1, PixelFormat::Gray8, vec![0x10, 0x80, 0xf0]).unwrap();
        let resized = image.resize_nearest(2, 1).unwrap();
        assert_eq!(resized.pixels(), &[0x10, 0xf0]);
    }

    #[test]
    fn resize_filtered_point_matches_resize_nearest_byte_for_byte() {
        let image = Image::new(
            3,
            2,
            PixelFormat::Rgb8,
            vec![
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
            ],
        )
        .unwrap();
        for (w, h) in [(2, 1), (5, 4), (1, 1), (7, 3)] {
            let nearest = image.resize_nearest(w, h).unwrap();
            let point = image.resize_filtered(w, h, ResizeFilter::Point).unwrap();
            assert_eq!(point.pixels(), nearest.pixels(), "{w}x{h}");
            assert_eq!(point.pixel_format(), nearest.pixel_format());
        }
    }

    #[test]
    fn resize_filtered_preserves_dimensions_and_format() {
        let image = Image::new(4, 4, PixelFormat::Rgba8, vec![128; 64]).unwrap();
        for filter in [
            ResizeFilter::Box,
            ResizeFilter::Triangle,
            ResizeFilter::CatmullRom,
            ResizeFilter::Lanczos3,
        ] {
            let resized = image.resize_filtered(2, 3, filter).unwrap();
            assert_eq!(resized.width(), 2);
            assert_eq!(resized.height(), 3);
            assert_eq!(resized.pixel_format(), PixelFormat::Rgba8);
        }
    }

    #[test]
    fn resize_filtered_preserves_flat_color() {
        // A uniform image must resample to the same uniform value under every
        // kernel because normalized weights sum to one.
        let image = Image::new(5, 5, PixelFormat::Rgb8, vec![73; 75]).unwrap();
        for filter in [
            ResizeFilter::Box,
            ResizeFilter::Triangle,
            ResizeFilter::CatmullRom,
            ResizeFilter::Lanczos3,
        ] {
            let down = image.resize_filtered(2, 2, filter).unwrap();
            assert!(down.pixels().iter().all(|&value| value == 73), "{filter:?}");
            let up = image.resize_filtered(9, 9, filter).unwrap();
            assert!(up.pixels().iter().all(|&value| value == 73), "{filter:?}");
        }
    }

    #[test]
    fn resize_filtered_box_downscale_averages_pixels() {
        // 2x1 row halved horizontally: box filter averages both source pixels.
        let image = Image::new(2, 1, PixelFormat::Gray8, vec![0, 200]).unwrap();
        let resized = image.resize_filtered(1, 1, ResizeFilter::Box).unwrap();
        // round_half_up((0 + 200) / 2) = 100.
        assert_eq!(resized.pixels(), &[100]);
    }

    #[test]
    fn resize_filtered_triangle_upscale_is_monotonic() {
        let image = Image::new(2, 1, PixelFormat::Gray8, vec![0, 255]).unwrap();
        let resized = image.resize_filtered(5, 1, ResizeFilter::Triangle).unwrap();
        let pixels = resized.pixels();
        assert_eq!(pixels.len(), 5);
        for window in pixels.windows(2) {
            assert!(window[1] >= window[0], "expected non-decreasing ramp");
        }
        assert_eq!(pixels[0], 0);
        assert_eq!(pixels[4], 255);
    }

    #[test]
    fn resize_filtered_is_byte_deterministic() {
        let image = Image::new(
            6,
            5,
            PixelFormat::Rgb8,
            (0..90).map(|value| (value * 7 % 256) as u8).collect(),
        )
        .unwrap();
        for filter in [
            ResizeFilter::Box,
            ResizeFilter::Triangle,
            ResizeFilter::CatmullRom,
            ResizeFilter::Lanczos3,
        ] {
            let first = image.resize_filtered(3, 4, filter).unwrap();
            let second = image.resize_filtered(3, 4, filter).unwrap();
            assert_eq!(first.pixels(), second.pixels(), "{filter:?}");
        }
    }

    #[test]
    fn resize_filtered_16bit_preserves_format_and_endianness() {
        let image = Image::new(2, 1, PixelFormat::Gray16Be, vec![0x00, 0x00, 0xff, 0xff]).unwrap();
        let resized = image.resize_filtered(1, 1, ResizeFilter::Box).unwrap();
        assert_eq!(resized.pixel_format(), PixelFormat::Gray16Be);
        // round_half_up((0 + 65535) / 2) = 32768 -> 0x8000 big-endian.
        assert_eq!(resized.pixels(), &[0x80, 0x00]);
    }

    #[test]
    fn resize_filtered_bilevel_rethresholds_output() {
        let image = Image::new(4, 1, PixelFormat::Bilevel, vec![0, 0, 255, 255]).unwrap();
        let resized = image.resize_filtered(2, 1, ResizeFilter::Triangle).unwrap();
        assert_eq!(resized.pixel_format(), PixelFormat::Bilevel);
        for &value in resized.pixels() {
            assert!(
                value == 0 || value == 255,
                "bilevel output must be 0 or 255"
            );
        }
    }

    #[test]
    fn resize_filtered_same_dimensions_returns_clone() {
        let image = Image::new(3, 2, PixelFormat::Rgb8, (0..18).collect()).unwrap();
        let resized = image.resize_filtered(3, 2, ResizeFilter::Lanczos3).unwrap();
        assert_eq!(resized.pixels(), image.pixels());
    }

    #[test]
    fn resize_filtered_rejects_invalid_and_oversized_dimensions() {
        let image = Image::new(2, 2, PixelFormat::Rgb8, vec![0; 12]).unwrap();
        assert_eq!(
            image.resize_filtered(0, 1, ResizeFilter::Lanczos3),
            Err(ImageError::InvalidDimensions)
        );
        assert!(matches!(
            image.resize_filtered(u32::MAX, u32::MAX, ResizeFilter::Lanczos3),
            Err(ImageError::LengthOverflow | ImageError::ImageTooLarge { .. })
        ));
    }

    #[test]
    fn resize_filtered_fit_uses_fitted_dimensions() {
        let image = Image::new(4, 2, PixelFormat::Rgb8, vec![20; 24]).unwrap();
        let resized = image
            .resize_filtered_fit(2, 2, ResizeFilter::Triangle)
            .unwrap();
        assert_eq!(resized.width(), 2);
        assert_eq!(resized.height(), 1);
        assert!(resized.pixels().iter().all(|&value| value == 20));
    }

    #[test]
    fn resize_filter_lanczos3_weight_is_unit_at_origin_and_zero_at_integers() {
        assert!((ResizeFilter::Lanczos3.weight(0.0) - 1.0).abs() < 1e-9);
        assert!(ResizeFilter::Lanczos3.weight(1.0).abs() < 1e-9);
        assert!(ResizeFilter::Lanczos3.weight(2.0).abs() < 1e-9);
        assert_eq!(ResizeFilter::Lanczos3.weight(3.5), 0.0);
    }

    #[test]
    fn fit_dimensions_match_imagemagick_resize_box_rounding() {
        assert_eq!(fit_dimensions(64, 64, 17, 11).unwrap(), (11, 11));
        assert_eq!(fit_dimensions(64, 32, 17, 11).unwrap(), (17, 9));
        assert_eq!(fit_dimensions(32, 64, 17, 11).unwrap(), (6, 11));
        assert_eq!(fit_dimensions(3, 2, 17, 11).unwrap(), (17, 11));
        assert_eq!(fit_dimensions(2, 3, 17, 11).unwrap(), (7, 11));
        assert_eq!(fit_dimensions(5, 1, 17, 11).unwrap(), (17, 3));
        assert_eq!(fit_dimensions(1, 5, 17, 11).unwrap(), (2, 11));
        assert_eq!(fit_dimensions(64, 32, 1, 100).unwrap(), (1, 1));
        assert_eq!(fit_dimensions(32, 64, 100, 1).unwrap(), (1, 1));
    }

    #[test]
    fn parse_resize_geometry_accepts_supported_forms() {
        assert_eq!(
            ResizeGeometry::parse("100x40").unwrap(),
            ResizeGeometry::Exact {
                width: 100,
                height: 40
            }
        );
        assert_eq!(
            ResizeGeometry::parse("200x").unwrap(),
            ResizeGeometry::Width(200)
        );
        assert_eq!(
            ResizeGeometry::parse("x600").unwrap(),
            ResizeGeometry::Height(600)
        );
        assert_eq!(
            ResizeGeometry::parse("50%").unwrap(),
            ResizeGeometry::Percent(50)
        );
        assert_eq!(
            ResizeGeometry::parse("1%").unwrap(),
            ResizeGeometry::Percent(1)
        );
    }

    #[test]
    fn parse_resize_geometry_rejects_malformed_input() {
        for garbage in [
            "", "x", "abc", "50%%", "%50", "-5x10", "10x-5", "0x10", "10x0", "0%", "x0", "0x",
            "1.5x2", "50.0%", "10x10x10", "10 x10", " 10x10", "10x10 ", "1e3%", "+5x5", "xx",
        ] {
            assert!(
                ResizeGeometry::parse(garbage).is_err(),
                "expected {garbage:?} to be rejected"
            );
        }
    }

    #[test]
    fn resize_geometry_resolves_exact_dimensions_verbatim() {
        assert_eq!(
            ResizeGeometry::Exact {
                width: 5,
                height: 3
            }
            .resolve(9, 7)
            .unwrap(),
            (5, 3)
        );
    }

    #[test]
    fn resize_geometry_percent_matches_imagemagick_round_half_up() {
        assert_eq!(
            ResizeGeometry::Percent(50).resolve(100, 40).unwrap(),
            (50, 20)
        );
        assert_eq!(
            ResizeGeometry::Percent(50).resolve(101, 41).unwrap(),
            (51, 21)
        );
        assert_eq!(
            ResizeGeometry::Percent(33).resolve(100, 100).unwrap(),
            (33, 33)
        );
        assert_eq!(ResizeGeometry::Percent(25).resolve(10, 10).unwrap(), (3, 3));
        assert_eq!(ResizeGeometry::Percent(200).resolve(7, 3).unwrap(), (14, 6));
        assert_eq!(ResizeGeometry::Percent(10).resolve(5, 5).unwrap(), (1, 1));
        assert_eq!(ResizeGeometry::Percent(1).resolve(1, 1).unwrap(), (1, 1));
    }

    #[test]
    fn resize_geometry_single_axis_matches_imagemagick_round_half_up() {
        assert_eq!(
            ResizeGeometry::Width(200).resolve(100, 40).unwrap(),
            (200, 80)
        );
        assert_eq!(
            ResizeGeometry::Width(50).resolve(100, 40).unwrap(),
            (50, 20)
        );
        assert_eq!(
            ResizeGeometry::Height(10).resolve(100, 40).unwrap(),
            (25, 10)
        );
        assert_eq!(ResizeGeometry::Width(3).resolve(9, 7).unwrap(), (3, 2));
        assert_eq!(ResizeGeometry::Height(3).resolve(9, 7).unwrap(), (4, 3));
        assert_eq!(ResizeGeometry::Width(1).resolve(100, 1).unwrap(), (1, 1));
        assert_eq!(ResizeGeometry::Height(1).resolve(1, 100).unwrap(), (1, 1));
    }

    #[test]
    fn resize_geometry_rejects_zero_source_dimensions() {
        assert_eq!(
            ResizeGeometry::Percent(50).resolve(0, 10),
            Err(ImageError::InvalidDimensions)
        );
        assert_eq!(
            ResizeGeometry::Width(10).resolve(10, 0),
            Err(ImageError::InvalidDimensions)
        );
    }

    #[test]
    fn scale_percent_clamps_to_one() {
        assert_eq!(scale_percent(1, 10).unwrap(), 1);
        assert_eq!(scale_percent(100, 50).unwrap(), 50);
        assert_eq!(scale_percent(3, 50).unwrap(), 2);
        assert_eq!(scale_percent(0, 50).unwrap(), 1);
    }

    #[test]
    fn resize_geometry_rejects_dimensions_overflowing_u32() {
        assert_eq!(
            ResizeGeometry::Percent(u32::MAX).resolve(1000, 1000),
            Err(ImageError::LengthOverflow)
        );
        assert_eq!(
            ResizeGeometry::Width(u32::MAX).resolve(1, 1000),
            Err(ImageError::LengthOverflow)
        );
    }

    #[test]
    fn resize_nearest_fit_preserves_format_and_uses_fitted_dimensions() {
        let image = Image::new(
            3,
            2,
            PixelFormat::Rgb8,
            vec![
                255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0, 0, 255, 255, 255, 0, 255,
            ],
        )
        .unwrap();
        let resized = image.resize_nearest_fit(4, 4).unwrap();
        assert_eq!(resized.width(), 4);
        assert_eq!(resized.height(), 3);
        assert_eq!(resized.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(
            resized.pixels(),
            &[
                255, 0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0, 0, 255, 255, 0, 255, 255,
                255, 0, 255, 255, 255, 0, 0, 255, 255, 0, 255, 255, 255, 0, 255
            ]
        );
    }

    #[test]
    fn resize_nearest_fit_rejects_invalid_and_oversized_dimensions() {
        let image = Image::new(1, 1, PixelFormat::Rgba16Be, vec![0; 8]).unwrap();
        assert_eq!(
            image.resize_nearest_fit(0, 1),
            Err(ImageError::InvalidDimensions)
        );
        assert!(matches!(
            image.resize_nearest_fit(u32::MAX, u32::MAX),
            Err(ImageError::LengthOverflow | ImageError::ImageTooLarge { .. })
        ));
    }

    fn rgb8_3x2() -> Image {
        Image::new(
            3,
            2,
            PixelFormat::Rgb8,
            vec![1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6],
        )
        .unwrap()
    }

    fn gray8_2x3() -> Image {
        Image::new(2, 3, PixelFormat::Gray8, vec![1, 2, 3, 4, 5, 6]).unwrap()
    }

    #[test]
    fn crop_extracts_subregion_and_preserves_format() {
        let image = rgb8_3x2();
        let cropped = image.crop(1, 0, 2, 2).unwrap();
        assert_eq!(cropped.width(), 2);
        assert_eq!(cropped.height(), 2);
        assert_eq!(cropped.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(cropped.pixels(), &[2, 2, 2, 3, 3, 3, 5, 5, 5, 6, 6, 6]);
    }

    #[test]
    fn crop_single_pixel_picks_exact_offset() {
        let image = gray8_2x3();
        assert_eq!(image.crop(1, 2, 1, 1).unwrap().pixels(), &[6]);
        assert_eq!(image.crop(0, 0, 1, 1).unwrap().pixels(), &[1]);
    }

    #[test]
    fn crop_rejects_zero_and_out_of_bounds_regions() {
        let image = rgb8_3x2();
        assert!(matches!(
            image.crop(0, 0, 0, 1),
            Err(ImageError::CropOutOfBounds { .. })
        ));
        assert!(matches!(
            image.crop(0, 0, 1, 0),
            Err(ImageError::CropOutOfBounds { .. })
        ));
        assert!(matches!(
            image.crop(2, 0, 2, 1),
            Err(ImageError::CropOutOfBounds { .. })
        ));
        assert!(matches!(
            image.crop(0, 1, 1, 2),
            Err(ImageError::CropOutOfBounds { .. })
        ));
        assert_eq!(
            image.crop(u32::MAX, 0, u32::MAX, 1).unwrap_err(),
            ImageError::CropOutOfBounds {
                x: u32::MAX,
                y: 0,
                width: u32::MAX,
                height: 1,
                source_width: 3,
                source_height: 2,
            }
        );
    }

    #[test]
    fn rotate_90_swaps_dimensions_and_places_pixels_clockwise() {
        let image = rgb8_3x2();
        let rotated = image.rotate_90().unwrap();
        assert_eq!(rotated.width(), 2);
        assert_eq!(rotated.height(), 3);
        assert_eq!(rotated.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(
            rotated.pixels(),
            &[4, 4, 4, 1, 1, 1, 5, 5, 5, 2, 2, 2, 6, 6, 6, 3, 3, 3]
        );
    }

    #[test]
    fn rotate_270_swaps_dimensions_and_places_pixels_counterclockwise() {
        let image = rgb8_3x2();
        let rotated = image.rotate_270().unwrap();
        assert_eq!(rotated.width(), 2);
        assert_eq!(rotated.height(), 3);
        assert_eq!(
            rotated.pixels(),
            &[3, 3, 3, 6, 6, 6, 2, 2, 2, 5, 5, 5, 1, 1, 1, 4, 4, 4]
        );
    }

    #[test]
    fn rotate_180_reverses_pixels_and_keeps_dimensions() {
        let image = gray8_2x3();
        let rotated = image.rotate_180().unwrap();
        assert_eq!(rotated.width(), 2);
        assert_eq!(rotated.height(), 3);
        assert_eq!(rotated.pixels(), &[6, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn rotate_90_then_270_round_trips() {
        let image = gray8_2x3();
        assert_eq!(image.rotate_90().unwrap().rotate_270().unwrap(), image);
    }

    #[test]
    fn flip_vertical_mirrors_rows() {
        let image = gray8_2x3();
        let flipped = image.flip_vertical().unwrap();
        assert_eq!(flipped.width(), 2);
        assert_eq!(flipped.height(), 3);
        assert_eq!(flipped.pixels(), &[5, 6, 3, 4, 1, 2]);
    }

    #[test]
    fn flop_horizontal_mirrors_columns() {
        let image = rgb8_3x2();
        let flopped = image.flop_horizontal().unwrap();
        assert_eq!(flopped.width(), 3);
        assert_eq!(flopped.height(), 2);
        assert_eq!(
            flopped.pixels(),
            &[3, 3, 3, 2, 2, 2, 1, 1, 1, 6, 6, 6, 5, 5, 5, 4, 4, 4]
        );
    }

    #[test]
    fn geometry_operations_preserve_wide_pixel_formats() {
        let image = Image::new(
            2,
            1,
            PixelFormat::Rgba16Be,
            vec![
                0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
                0xff, 0x00,
            ],
        )
        .unwrap();
        let rotated = image.rotate_90().unwrap();
        assert_eq!(rotated.width(), 1);
        assert_eq!(rotated.height(), 2);
        assert_eq!(
            rotated.pixels(),
            &[
                0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
                0xff, 0x00
            ]
        );
        let flopped = image.flop_horizontal().unwrap();
        assert_eq!(
            flopped.pixels(),
            &[
                0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
                0x77, 0x88
            ]
        );
    }

    #[test]
    fn compare_rgba8_reports_identical_for_equal_pixels() {
        let a = Image::new(2, 1, PixelFormat::Rgb8, vec![10, 20, 30, 40, 50, 60]).unwrap();
        let b = Image::new(2, 1, PixelFormat::Rgb8, vec![10, 20, 30, 40, 50, 60]).unwrap();
        let cmp = compare_rgba8(&a, &b).unwrap();
        assert!(cmp.is_identical());
        assert_eq!(cmp.differing_pixels, 0);
        assert_eq!(cmp.total_pixels, 2);
        assert_eq!(cmp.max_abs_diff, 0);
        assert_eq!(cmp.sum_abs_diff, 0);
        assert_eq!(cmp.mae(), 0.0);
        assert!(cmp.psnr().is_infinite());
    }

    #[test]
    fn compare_rgba8_normalizes_rgb_and_rgba_for_equal_color() {
        let rgb = Image::new(1, 1, PixelFormat::Rgb8, vec![1, 2, 3]).unwrap();
        let rgba = Image::new(1, 1, PixelFormat::Rgba8, vec![1, 2, 3, 0xff]).unwrap();
        let cmp = compare_rgba8(&rgb, &rgba).unwrap();
        assert!(cmp.is_identical());
    }

    #[test]
    fn compare_rgba8_counts_pixels_and_peaks_per_channel() {
        let a = Image::new(1, 1, PixelFormat::Rgba8, vec![100, 100, 100, 100]).unwrap();
        let b = Image::new(1, 1, PixelFormat::Rgba8, vec![105, 110, 100, 100]).unwrap();
        let cmp = compare_rgba8(&a, &b).unwrap();
        assert!(!cmp.is_identical());
        assert_eq!(cmp.differing_pixels, 1);
        assert_eq!(cmp.total_pixels, 1);
        assert_eq!(cmp.max_abs_diff, 10);
        assert_eq!(cmp.sum_abs_diff, 15);
        assert_eq!(cmp.mae(), 3.75);
        let psnr = cmp.psnr();
        assert!(psnr.is_finite() && psnr > 0.0);
    }

    #[test]
    fn compare_rgba8_rejects_dimension_mismatch() {
        let a = Image::new(2, 1, PixelFormat::Rgb8, vec![0, 0, 0, 0, 0, 0]).unwrap();
        let b = Image::new(1, 1, PixelFormat::Rgb8, vec![0, 0, 0]).unwrap();
        assert_eq!(compare_rgba8(&a, &b), Err(ImageError::InvalidDimensions));
    }

    /// A 3x2 grayscale image with a distinct value per pixel, used as an
    /// asymmetric reference for orientation tests:
    ///
    /// ```text
    /// 1 2 3
    /// 4 5 6
    /// ```
    fn asymmetric_3x2() -> Image {
        Image::new(3, 2, PixelFormat::Gray8, vec![1, 2, 3, 4, 5, 6]).unwrap()
    }

    /// Reference implementation of the EXIF orientation target mapping,
    /// independent of the [`Image`] rotate/flip helpers, so the helper under
    /// test is checked against an from-first-principles transform.
    fn reference_oriented(image: &Image, orientation: u16) -> Image {
        let width = image.width() as usize;
        let height = image.height() as usize;
        let bpp = image.pixel_format().bytes_per_pixel();
        let (out_width, out_height) = match orientation {
            5..=8 => (height, width),
            _ => (width, height),
        };
        let mut out = vec![0u8; out_width * out_height * bpp];
        for y in 0..height {
            for x in 0..width {
                let (ox, oy) = match orientation {
                    2 => (width - 1 - x, y),
                    3 => (width - 1 - x, height - 1 - y),
                    4 => (x, height - 1 - y),
                    5 => (y, x),
                    6 => (height - 1 - y, x),
                    7 => (height - 1 - y, width - 1 - x),
                    8 => (y, width - 1 - x),
                    _ => (x, y),
                };
                let src = (y * width + x) * bpp;
                let dst = (oy * out_width + ox) * bpp;
                out[dst..dst + bpp].copy_from_slice(&image.pixels()[src..src + bpp]);
            }
        }
        Image::new(
            out_width as u32,
            out_height as u32,
            image.pixel_format(),
            out,
        )
        .unwrap()
    }

    #[test]
    fn apply_exif_orientation_identity_is_noop() {
        let image = asymmetric_3x2();
        assert_eq!(
            apply_exif_orientation(image.clone(), 1).unwrap(),
            image,
            "orientation 1 must return the image unchanged"
        );
    }

    #[test]
    fn apply_exif_orientation_out_of_range_is_identity() {
        let image = asymmetric_3x2();
        for orientation in [0u16, 9, 42, u16::MAX] {
            assert_eq!(
                apply_exif_orientation(image.clone(), orientation).unwrap(),
                image,
                "orientation {orientation} must be treated as identity"
            );
        }
    }

    #[test]
    fn apply_exif_orientation_matches_reference_for_all_values() {
        let image = asymmetric_3x2();
        for orientation in 1..=8u16 {
            let oriented = apply_exif_orientation(image.clone(), orientation).unwrap();
            let expected = reference_oriented(&image, orientation);
            assert_eq!(
                oriented, expected,
                "orientation {orientation} did not match the reference transform"
            );
            let (ew, eh) = exif_oriented_dimensions(orientation, image.width(), image.height());
            assert_eq!(
                (oriented.width(), oriented.height()),
                (ew, eh),
                "orientation {orientation} dimensions disagree with exif_oriented_dimensions"
            );
        }
    }

    #[test]
    fn exif_oriented_dimensions_swaps_only_for_rotated_values() {
        for orientation in [1u16, 2, 3, 4, 0, 9] {
            assert_eq!(exif_oriented_dimensions(orientation, 3, 2), (3, 2));
        }
        for orientation in 5..=8u16 {
            assert_eq!(exif_oriented_dimensions(orientation, 3, 2), (2, 3));
        }
    }
}
