use imx_core::{
    pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError, PixelFormat,
};

pub const P3_MAGIC: &[u8; 2] = b"P3";
pub const P6_MAGIC: &[u8; 2] = b"P6";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpmHeader {
    pub encoding: PpmEncoding,
    pub width: u32,
    pub height: u32,
    pub max_value: u32,
    pub raster_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PpmEncoding {
    AsciiP3,
    BinaryP6,
}

pub fn identify(input: &[u8]) -> Result<Identify, ImageError> {
    let header = decode_header(input)?;
    Ok(Identify {
        format: Format::Ppm,
        width: header.width,
        height: header.height,
        pixel_format: PixelFormat::Rgb8,
    })
}

pub fn decode_header(input: &[u8]) -> Result<PpmHeader, ImageError> {
    let mut parser = Parser::new(input);
    let encoding = match parser.next_token()? {
        magic if magic == P3_MAGIC => PpmEncoding::AsciiP3,
        magic if magic == P6_MAGIC => PpmEncoding::BinaryP6,
        _ => return Err(ImageError::InvalidHeader("PPM")),
    };

    let width = parse_u32(parser.next_token()?)?;
    let height = parse_u32(parser.next_token()?)?;
    let max_value = parse_u32(parser.next_token()?)?;
    if max_value == 0 || max_value > 255 {
        return Err(ImageError::InvalidMaxValue { max_value });
    }
    let _ = pixel_len(width, height, 3)?;
    if encoding == PpmEncoding::BinaryP6 {
        parser.consume_raster_separator()?;
    }

    Ok(PpmHeader {
        encoding,
        width,
        height,
        max_value,
        raster_offset: parser.offset(),
    })
}

pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    let header = decode_header(input)?;
    let payload_len = pixel_len(header.width, header.height, 3)?;
    let pixels = match header.encoding {
        PpmEncoding::AsciiP3 => decode_ascii_raster(input, header, payload_len)?,
        PpmEncoding::BinaryP6 => decode_binary_raster(input, header, payload_len)?,
    };
    Image::new(header.width, header.height, PixelFormat::Rgb8, pixels)
}

pub fn encode(image: &Image) -> Result<Vec<u8>, ImageError> {
    let rgb = image.to_rgb8()?;
    let payload_len = pixel_len(rgb.width(), rgb.height(), 3)?;
    let header = format!("P6\n{} {}\n255\n", rgb.width(), rgb.height());
    let capacity = header
        .len()
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = try_vec_with_capacity(capacity)?;
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(rgb.pixels());
    Ok(out)
}

fn parse_u32(token: &[u8]) -> Result<u32, ImageError> {
    if token.is_empty() || !token.iter().all(u8::is_ascii_digit) {
        return Err(ImageError::InvalidHeader("PPM"));
    }
    let text = std::str::from_utf8(token).map_err(|_| ImageError::InvalidHeader("PPM"))?;
    text.parse::<u32>()
        .map_err(|_| ImageError::InvalidHeader("PPM"))
}

fn decode_ascii_raster(
    input: &[u8],
    header: PpmHeader,
    payload_len: usize,
) -> Result<Vec<u8>, ImageError> {
    let mut parser = Parser {
        input,
        offset: header.raster_offset,
    };
    let mut pixels = try_vec_with_capacity(payload_len)?;
    for index in 0..payload_len {
        let token = parser.next_token().map_err(|err| match err {
            ImageError::UnexpectedEof { actual, .. } => ImageError::UnexpectedEof {
                expected: header.raster_offset + index + 1,
                actual,
            },
            other => other,
        })?;
        let sample = parse_u32(token)?;
        if sample > header.max_value {
            return Err(ImageError::InvalidHeader("PPM"));
        }
        pixels.push(scale_sample_to_255(sample, header.max_value));
    }
    Ok(pixels)
}

fn decode_binary_raster(
    input: &[u8],
    header: PpmHeader,
    payload_len: usize,
) -> Result<Vec<u8>, ImageError> {
    let expected_len = header
        .raster_offset
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    if input.len() < expected_len {
        return Err(ImageError::UnexpectedEof {
            expected: expected_len,
            actual: input.len(),
        });
    }

    let mut pixels = input[header.raster_offset..expected_len].to_vec();
    if header.max_value != 255 {
        for sample in &mut pixels {
            *sample = scale_sample_to_255(u32::from(*sample), header.max_value);
        }
    }
    Ok(pixels)
}

fn scale_sample_to_255(sample: u32, max_value: u32) -> u8 {
    ((sample * 255 + (max_value / 2)) / max_value) as u8
}

struct Parser<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn offset(&self) -> usize {
        self.offset
    }

    fn next_token(&mut self) -> Result<&'a [u8], ImageError> {
        self.skip_whitespace_and_comments()?;
        let start = self.offset;
        while self.offset < self.input.len() {
            let byte = self.input[self.offset];
            if byte.is_ascii_whitespace() || byte == b'#' {
                break;
            }
            self.offset += 1;
        }
        if start == self.offset {
            return Err(ImageError::UnexpectedEof {
                expected: self.offset + 1,
                actual: self.input.len(),
            });
        }
        Ok(&self.input[start..self.offset])
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<(), ImageError> {
        loop {
            while self.offset < self.input.len() && self.input[self.offset].is_ascii_whitespace() {
                self.offset += 1;
            }
            if self.offset < self.input.len() && self.input[self.offset] == b'#' {
                self.offset += 1;
                while self.offset < self.input.len() {
                    let byte = self.input[self.offset];
                    self.offset += 1;
                    if byte == b'\n' || byte == b'\r' {
                        break;
                    }
                }
                continue;
            }
            break;
        }
        if self.offset >= self.input.len() {
            return Err(ImageError::UnexpectedEof {
                expected: self.offset + 1,
                actual: self.input.len(),
            });
        }
        Ok(())
    }

    fn consume_raster_separator(&mut self) -> Result<(), ImageError> {
        if self.offset >= self.input.len() {
            return Err(ImageError::UnexpectedEof {
                expected: self.offset + 1,
                actual: self.input.len(),
            });
        }
        if !self.input[self.offset].is_ascii_whitespace() {
            return Err(ImageError::InvalidHeader("PPM"));
        }
        self.offset += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_binary_ppm_with_comments() {
        let ppm = b"P6\n# generated\n2 1\n255\n\xff\x00\x00\x00\x80\xff";
        let image = decode(ppm).unwrap();
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 1);
        assert_eq!(image.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(image.pixels(), &[255, 0, 0, 0, 128, 255]);
    }

    #[test]
    fn decodes_ascii_ppm_with_comments_and_scaling() {
        let ppm = b"P3\n# generated\n2 1\n31\n0 15 31\n31 # mid-comment\n0 15";
        let image = decode(ppm).unwrap();
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 1);
        assert_eq!(image.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(image.pixels(), &[0, 123, 255, 255, 0, 123]);
    }

    #[test]
    fn encodes_deterministic_binary_ppm() {
        let image = Image::new(1, 1, PixelFormat::Rgb8, vec![1, 2, 3]).unwrap();
        assert_eq!(encode(&image).unwrap(), b"P6\n1 1\n255\n\x01\x02\x03");
    }

    #[test]
    fn rejects_high_maxval_for_first_slice() {
        assert_eq!(
            decode(b"P6\n1 1\n65535\n\0\0\0\0\0\0"),
            Err(ImageError::InvalidMaxValue { max_value: 65535 })
        );
    }
}
