use std::io::Cursor;

use imx_core::{
    pixel_count, pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError,
    PixelFormat, MAX_PIXEL_BYTES,
};
use jpeg_decoder::{Decoder, PixelFormat as JpegPixelFormat};
use jpeg_encoder::{ColorType, Encoder, EncodingError};

pub const MAGIC: &[u8; 3] = b"\xff\xd8\xff";
pub const DEFAULT_QUALITY: u8 = 90;
pub const MAX_JPEG_DECODE_BYTES: usize = MAX_PIXEL_BYTES / 4;

pub fn identify(input: &[u8]) -> Result<Identify, ImageError> {
    let orientation = exif_orientation(input)?;
    let mut decoder = decoder(input)?;
    let (width, height, pixel_format) = checked_info(&mut decoder, "identify")?;
    let (width, height) = orientation.dimensions(width, height);
    Ok(Identify {
        format: Format::Jpeg,
        width,
        height,
        pixel_format,
    })
}

pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    let orientation = exif_orientation(input)?;
    let mut decoder = decoder(input)?;
    let (width, height, pixel_format) = checked_info(&mut decoder, "decode")?;
    let pixels = decoder
        .decode()
        .map_err(|err| jpeg_decode_error("decode", err))?;
    let image = Image::new(width, height, pixel_format, pixels)?;
    orient_image(image, orientation)
}

pub fn encode(image: &Image) -> Result<Vec<u8>, ImageError> {
    encode_with_quality(image, DEFAULT_QUALITY)
}

pub fn encode_with_quality(image: &Image, quality: u8) -> Result<Vec<u8>, ImageError> {
    if !(1..=100).contains(&quality) {
        return Err(ImageError::UnsupportedFormat(format!(
            "JPEG quality must be between 1 and 100, got {quality}"
        )));
    }
    let (encoded, color_type) = encode_source(image)?;
    let width = u16::try_from(encoded.width()).map_err(|_| {
        ImageError::UnsupportedFormat("JPEG dimensions exceed 65535 pixels".to_string())
    })?;
    let height = u16::try_from(encoded.height()).map_err(|_| {
        ImageError::UnsupportedFormat("JPEG dimensions exceed 65535 pixels".to_string())
    })?;

    let mut out = Vec::new();
    Encoder::new(&mut out, quality)
        .encode(encoded.pixels(), width, height, color_type)
        .map_err(jpeg_encode_error)?;
    Ok(out)
}

fn decoder(input: &[u8]) -> Result<Decoder<Cursor<&[u8]>>, ImageError> {
    if input.len() < MAGIC.len() {
        return Err(ImageError::UnexpectedEof {
            expected: MAGIC.len(),
            actual: input.len(),
        });
    }
    if &input[..MAGIC.len()] != MAGIC {
        return Err(ImageError::InvalidHeader("JPEG"));
    }

    let mut decoder = Decoder::new(Cursor::new(input));
    decoder.set_max_decoding_buffer_size(MAX_JPEG_DECODE_BYTES);
    Ok(decoder)
}

fn checked_info(
    decoder: &mut Decoder<Cursor<&[u8]>>,
    operation: &'static str,
) -> Result<(u32, u32, PixelFormat), ImageError> {
    decoder
        .read_info()
        .map_err(|err| jpeg_decode_error(operation, err))?;
    let info = decoder.info().ok_or_else(|| {
        ImageError::UnsupportedFormat(format!("JPEG {operation} failed: missing image info"))
    })?;
    let width = u32::from(info.width);
    let height = u32::from(info.height);
    let pixel_format = supported_pixel_format(info.pixel_format)?;
    let _ = jpeg_pixel_len(width, height, pixel_format.bytes_per_pixel())?;
    Ok((width, height, pixel_format))
}

fn jpeg_pixel_len(width: u32, height: u32, bytes_per_pixel: usize) -> Result<usize, ImageError> {
    let bytes = pixel_count(width, height)?
        .checked_mul(bytes_per_pixel)
        .ok_or(ImageError::LengthOverflow)?;
    if bytes > MAX_JPEG_DECODE_BYTES {
        return Err(ImageError::ImageTooLarge {
            required: bytes,
            limit: MAX_JPEG_DECODE_BYTES,
        });
    }
    Ok(bytes)
}

