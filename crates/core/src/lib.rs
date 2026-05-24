use std::fmt;

pub const MAX_PIXEL_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Farbfeld,
    Ppm,
    Qoi,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Self::Farbfeld => "FARBFELD",
            Self::Ppm => "PPM",
            Self::Qoi => "QOI",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb8,
    Rgba8,
    Rgba16Be,
}

impl PixelFormat {
    pub fn channels(self) -> &'static str {
        match self {
            Self::Rgb8 => "RGB",
            Self::Rgba8 => "RGBA",
            Self::Rgba16Be => "RGBA",
        }
    }

    pub fn depth(self) -> u8 {
        match self {
            Self::Rgb8 | Self::Rgba8 => 8,
            Self::Rgba16Be => 16,
        }
    }

    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgb8 => 3,
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
            PixelFormat::Rgb8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 4)?)?;
                for px in self.pixels.chunks_exact(3) {
                    out.extend_from_slice(px);
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
            PixelFormat::Rgba8 => {
                let mut out = try_vec_with_capacity(pixel_len(self.width, self.height, 3)?)?;
                for px in self.pixels.chunks_exact(4) {
                    out.extend_from_slice(&px[..3]);
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
    ImageTooLarge { required: usize, limit: usize },
    UnexpectedEof { expected: usize, actual: usize },
    InvalidPixelBuffer { expected: usize, actual: usize },
    AllocationFailed { requested: usize },
    InvalidChannels { channels: u8 },
    InvalidColorspace { colorspace: u8 },
    InvalidMaxValue { max_value: u32 },
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
            Self::InvalidMaxValue { max_value } => {
                write!(f, "PPM max value must be 1..=255, got {max_value}")
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
}
