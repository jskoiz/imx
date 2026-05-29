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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    pixels: Vec<u8>,
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
        })
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

        Self::new(width, height, self.pixel_format, out)
    }

    pub fn resize_nearest_fit(&self, width: u32, height: u32) -> Result<Self, ImageError> {
        let (width, height) = fit_dimensions(self.width, self.height, width, height)?;
        self.resize_nearest(width, height)
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

        Self::new(width, height, self.pixel_format, out)
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
        Self::new(self.height, self.width, self.pixel_format, out)
    }

    pub fn rotate_180(&self) -> Result<Self, ImageError> {
        let bytes_per_pixel = self.pixel_format.bytes_per_pixel();
        let output_len = pixel_len(self.width, self.height, bytes_per_pixel)?;
        let mut out = try_vec_with_capacity(output_len)?;
        for pixel in self.pixels.chunks_exact(bytes_per_pixel).rev() {
            out.extend_from_slice(pixel);
        }
        Self::new(self.width, self.height, self.pixel_format, out)
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
        Self::new(self.height, self.width, self.pixel_format, out)
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
        Self::new(self.width, self.height, self.pixel_format, out)
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
        Self::new(self.width, self.height, self.pixel_format, out)
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
