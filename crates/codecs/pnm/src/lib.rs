//! PNM (Netpbm) decoding and encoding for the `imx` image toolkit.
//!
//! `imx-codec-pnm` reads and writes the Netpbm family: PBM bitmaps (`P1`/`P4`),
//! PGM grayscale (`P2`/`P5`), and PPM color (`P3`/`P6`), in both ASCII and
//! binary encodings. It produces and consumes the format-agnostic
//! [`imx_core::Image`] type shared across the workspace. Decoding is
//! memory-safe and deterministic: headers are parsed strictly and pixel buffers
//! are bounded by the `imx-core` allocation limits, so malformed or hostile
//! inputs cannot trigger uncontrolled allocation. Round-trips are
//! differentially verified against the real ImageMagick binary as an oracle.

use imx_core::{
    pixel_count, pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError, PixelFormat,
};

pub const P2_MAGIC: &[u8; 2] = b"P2";
pub const P3_MAGIC: &[u8; 2] = b"P3";
pub const P1_MAGIC: &[u8; 2] = b"P1";
pub const P4_MAGIC: &[u8; 2] = b"P4";
pub const P5_MAGIC: &[u8; 2] = b"P5";
pub const P6_MAGIC: &[u8; 2] = b"P6";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PnmHeader {
    pub encoding: PnmEncoding,
    pub width: u32,
    pub height: u32,
    pub max_value: u32,
    pub raster_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PnmEncoding {
    AsciiP1,
    AsciiP2,
    AsciiP3,
    BinaryP4,
    BinaryP5,
    BinaryP6,
}

impl PnmEncoding {
    fn is_pbm(self) -> bool {
        matches!(self, Self::AsciiP1 | Self::BinaryP4)
    }

    fn is_pgm(self) -> bool {
        matches!(self, Self::AsciiP2 | Self::BinaryP5)
    }

    fn is_ppm(self) -> bool {
        matches!(self, Self::AsciiP3 | Self::BinaryP6)
    }

    fn is_binary(self) -> bool {
        matches!(self, Self::BinaryP4 | Self::BinaryP5 | Self::BinaryP6)
    }

    fn samples_per_pixel(self) -> usize {
        match self {
            Self::AsciiP1 | Self::BinaryP4 => 1,
            Self::AsciiP2 | Self::BinaryP5 => 1,
            Self::AsciiP3 | Self::BinaryP6 => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SampleOutput {
    U8,
    U16Be,
}

pub fn identify_pbm(input: &[u8]) -> Result<Identify, ImageError> {
    let header = decode_pbm_header(input)?;
    Ok(Identify {
        format: Format::Pbm,
        width: header.width,
        height: header.height,
        pixel_format: PixelFormat::Bilevel,
    })
}

pub fn identify_ppm(input: &[u8]) -> Result<Identify, ImageError> {
    let header = decode_ppm_header(input)?;
    Ok(Identify {
        format: Format::Ppm,
        width: header.width,
        height: header.height,
        pixel_format: ppm_pixel_format(header.max_value),
    })
}

pub fn identify_pgm(input: &[u8]) -> Result<Identify, ImageError> {
    let header = decode_pgm_header(input)?;
    Ok(Identify {
        format: Format::Pgm,
        width: header.width,
        height: header.height,
        pixel_format: pgm_pixel_format(header.max_value),
    })
}

pub fn decode_pbm_header(input: &[u8]) -> Result<PnmHeader, ImageError> {
    let header = decode_header(input, "PBM")?;
    if !header.encoding.is_pbm() {
        return Err(ImageError::InvalidHeader("PBM"));
    }
    let _ = pixel_len(header.width, header.height, 1)?;
    Ok(header)
}

pub fn decode_ppm_header(input: &[u8]) -> Result<PnmHeader, ImageError> {
    let header = decode_header(input, "PPM")?;
    if !header.encoding.is_ppm() {
        return Err(ImageError::InvalidHeader("PPM"));
    }
    let bytes_per_pixel = ppm_pixel_format(header.max_value).bytes_per_pixel();
    let _ = pixel_len(header.width, header.height, bytes_per_pixel)?;
    Ok(header)
}

pub fn decode_pgm_header(input: &[u8]) -> Result<PnmHeader, ImageError> {
    let header = decode_header(input, "PGM")?;
    if !header.encoding.is_pgm() {
        return Err(ImageError::InvalidHeader("PGM"));
    }
    let bytes_per_pixel = pgm_pixel_format(header.max_value).bytes_per_pixel();
    let _ = pixel_len(header.width, header.height, bytes_per_pixel)?;
    Ok(header)
}

pub fn decode_header(input: &[u8], format_name: &'static str) -> Result<PnmHeader, ImageError> {
    let mut parser = Parser::new(input);
    let encoding = match parser.next_token()? {
        magic if magic == P1_MAGIC => PnmEncoding::AsciiP1,
        magic if magic == P2_MAGIC => PnmEncoding::AsciiP2,
        magic if magic == P3_MAGIC => PnmEncoding::AsciiP3,
        magic if magic == P4_MAGIC => PnmEncoding::BinaryP4,
        magic if magic == P5_MAGIC => PnmEncoding::BinaryP5,
        magic if magic == P6_MAGIC => PnmEncoding::BinaryP6,
        _ => return Err(ImageError::InvalidHeader(format_name)),
    };

    let width = parse_u32(parser.next_token()?, format_name)?;
    let height = parse_u32(parser.next_token()?, format_name)?;
    let max_value = if encoding.is_pbm() {
        1
    } else {
        parse_u32(parser.next_token()?, format_name)?
    };
    if !encoding.is_pbm() && (max_value == 0 || max_value > 65_535) {
        return Err(ImageError::InvalidMaxValue {
            format: format_name,
            max_value,
            max_supported: 65_535,
        });
    }
    if encoding.is_binary() {
        parser.consume_raster_separator(format_name)?;
    }

    Ok(PnmHeader {
        encoding,
        width,
        height,
        max_value,
        raster_offset: parser.offset(),
    })
}

pub fn decode_pbm(input: &[u8]) -> Result<Image, ImageError> {
    let header = decode_pbm_header(input)?;
    let sample_count = sample_count(header)?;
    let payload_len = pixel_len(header.width, header.height, 1)?;
    let pixels = match header.encoding {
        PnmEncoding::AsciiP1 => decode_ascii_pbm(input, header, sample_count, payload_len)?,
        PnmEncoding::BinaryP4 => decode_binary_pbm(input, header, payload_len)?,
        _ => return Err(ImageError::InvalidHeader("PBM")),
    };
    Image::new(header.width, header.height, PixelFormat::Bilevel, pixels)
}

pub fn decode_ppm(input: &[u8]) -> Result<Image, ImageError> {
    let header = decode_ppm_header(input)?;
    let sample_count = sample_count(header)?;
    let pixel_format = ppm_pixel_format(header.max_value);
    let payload_len = pixel_len(header.width, header.height, pixel_format.bytes_per_pixel())?;
    let output = if pixel_format == PixelFormat::Rgb8 {
        SampleOutput::U8
    } else {
        SampleOutput::U16Be
    };
    let pixels = decode_raster(input, header, sample_count, payload_len, output, "PPM")?;
    Image::new(header.width, header.height, pixel_format, pixels)
}

pub fn decode_pgm(input: &[u8]) -> Result<Image, ImageError> {
    let header = decode_pgm_header(input)?;
    let sample_count = sample_count(header)?;
    let pixel_format = pgm_pixel_format(header.max_value);
    let payload_len = pixel_len(header.width, header.height, pixel_format.bytes_per_pixel())?;
    let output = if pixel_format == PixelFormat::Gray8 {
        SampleOutput::U8
    } else {
        SampleOutput::U16Be
    };
    let pixels = decode_raster(input, header, sample_count, payload_len, output, "PGM")?;
    Image::new(header.width, header.height, pixel_format, pixels)
}

pub fn encode_pbm(image: &Image) -> Result<Vec<u8>, ImageError> {
    let bilevel = image.to_bilevel()?;
    let row_bytes = pbm_row_bytes(bilevel.width())?;
    let rows = usize::try_from(bilevel.height()).map_err(|_| ImageError::LengthOverflow)?;
    let payload_len = row_bytes
        .checked_mul(rows)
        .ok_or(ImageError::LengthOverflow)?;
    let header = format!("P4\n{} {}\n", bilevel.width(), bilevel.height());
    let capacity = header
        .len()
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = try_vec_with_capacity(capacity)?;
    out.extend_from_slice(header.as_bytes());
    out.resize(header.len() + payload_len, 0);

    let width = usize::try_from(bilevel.width()).map_err(|_| ImageError::LengthOverflow)?;
    for (index, pixel) in bilevel.pixels().iter().enumerate() {
        if *pixel < 128 {
            let y = index / width;
            let x = index % width;
            let byte_offset = header.len() + y * row_bytes + x / 8;
            out[byte_offset] |= 0x80 >> (x % 8);
        }
    }
    Ok(out)
}

pub fn encode_ppm(image: &Image) -> Result<Vec<u8>, ImageError> {
    match image.pixel_format() {
        PixelFormat::Gray16Be | PixelFormat::Rgb16Be | PixelFormat::Rgba16Be => {
            encode_ppm16(&image.to_rgb16be()?)
        }
        PixelFormat::Bilevel | PixelFormat::Gray8 | PixelFormat::Rgb8 | PixelFormat::Rgba8 => {
            encode_ppm8(&image.to_rgb8()?)
        }
    }
}

fn encode_ppm8(image: &Image) -> Result<Vec<u8>, ImageError> {
    let payload_len = pixel_len(image.width(), image.height(), 3)?;
    let header = format!("P6\n{} {}\n255\n", image.width(), image.height());
    let capacity = header
        .len()
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = try_vec_with_capacity(capacity)?;
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(image.pixels());
    Ok(out)
}

fn encode_ppm16(image: &Image) -> Result<Vec<u8>, ImageError> {
    let payload_len = pixel_len(image.width(), image.height(), 6)?;
    let header = format!("P6\n{} {}\n65535\n", image.width(), image.height());
    let capacity = header
        .len()
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = try_vec_with_capacity(capacity)?;
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(image.pixels());
    Ok(out)
}

pub fn encode_pgm(image: &Image) -> Result<Vec<u8>, ImageError> {
    match image.pixel_format() {
        PixelFormat::Gray16Be | PixelFormat::Rgb16Be | PixelFormat::Rgba16Be => {
            encode_pgm16(&image.to_gray16be()?)
        }
        PixelFormat::Bilevel | PixelFormat::Gray8 | PixelFormat::Rgb8 | PixelFormat::Rgba8 => {
            encode_pgm8(&image.to_gray8()?)
        }
    }
}

fn encode_pgm8(image: &Image) -> Result<Vec<u8>, ImageError> {
    let payload_len = pixel_len(image.width(), image.height(), 1)?;
    let header = format!("P5\n{} {}\n255\n", image.width(), image.height());
    let capacity = header
        .len()
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = try_vec_with_capacity(capacity)?;
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(image.pixels());
    Ok(out)
}

fn encode_pgm16(image: &Image) -> Result<Vec<u8>, ImageError> {
    let payload_len = pixel_len(image.width(), image.height(), 2)?;
    let header = format!("P5\n{} {}\n65535\n", image.width(), image.height());
    let capacity = header
        .len()
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = try_vec_with_capacity(capacity)?;
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(image.pixels());
    Ok(out)
}

fn decode_raster(
    input: &[u8],
    header: PnmHeader,
    sample_count: usize,
    payload_len: usize,
    output: SampleOutput,
    format_name: &'static str,
) -> Result<Vec<u8>, ImageError> {
    match header.encoding {
        PnmEncoding::AsciiP1 | PnmEncoding::BinaryP4 => Err(ImageError::InvalidHeader(format_name)),
        PnmEncoding::AsciiP2 | PnmEncoding::AsciiP3 => decode_ascii_raster(
            input,
            header,
            sample_count,
            payload_len,
            output,
            format_name,
        ),
        PnmEncoding::BinaryP5 | PnmEncoding::BinaryP6 => decode_binary_raster(
            input,
            header,
            sample_count,
            payload_len,
            output,
            format_name,
        ),
    }
}

fn decode_ascii_pbm(
    input: &[u8],
    header: PnmHeader,
    sample_count: usize,
    payload_len: usize,
) -> Result<Vec<u8>, ImageError> {
    let mut parser = Parser {
        input,
        offset: header.raster_offset,
    };
    let mut pixels = try_vec_with_capacity(payload_len)?;
    for index in 0..sample_count {
        let bit = parser.next_pbm_bit().map_err(|err| match err {
            ImageError::UnexpectedEof { actual, .. } => ImageError::UnexpectedEof {
                expected: header.raster_offset + index + 1,
                actual,
            },
            other => other,
        })?;
        match bit {
            b'0' => pixels.push(255),
            b'1' => pixels.push(0),
            _ => return Err(ImageError::InvalidPbmSample { byte: bit }),
        }
    }
    Ok(pixels)
}

fn decode_binary_pbm(
    input: &[u8],
    header: PnmHeader,
    payload_len: usize,
) -> Result<Vec<u8>, ImageError> {
    let row_bytes = pbm_row_bytes(header.width)?;
    let rows = usize::try_from(header.height).map_err(|_| ImageError::LengthOverflow)?;
    let file_payload_len = row_bytes
        .checked_mul(rows)
        .ok_or(ImageError::LengthOverflow)?;
    let expected_len = header
        .raster_offset
        .checked_add(file_payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    if input.len() < expected_len {
        return Err(ImageError::UnexpectedEof {
            expected: expected_len,
            actual: input.len(),
        });
    }

    let width = usize::try_from(header.width).map_err(|_| ImageError::LengthOverflow)?;
    let mut pixels = try_vec_with_capacity(payload_len)?;
    let raster = &input[header.raster_offset..expected_len];
    for row in raster.chunks_exact(row_bytes) {
        for x in 0..width {
            let byte = row[x / 8];
            let bit = (byte >> (7 - (x % 8))) & 1;
            pixels.push(if bit == 1 { 0 } else { 255 });
        }
    }
    Ok(pixels)
}

fn parse_u32(token: &[u8], format_name: &'static str) -> Result<u32, ImageError> {
    if token.is_empty() || !token.iter().all(u8::is_ascii_digit) {
        return Err(ImageError::InvalidHeader(format_name));
    }
    let text = std::str::from_utf8(token).map_err(|_| ImageError::InvalidHeader(format_name))?;
    text.parse::<u32>()
        .map_err(|_| ImageError::InvalidHeader(format_name))
}

fn sample_count(header: PnmHeader) -> Result<usize, ImageError> {
    pixel_count(header.width, header.height)?
        .checked_mul(header.encoding.samples_per_pixel())
        .ok_or(ImageError::LengthOverflow)
}

fn pbm_row_bytes(width: u32) -> Result<usize, ImageError> {
    let width = usize::try_from(width).map_err(|_| ImageError::LengthOverflow)?;
    width
        .checked_add(7)
        .ok_or(ImageError::LengthOverflow)
        .map(|rounded| rounded / 8)
}

fn decode_ascii_raster(
    input: &[u8],
    header: PnmHeader,
    sample_count: usize,
    payload_len: usize,
    output: SampleOutput,
    format_name: &'static str,
) -> Result<Vec<u8>, ImageError> {
    let mut parser = Parser {
        input,
        offset: header.raster_offset,
    };
    let mut pixels = try_vec_with_capacity(payload_len)?;
    for index in 0..sample_count {
        let token = parser.next_token().map_err(|err| match err {
            ImageError::UnexpectedEof { actual, .. } => ImageError::UnexpectedEof {
                expected: header.raster_offset + index + 1,
                actual,
            },
            other => other,
        })?;
        let sample = parse_u32(token, format_name)?;
        push_scaled_sample(&mut pixels, sample, header.max_value, output, format_name)?;
    }
    Ok(pixels)
}

fn decode_binary_raster(
    input: &[u8],
    header: PnmHeader,
    sample_count: usize,
    payload_len: usize,
    output: SampleOutput,
    format_name: &'static str,
) -> Result<Vec<u8>, ImageError> {
    let file_bytes_per_sample = if header.max_value <= 255 { 1 } else { 2 };
    let file_payload_len = sample_count
        .checked_mul(file_bytes_per_sample)
        .ok_or(ImageError::LengthOverflow)?;
    let expected_len = header
        .raster_offset
        .checked_add(file_payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    if input.len() < expected_len {
        return Err(ImageError::UnexpectedEof {
            expected: expected_len,
            actual: input.len(),
        });
    }

    let mut pixels = try_vec_with_capacity(payload_len)?;
    let raster = &input[header.raster_offset..expected_len];
    if file_bytes_per_sample == 1 {
        for sample in raster {
            push_scaled_sample(
                &mut pixels,
                u32::from(*sample),
                header.max_value,
                output,
                format_name,
            )?;
        }
    } else {
        for sample in raster.chunks_exact(2) {
            push_scaled_sample(
                &mut pixels,
                u32::from(u16::from_be_bytes([sample[0], sample[1]])),
                header.max_value,
                output,
                format_name,
            )?;
        }
    }
    Ok(pixels)
}

fn push_scaled_sample(
    out: &mut Vec<u8>,
    sample: u32,
    max_value: u32,
    output: SampleOutput,
    format_name: &'static str,
) -> Result<(), ImageError> {
    if sample > max_value {
        return Err(ImageError::InvalidSampleValue {
            format: format_name,
            sample_value: sample,
            max_value,
        });
    }
    match output {
        SampleOutput::U8 => out.push(scale_sample_to_u8(sample, max_value)),
        SampleOutput::U16Be => {
            out.extend_from_slice(&scale_sample_to_u16(sample, max_value).to_be_bytes());
        }
    }
    Ok(())
}

fn scale_sample_to_u8(sample: u32, max_value: u32) -> u8 {
    ((sample * 255 + (max_value / 2)) / max_value) as u8
}

fn scale_sample_to_u16(sample: u32, max_value: u32) -> u16 {
    ((sample * 65_535 + (max_value / 2)) / max_value) as u16
}

fn pgm_pixel_format(max_value: u32) -> PixelFormat {
    if max_value <= 255 {
        PixelFormat::Gray8
    } else {
        PixelFormat::Gray16Be
    }
}

fn ppm_pixel_format(max_value: u32) -> PixelFormat {
    if max_value <= 255 {
        PixelFormat::Rgb8
    } else {
        PixelFormat::Rgb16Be
    }
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

    fn next_pbm_bit(&mut self) -> Result<u8, ImageError> {
        self.skip_whitespace_and_comments()?;
        let bit = self.input[self.offset];
        if bit != b'0' && bit != b'1' {
            return Err(ImageError::InvalidPbmSample { byte: bit });
        }
        self.offset += 1;
        Ok(bit)
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

    fn consume_raster_separator(&mut self, format_name: &'static str) -> Result<(), ImageError> {
        if self.offset >= self.input.len() {
            return Err(ImageError::UnexpectedEof {
                expected: self.offset + 1,
                actual: self.input.len(),
            });
        }
        while self.offset < self.input.len() && self.input[self.offset] == b'#' {
            self.offset += 1;
            while self.offset < self.input.len() {
                let byte = self.input[self.offset];
                if byte == b'\n' || byte == b'\r' {
                    break;
                }
                self.offset += 1;
            }
        }
        if self.offset >= self.input.len() {
            return Err(ImageError::UnexpectedEof {
                expected: self.offset + 1,
                actual: self.input.len(),
            });
        }
        if !self.input[self.offset].is_ascii_whitespace() {
            return Err(ImageError::InvalidHeader(format_name));
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
        let image = decode_ppm(ppm).unwrap();
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 1);
        assert_eq!(image.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(image.pixels(), &[255, 0, 0, 0, 128, 255]);
    }

    #[test]
    fn decodes_ascii_ppm_with_comments_and_scaling() {
        let ppm = b"P3\n# generated\n2 1\n31\n0 15 31\n31 # mid-comment\n0 15";
        let image = decode_ppm(ppm).unwrap();
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 1);
        assert_eq!(image.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(image.pixels(), &[0, 123, 255, 255, 0, 123]);
    }

    #[test]
    fn decodes_sixteen_bit_binary_ppm() {
        let ppm = b"P6\n2 1\n65535\n\x12\x34\x80\x00\xff\xff\x00\x00\x40\x00\x7f\xff";
        let image = decode_ppm(ppm).unwrap();
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 1);
        assert_eq!(image.pixel_format(), PixelFormat::Rgb16Be);
        assert_eq!(
            image.pixels(),
            &[0x12, 0x34, 0x80, 0x00, 0xff, 0xff, 0x00, 0x00, 0x40, 0x00, 0x7f, 0xff]
        );
    }

    #[test]
    fn decodes_sixteen_bit_ascii_ppm_with_scaling() {
        let ppm = b"P3\n2 1\n1023\n0 512 1023\n1023 256 128";
        let image = decode_ppm(ppm).unwrap();
        assert_eq!(image.pixel_format(), PixelFormat::Rgb16Be);
        assert_eq!(
            image.pixels(),
            &[0, 0, 0x80, 0x20, 0xff, 0xff, 0xff, 0xff, 0x40, 0x10, 0x20, 0x08]
        );
    }

    #[test]
    fn decodes_ascii_pbm_with_comments() {
        let pbm = b"P1\n# generated\n4 2\n0110\n1 # black\n0 0 1\n";
        let image = decode_pbm(pbm).unwrap();
        assert_eq!(image.width(), 4);
        assert_eq!(image.height(), 2);
        assert_eq!(image.pixel_format(), PixelFormat::Bilevel);
        assert_eq!(image.pixels(), &[255, 0, 0, 255, 0, 255, 255, 0]);
    }

    #[test]
    fn decodes_binary_pbm_msb_first_with_row_padding() {
        let pbm = b"P4\n9 2\n\xaa\x80\x55\x00";
        let image = decode_pbm(pbm).unwrap();
        assert_eq!(image.width(), 9);
        assert_eq!(image.height(), 2);
        assert_eq!(image.pixel_format(), PixelFormat::Bilevel);
        assert_eq!(
            image.pixels(),
            &[0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255]
        );
    }

    #[test]
    fn decodes_binary_pbm_with_comment_attached_to_dimensions() {
        let pbm = b"P4\n4 1# attached comment\n\x90";
        let image = decode_pbm(pbm).unwrap();
        assert_eq!(image.pixels(), &[0, 255, 255, 0]);
    }

    #[test]
    fn decodes_binary_pgm_with_comments() {
        let pgm = b"P5\n# generated\n3 1\n255\n\x00\x80\xff";
        let image = decode_pgm(pgm).unwrap();
        assert_eq!(image.width(), 3);
        assert_eq!(image.height(), 1);
        assert_eq!(image.pixel_format(), PixelFormat::Gray8);
        assert_eq!(image.pixels(), &[0, 128, 255]);
    }

    #[test]
    fn decodes_ascii_pgm_with_comments_and_scaling() {
        let pgm = b"P2\n# generated\n3 1\n31\n0 15 31";
        let image = decode_pgm(pgm).unwrap();
        assert_eq!(image.width(), 3);
        assert_eq!(image.height(), 1);
        assert_eq!(image.pixel_format(), PixelFormat::Gray8);
        assert_eq!(image.pixels(), &[0, 123, 255]);
    }

    #[test]
    fn decodes_sixteen_bit_binary_pgm() {
        let pgm = b"P5\n2 1\n65535\n\x12\x34\xff\xff";
        let image = decode_pgm(pgm).unwrap();
        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 1);
        assert_eq!(image.pixel_format(), PixelFormat::Gray16Be);
        assert_eq!(image.pixels(), &[0x12, 0x34, 0xff, 0xff]);
    }

    #[test]
    fn decodes_sixteen_bit_ascii_pgm_with_scaling() {
        let pgm = b"P2\n2 1\n1023\n0 1023";
        let image = decode_pgm(pgm).unwrap();
        assert_eq!(image.pixel_format(), PixelFormat::Gray16Be);
        assert_eq!(image.pixels(), &[0, 0, 0xff, 0xff]);
    }

    #[test]
    fn encodes_deterministic_binary_ppm() {
        let image = Image::new(1, 1, PixelFormat::Rgb8, vec![1, 2, 3]).unwrap();
        assert_eq!(encode_ppm(&image).unwrap(), b"P6\n1 1\n255\n\x01\x02\x03");
    }

    #[test]
    fn encodes_deterministic_sixteen_bit_binary_ppm() {
        let image = Image::new(
            1,
            2,
            PixelFormat::Rgba16Be,
            vec![
                0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x00, 0x00, 0x80, 0x00, 0xff, 0xff,
                0xff, 0xff,
            ],
        )
        .unwrap();
        assert_eq!(
            encode_ppm(&image).unwrap(),
            b"P6\n1 2\n65535\n\x12\x34\x56\x78\x9a\xbc\x00\x00\x80\x00\xff\xff"
        );
    }

    #[test]
    fn encodes_deterministic_binary_pbm_from_gray8() {
        let image = Image::new(4, 1, PixelFormat::Gray8, vec![0, 127, 128, 255]).unwrap();
        assert_eq!(encode_pbm(&image).unwrap(), b"P4\n4 1\n\xc0");
    }

    #[test]
    fn encodes_deterministic_binary_pbm_from_rgba16be() {
        let image = Image::new(
            2,
            1,
            PixelFormat::Rgba16Be,
            vec![
                0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00,
                0xff, 0xff,
            ],
        )
        .unwrap();
        assert_eq!(encode_pbm(&image).unwrap(), b"P4\n2 1\n\x80");
    }

    #[test]
    fn encodes_deterministic_binary_pgm_from_rgb8() {
        let image = Image::new(
            4,
            1,
            PixelFormat::Rgb8,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
        )
        .unwrap();
        assert_eq!(
            encode_pgm(&image).unwrap(),
            b"P5\n4 1\n255\n\x36\xb6\x12\xff"
        );
    }

    #[test]
    fn encodes_deterministic_binary_pgm_from_rgba16be() {
        let image = Image::new(
            1,
            1,
            PixelFormat::Rgba16Be,
            vec![0x80, 0x00, 0x40, 0x00, 0x20, 0x00, 0xff, 0xff],
        )
        .unwrap();
        assert_eq!(encode_pgm(&image).unwrap(), b"P5\n1 1\n65535\n\x4b\x4d");
    }

    #[test]
    fn rejects_out_of_range_ppm_maxval_and_sample() {
        assert_eq!(
            decode_ppm(b"P6\n1 1\n65536\n\0\0\0\0\0\0"),
            Err(ImageError::InvalidMaxValue {
                format: "PPM",
                max_value: 65536,
                max_supported: 65535,
            })
        );
        assert_eq!(
            decode_ppm(b"P3\n1 1\n1023\n0 1024 1\n"),
            Err(ImageError::InvalidSampleValue {
                format: "PPM",
                sample_value: 1024,
                max_value: 1023,
            })
        );
        assert!(matches!(
            decode_ppm(b"P6\n1 1\n65535\n\x12"),
            Err(ImageError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn rejects_out_of_range_pgm_maxval() {
        assert_eq!(
            decode_pgm(b"P5\n1 1\n65536\n\0\0"),
            Err(ImageError::InvalidMaxValue {
                format: "PGM",
                max_value: 65536,
                max_supported: 65535,
            })
        );
    }
}