fn supported_pixel_format(pixel_format: JpegPixelFormat) -> Result<PixelFormat, ImageError> {
    match pixel_format {
        JpegPixelFormat::L8 => Ok(PixelFormat::Gray8),
        JpegPixelFormat::RGB24 => Ok(PixelFormat::Rgb8),
        JpegPixelFormat::L16 => Err(ImageError::UnsupportedFormat(
            "JPEG 16-bit samples are not supported".to_string(),
        )),
        JpegPixelFormat::CMYK32 => Err(ImageError::UnsupportedFormat(
            "JPEG CMYK is not supported".to_string(),
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orientation {
    Normal,
    MirrorHorizontal,
    Rotate180,
    MirrorVertical,
    Transpose,
    Rotate90,
    Transverse,
    Rotate270,
}

impl Orientation {
    fn from_exif(value: u16) -> Result<Self, ImageError> {
        match value {
            1 => Ok(Self::Normal),
            2 => Ok(Self::MirrorHorizontal),
            3 => Ok(Self::Rotate180),
            4 => Ok(Self::MirrorVertical),
            5 => Ok(Self::Transpose),
            6 => Ok(Self::Rotate90),
            7 => Ok(Self::Transverse),
            8 => Ok(Self::Rotate270),
            _ => Err(ImageError::UnsupportedFormat(format!(
                "JPEG EXIF Orientation value {value} is not supported"
            ))),
        }
    }

    fn dimensions(self, width: u32, height: u32) -> (u32, u32) {
        match self {
            Self::Transpose | Self::Rotate90 | Self::Transverse | Self::Rotate270 => {
                (height, width)
            }
            Self::Normal | Self::MirrorHorizontal | Self::Rotate180 | Self::MirrorVertical => {
                (width, height)
            }
        }
    }

    fn target(self, x: usize, y: usize, width: usize, height: usize) -> (usize, usize) {
        match self {
            Self::Normal => (x, y),
            Self::MirrorHorizontal => (width - 1 - x, y),
            Self::Rotate180 => (width - 1 - x, height - 1 - y),
            Self::MirrorVertical => (x, height - 1 - y),
            Self::Transpose => (y, x),
            Self::Rotate90 => (height - 1 - y, x),
            Self::Transverse => (height - 1 - y, width - 1 - x),
            Self::Rotate270 => (y, width - 1 - x),
        }
    }
}

fn orient_image(image: Image, orientation: Orientation) -> Result<Image, ImageError> {
    if orientation == Orientation::Normal {
        return Ok(image);
    }

    let width = usize::try_from(image.width()).map_err(|_| ImageError::LengthOverflow)?;
    let height = usize::try_from(image.height()).map_err(|_| ImageError::LengthOverflow)?;
    let bpp = image.pixel_format().bytes_per_pixel();
    let (out_width, out_height) = orientation.dimensions(image.width(), image.height());
    let out_width_usize = usize::try_from(out_width).map_err(|_| ImageError::LengthOverflow)?;
    let out_len = pixel_len(out_width, out_height, bpp)?;
    let mut out = try_vec_with_capacity(out_len)?;
    out.resize(out_len, 0);

    for y in 0..height {
        for x in 0..width {
            let source = (y * width + x) * bpp;
            let (out_x, out_y) = orientation.target(x, y, width, height);
            let target = (out_y * out_width_usize + out_x) * bpp;
            out[target..target + bpp].copy_from_slice(&image.pixels()[source..source + bpp]);
        }
    }

    Image::new(out_width, out_height, image.pixel_format(), out)
}

fn exif_orientation(input: &[u8]) -> Result<Orientation, ImageError> {
    if input.len() < 2 || &input[..2] != b"\xff\xd8" {
        return Ok(Orientation::Normal);
    }

    let mut offset = 2;
    while offset < input.len() {
        if input[offset] != 0xff {
            break;
        }
        while offset < input.len() && input[offset] == 0xff {
            offset += 1;
        }
        if offset >= input.len() {
            break;
        }
        let marker = input[offset];
        offset += 1;

        if marker == 0xda || marker == 0xd9 {
            break;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        if offset + 2 > input.len() {
            break;
        }

        let length = usize::from(u16::from_be_bytes([input[offset], input[offset + 1]]));
        if length < 2 {
            break;
        }
        let data_start = offset + 2;
        let data_end = data_start
            .checked_add(length - 2)
            .ok_or(ImageError::LengthOverflow)?;
        if data_end > input.len() {
            if marker == 0xe1 && input[data_start..].starts_with(b"Exif\0\0") {
                return Err(malformed_exif("APP1 segment is truncated"));
            }
            break;
        }

        let data = &input[data_start..data_end];
        if marker == 0xe1 && data.starts_with(b"Exif\0\0") {
            if let Some(orientation) = parse_exif_orientation(&data[6..])? {
                return Ok(orientation);
            }
        }
        offset = data_end;
    }

    Ok(Orientation::Normal)
}

#[derive(Debug, Clone, Copy)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn u16(self, bytes: &[u8]) -> u16 {
        match self {
            Self::Little => u16::from_le_bytes([bytes[0], bytes[1]]),
            Self::Big => u16::from_be_bytes([bytes[0], bytes[1]]),
        }
    }

    fn u32(self, bytes: &[u8]) -> u32 {
        match self {
            Self::Little => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            Self::Big => u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        }
    }
}

fn parse_exif_orientation(tiff: &[u8]) -> Result<Option<Orientation>, ImageError> {
    if tiff.len() < 8 {
        return Err(malformed_exif("TIFF header is truncated"));
    }
    let endian = match &tiff[..2] {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return Err(malformed_exif("TIFF byte order is invalid")),
    };
    if endian.u16(&tiff[2..4]) != 42 {
        return Err(malformed_exif("TIFF magic is invalid"));
    }
    let ifd_offset =
        usize::try_from(endian.u32(&tiff[4..8])).map_err(|_| ImageError::LengthOverflow)?;
    let entry_count_end = ifd_offset
        .checked_add(2)
        .ok_or(ImageError::LengthOverflow)?;
    if entry_count_end > tiff.len() {
        return Err(malformed_exif("IFD0 offset is outside the EXIF payload"));
    }

    let entry_count = usize::from(endian.u16(&tiff[ifd_offset..entry_count_end]));
    let entries_start = entry_count_end;
    let entries_len = entry_count
        .checked_mul(12)
        .ok_or(ImageError::LengthOverflow)?;
    let entries_end = entries_start
        .checked_add(entries_len)
        .ok_or(ImageError::LengthOverflow)?;
    if entries_end > tiff.len() {
        return Err(malformed_exif("IFD0 entries are truncated"));
    }

    for entry in tiff[entries_start..entries_end].chunks_exact(12) {
        let tag = endian.u16(&entry[0..2]);
        if tag != 0x0112 {
            continue;
        }
        let field_type = endian.u16(&entry[2..4]);
        let count = endian.u32(&entry[4..8]);
        if field_type != 3 || count != 1 {
            return Err(malformed_exif(
                "Orientation tag has unsupported type or count",
            ));
        }
        let value = endian.u16(&entry[8..10]);
        return Orientation::from_exif(value).map(Some);
    }

    Ok(None)
}

fn malformed_exif(reason: &'static str) -> ImageError {
    ImageError::UnsupportedFormat(format!(
        "JPEG EXIF Orientation metadata is malformed: {reason}"
    ))
}

fn encode_source(image: &Image) -> Result<(Image, ColorType), ImageError> {
    match image.pixel_format() {
        PixelFormat::Bilevel | PixelFormat::Gray8 | PixelFormat::Gray16Be => {
            Ok((image.to_gray8()?, ColorType::Luma))
        }
        PixelFormat::Rgb8 | PixelFormat::Rgb16Be => Ok((image.to_rgb8()?, ColorType::Rgb)),
        PixelFormat::Rgba8 => {
            reject_non_opaque_rgba8(image.pixels())?;
            Ok((image.to_rgb8()?, ColorType::Rgb))
        }
        PixelFormat::Rgba16Be => {
            reject_non_opaque_rgba16be(image.pixels())?;
            Ok((image.to_rgb8()?, ColorType::Rgb))
        }
    }
}

fn reject_non_opaque_rgba8(pixels: &[u8]) -> Result<(), ImageError> {
    if pixels.chunks_exact(4).all(|px| px[3] == 0xff) {
        return Ok(());
    }
    Err(ImageError::UnsupportedFormat(
        "JPEG encoding requires fully opaque input; alpha is not supported".to_string(),
    ))
}

fn reject_non_opaque_rgba16be(pixels: &[u8]) -> Result<(), ImageError> {
    if pixels
        .chunks_exact(8)
        .all(|px| px[6] == 0xff && px[7] == 0xff)
    {
        return Ok(());
    }
    Err(ImageError::UnsupportedFormat(
        "JPEG encoding requires fully opaque input; alpha is not supported".to_string(),
    ))
}

fn jpeg_decode_error(operation: &'static str, err: jpeg_decoder::Error) -> ImageError {
    ImageError::UnsupportedFormat(format!("JPEG {operation} failed: {err}"))
}

fn jpeg_encode_error(err: EncodingError) -> ImageError {
    ImageError::UnsupportedFormat(format!("JPEG encode failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    mod progressive_jpeg_fixtures {
        include!("../../../cli/src/progressive_jpeg_fixtures.rs");
    }

    fn jpeg_with_exif_app1(jpeg: &[u8], app1_data: &[u8]) -> Vec<u8> {
        let segment_len = u16::try_from(app1_data.len() + 2).unwrap();
        let mut out = Vec::new();
        out.extend_from_slice(&jpeg[..2]);
        out.extend_from_slice(&[0xff, 0xe1]);
        out.extend_from_slice(&segment_len.to_be_bytes());
        out.extend_from_slice(app1_data);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    fn jpeg_with_exif_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
        let mut app1 = Vec::from(b"Exif\0\0MM\0*\0\0\0\x08".as_slice());
        app1.extend_from_slice(&1_u16.to_be_bytes());
        app1.extend_from_slice(&0x0112_u16.to_be_bytes());
        app1.extend_from_slice(&3_u16.to_be_bytes());
        app1.extend_from_slice(&1_u32.to_be_bytes());
        app1.extend_from_slice(&orientation.to_be_bytes());
        app1.extend_from_slice(&[0, 0]);
        app1.extend_from_slice(&0_u32.to_be_bytes());
        jpeg_with_exif_app1(jpeg, &app1)
    }

    fn expected_oriented(image: &Image, orientation: u16) -> Image {
        let width = image.width() as usize;
        let height = image.height() as usize;
        let bpp = image.pixel_format().bytes_per_pixel();
        let (out_width, out_height) = match orientation {
            5..=8 => (height, width),
            _ => (width, height),
        };
        let mut out = vec![0; out_width * out_height * bpp];
        for y in 0..height {
            for x in 0..width {
                let (out_x, out_y) = match orientation {
                    1 => (x, y),
                    2 => (width - 1 - x, y),
                    3 => (width - 1 - x, height - 1 - y),
                    4 => (x, height - 1 - y),
                    5 => (y, x),
                    6 => (height - 1 - y, x),
                    7 => (height - 1 - y, width - 1 - x),
                    8 => (y, width - 1 - x),
                    _ => unreachable!(),
                };
                let source = (y * width + x) * bpp;
                let target = (out_y * out_width + out_x) * bpp;
                out[target..target + bpp].copy_from_slice(&image.pixels()[source..source + bpp]);
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
    fn encodes_and_decodes_rgb8_jpeg() {
        let image = Image::new(
            8,
            8,
            PixelFormat::Rgb8,
            (0..8)
                .flat_map(|y| {
                    (0..8).flat_map(move |x| {
                        [
                            (x * 31 + y * 3) as u8,
                            (x * 5 + y * 29) as u8,
                            (x * 17 + y * 11) as u8,
                        ]
                    })
                })
                .collect(),
        )
        .unwrap();
        let jpeg = encode(&image).unwrap();
        assert_eq!(&jpeg[..MAGIC.len()], MAGIC);
        assert_eq!(
            identify(&jpeg).unwrap().stable_line(),
            "format=JPEG width=8 height=8 channels=RGB depth=8"
        );
        let decoded = decode(&jpeg).unwrap();
        assert_eq!(decoded.width(), 8);
        assert_eq!(decoded.height(), 8);
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgb8);
    }

    #[test]
    fn encodes_and_decodes_gray8_jpeg() {
        let image = Image::new(4, 1, PixelFormat::Gray8, vec![0, 85, 170, 255]).unwrap();
        let jpeg = encode(&image).unwrap();
        assert_eq!(
            identify(&jpeg).unwrap().stable_line(),
            "format=JPEG width=4 height=1 channels=GRAY depth=8"
        );
        let decoded = decode(&jpeg).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Gray8);
    }

    #[test]
    fn identifies_and_decodes_progressive_rgb_and_gray_jpegs() {
        let rgb = progressive_jpeg_fixtures::progressive_rgb_jpeg();
        let gray = progressive_jpeg_fixtures::progressive_gray_jpeg();
        assert!(progressive_jpeg_fixtures::is_progressive_jpeg(&rgb));
        assert!(progressive_jpeg_fixtures::is_progressive_jpeg(&gray));

        assert_eq!(
            identify(&rgb).unwrap().stable_line(),
            "format=JPEG width=4 height=3 channels=RGB depth=8"
        );
        let rgb_image = decode(&rgb).unwrap();
        assert_eq!(rgb_image.width(), 4);
        assert_eq!(rgb_image.height(), 3);
        assert_eq!(rgb_image.pixel_format(), PixelFormat::Rgb8);

        assert_eq!(
            identify(&gray).unwrap().stable_line(),
            "format=JPEG width=4 height=2 channels=GRAY depth=8"
        );
        let gray_image = decode(&gray).unwrap();
        assert_eq!(gray_image.width(), 4);
        assert_eq!(gray_image.height(), 2);
        assert_eq!(gray_image.pixel_format(), PixelFormat::Gray8);
    }

    #[test]
    fn progressive_jpeg_exif_orientation_still_normalizes_pixels() {
        let rgb = progressive_jpeg_fixtures::progressive_rgb_jpeg();
        let baseline = decode(&rgb).unwrap();
        let oriented = jpeg_with_exif_orientation(&rgb, 6);

        assert_eq!(
            identify(&oriented).unwrap().stable_line(),
            "format=JPEG width=3 height=4 channels=RGB depth=8"
        );
        assert_eq!(decode(&oriented).unwrap(), expected_oriented(&baseline, 6));
    }

    #[test]
    fn rejects_non_opaque_alpha_on_encode() {
        let image = Image::new(
            1,
            2,
            PixelFormat::Rgba8,
            vec![255, 0, 0, 255, 0, 0, 255, 128],
        )
        .unwrap();
        assert!(encode(&image)
            .unwrap_err()
            .to_string()
            .contains("alpha is not supported"));
    }

    #[test]
    fn decodes_exif_orientation_values_to_normalized_pixels() {
        let image = Image::new(
            3,
            2,
            PixelFormat::Rgb8,
            vec![
                10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160, 170, 180,
            ],
        )
        .unwrap();
        let jpeg = encode(&image).unwrap();
        let baseline = decode(&jpeg).unwrap();

        for orientation in 1..=8 {
            let oriented = decode(&jpeg_with_exif_orientation(&jpeg, orientation)).unwrap();
            let expected = expected_oriented(&baseline, orientation);
            assert_eq!(
                oriented, expected,
                "EXIF Orientation {orientation} did not normalize pixels as expected"
            );
        }
    }

    #[test]
    fn identify_reports_exif_oriented_dimensions() {
        let image = Image::new(3, 2, PixelFormat::Rgb8, vec![0x80; 3 * 2 * 3]).unwrap();
        let jpeg = encode(&image).unwrap();
        assert_eq!(
            identify(&jpeg_with_exif_orientation(&jpeg, 6))
                .unwrap()
                .stable_line(),
            "format=JPEG width=2 height=3 channels=RGB depth=8"
        );
        assert_eq!(
            identify(&jpeg_with_exif_orientation(&jpeg, 3))
                .unwrap()
                .stable_line(),
            "format=JPEG width=3 height=2 channels=RGB depth=8"
        );
    }

    #[test]
    fn rejects_invalid_exif_orientation_metadata() {
        let image = Image::new(2, 2, PixelFormat::Rgb8, vec![0x80; 2 * 2 * 3]).unwrap();
        let jpeg = encode(&image).unwrap();

        let err = identify(&jpeg_with_exif_orientation(&jpeg, 9))
            .unwrap_err()
            .to_string();
        assert!(err.contains("JPEG EXIF Orientation value 9 is not supported"));

        let malformed = jpeg_with_exif_app1(&jpeg, b"Exif\0\0ZZ\0*\0\0\0\x08");
        let err = decode(&malformed).unwrap_err().to_string();
        assert!(err.contains("JPEG EXIF Orientation metadata is malformed"));
    }

    #[test]
    fn encode_with_quality_default_matches_encode() {
        let image = Image::new(
            8,
            8,
            PixelFormat::Rgb8,
            (0..8)
                .flat_map(|y| {
                    (0..8).flat_map(move |x| {
                        [
                            (x * 31 + y * 3) as u8,
                            (x * 5 + y * 29) as u8,
                            (x * 17 + y * 11) as u8,
                        ]
                    })
                })
                .collect(),
        )
        .unwrap();
        assert_eq!(
            encode(&image).unwrap(),
            encode_with_quality(&image, DEFAULT_QUALITY).unwrap()
        );
    }

    #[test]
    fn encode_with_quality_rejects_out_of_range() {
        let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap();
        for quality in [0u8, 101, 255] {
            let err = encode_with_quality(&image, quality)
                .unwrap_err()
                .to_string();
            assert!(
                err.contains("JPEG quality must be between 1 and 100"),
                "quality {quality} gave unexpected error: {err}"
            );
        }
        assert!(encode_with_quality(&image, 1).is_ok());
        assert!(encode_with_quality(&image, 100).is_ok());
    }

    #[test]
    fn encode_with_quality_changes_output_size() {
        let image = Image::new(
            16,
            16,
            PixelFormat::Rgb8,
            (0..16)
                .flat_map(|y| {
                    (0..16).flat_map(move |x| {
                        [
                            (x * 13 + y * 7) as u8,
                            (x * 3 + y * 19) as u8,
                            (x * 23 + y * 5) as u8,
                        ]
                    })
                })
                .collect(),
        )
        .unwrap();
        let low = encode_with_quality(&image, 20).unwrap();
        let high = encode_with_quality(&image, 95).unwrap();
        assert_ne!(low, high);
        assert!(
            low.len() < high.len(),
            "low={} high={}",
            low.len(),
            high.len()
        );
    }

    #[test]
    fn rejects_cmyk_jpeg() {
        let mut jpeg = Vec::new();
        Encoder::new(&mut jpeg, DEFAULT_QUALITY)
            .encode(&[0, 255, 255, 0], 1, 1, ColorType::Cmyk)
            .unwrap();
        assert!(identify(&jpeg)
            .unwrap_err()
            .to_string()
            .contains("JPEG CMYK is not supported"));
    }

    #[test]
    fn rejects_oversized_jpeg_before_decode_allocation() {
        let mut jpeg = vec![
            0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08, 0xff, 0xff, 0xff, 0xff, 0x03, 0x01, 0x11,
            0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00,
        ];
        jpeg.extend_from_slice(&[0xff, 0xd9]);

        let err = identify(&jpeg).unwrap_err().to_string();
        assert!(err.contains("image pixel buffer too large"), "{err}");
        let err = decode(&jpeg).unwrap_err().to_string();
        assert!(err.contains("image pixel buffer too large"), "{err}");
    }
}
