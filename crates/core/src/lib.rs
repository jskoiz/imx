use std::fmt;

pub const MAX_PIXEL_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Farbfeld,
    Pbm,
    Pgm,
    Png,
    Ppm,
    Qoi,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Self::Farbfeld => "FARBFELD",
            Self::Pbm => "PBM",
            Self::Pgm => "PGM",
            Self::Png => "PNG",
            Self::Ppm => "PPM",
            Self::Qoi => "QOI",
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
    UnsupportedFormat(String),
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
}
