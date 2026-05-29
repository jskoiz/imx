use std::io::Cursor;

use exif::{In, Reader, Tag};
use imx_core::{
    apply_exif_orientation, exif_oriented_dimensions, pixel_count, Format, Identify, Image,
    ImageError, PixelFormat, MAX_PIXEL_BYTES,
};
use jpeg_decoder::{Decoder, PixelFormat as JpegPixelFormat};
use jpeg_encoder::{ColorType, Encoder, EncodingError};

pub const MAGIC: &[u8; 3] = b"\xff\xd8\xff";
pub const DEFAULT_QUALITY: u8 = 90;
pub const MAX_JPEG_DECODE_BYTES: usize = MAX_PIXEL_BYTES / 4;

/// Identify a JPEG image, auto-applying the EXIF Orientation tag.
///
/// Equivalent to [`identify_with_options`] with `auto_orient` set to `true`.
pub fn identify(input: &[u8]) -> Result<Identify, ImageError> {
    identify_with_options(input, true)
}

/// Identify a JPEG image.
///
/// When `auto_orient` is `true`, the reported dimensions reflect the EXIF
/// Orientation tag (values 5..=8 swap width and height). When it is `false`,
/// the raw stored dimensions are reported. A missing or malformed EXIF
/// Orientation tag is treated as orientation 1 (no-op).
pub fn identify_with_options(input: &[u8], auto_orient: bool) -> Result<Identify, ImageError> {
    let mut decoder = decoder(input)?;
    let (width, height, pixel_format) = checked_info(&mut decoder, "identify")?;
    let (width, height) = if auto_orient {
        exif_oriented_dimensions(exif_orientation(input), width, height)
    } else {
        (width, height)
    };
    Ok(Identify {
        format: Format::Jpeg,
        width,
        height,
        pixel_format,
    })
}

/// Decode a JPEG image, auto-applying the EXIF Orientation tag.
///
/// Equivalent to [`decode_with_options`] with `auto_orient` set to `true`.
pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    decode_with_options(input, true)
}

/// Decode a JPEG image.
///
/// When `auto_orient` is `true`, the EXIF Orientation tag is applied so the
/// returned [`Image`] is upright. When it is `false`, the raw stored pixels are
/// returned. A missing or malformed EXIF Orientation tag is treated as
/// orientation 1 (no-op), so decoding never fails on bad metadata.
pub fn decode_with_options(input: &[u8], auto_orient: bool) -> Result<Image, ImageError> {
    let orientation = if auto_orient {
        exif_orientation(input)
    } else {
        1
    };
    let mut decoder = decoder(input)?;
    let (width, height, pixel_format) = checked_info(&mut decoder, "decode")?;
    let pixels = decoder
        .decode()
        .map_err(|err| jpeg_decode_error("decode", err))?;
    // The ICC profile is only available after a successful `decode`; it is
    // reassembled from the APP2 `ICC_PROFILE` segments by the decoder crate.
    let icc = decoder.icc_profile();
    let image = Image::new(width, height, pixel_format, pixels)?.with_icc(icc);
    apply_exif_orientation(image, orientation)
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
    let mut encoder = Encoder::new(&mut out, quality);
    // Write the embedded ICC profile back as APP2 `ICC_PROFILE` segment(s). The
    // encoder crate handles the standard 14-byte header and ≤65519-byte
    // chunking; an over-large profile (>~16 MB, needing ≥255 chunks) is the only
    // failure mode and surfaces as a normal encode error.
    if let Some(profile) = image.icc() {
        encoder
            .add_icc_profile(profile)
            .map_err(jpeg_encode_error)?;
    }
    encoder
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

/// Extract the EXIF Orientation tag (1..=8) from a JPEG's APP1 segment.
///
/// EXIF parsing is delegated to the mature `kamadak-exif` crate, which locates
/// the APP1/EXIF segment and decodes the embedded TIFF directory. A missing
/// segment, a malformed directory, an out-of-range value, or any parse error is
/// treated as orientation `1` (no-op) so that decoding hostile or non-conforming
/// inputs never fails on metadata alone. The returned value is always within
/// `1..=8` (clamped to `1` otherwise), matching what
/// [`imx_core::apply_exif_orientation`] expects.
pub fn exif_orientation(input: &[u8]) -> u16 {
    let mut cursor = Cursor::new(input);
    let exif = match Reader::new()
        .continue_on_error(true)
        .read_from_container(&mut cursor)
    {
        Ok(exif) => exif,
        // `PartialResult` still carries the fields parsed before the error, so a
        // truncated or partially malformed directory can still yield a valid
        // Orientation tag; any other error means no usable EXIF data.
        Err(exif::Error::PartialResult(partial)) => partial.into_inner().0,
        Err(_) => return 1,
    };
    match exif
        .get_field(Tag::Orientation, In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
    {
        Some(value @ 1..=8) => value as u16,
        _ => 1,
    }
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
    fn round_trips_icc_profile() {
        // A multi-segment profile (>65519 bytes forces ≥2 APP2 chunks) to
        // exercise the encoder's chunking and the decoder's reassembly.
        let profile: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
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
        .unwrap()
        .with_icc(Some(profile.clone()));
        let jpeg = encode(&image).unwrap();
        let decoded = decode(&jpeg).unwrap();
        assert_eq!(decoded.icc(), Some(profile.as_slice()));
    }

    #[test]
    fn encodes_without_icc_when_absent() {
        let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap();
        let jpeg = encode(&image).unwrap();
        assert_eq!(decode(&jpeg).unwrap().icc(), None);
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
    fn invalid_or_malformed_exif_orientation_is_treated_as_identity() {
        let image = Image::new(2, 2, PixelFormat::Rgb8, vec![0x80; 2 * 2 * 3]).unwrap();
        let jpeg = encode(&image).unwrap();
        let baseline = decode(&jpeg).unwrap();

        // Out-of-range Orientation values fall back to identity (no rotation),
        // so dimensions stay raw and pixels are unchanged.
        let out_of_range = jpeg_with_exif_orientation(&jpeg, 9);
        assert_eq!(
            identify(&out_of_range).unwrap().stable_line(),
            "format=JPEG width=2 height=2 channels=RGB depth=8"
        );
        assert_eq!(decode(&out_of_range).unwrap(), baseline);

        // A malformed EXIF/TIFF byte order is tolerated as orientation 1.
        let malformed = jpeg_with_exif_app1(&jpeg, b"Exif\0\0ZZ\0*\0\0\0\x08");
        assert_eq!(exif_orientation(&malformed), 1);
        assert_eq!(decode(&malformed).unwrap(), baseline);
        assert_eq!(
            identify(&malformed).unwrap().stable_line(),
            "format=JPEG width=2 height=2 channels=RGB depth=8"
        );
    }

    #[test]
    fn exif_orientation_extracts_tag_value() {
        let image = Image::new(2, 2, PixelFormat::Rgb8, vec![0x80; 2 * 2 * 3]).unwrap();
        let jpeg = encode(&image).unwrap();
        assert_eq!(
            exif_orientation(&jpeg),
            1,
            "no EXIF tag means orientation 1"
        );
        for orientation in 1..=8u16 {
            assert_eq!(
                exif_orientation(&jpeg_with_exif_orientation(&jpeg, orientation)),
                orientation
            );
        }
    }

    #[test]
    fn auto_orient_option_toggles_normalization() {
        let image = Image::new(3, 2, PixelFormat::Rgb8, vec![0x80; 3 * 2 * 3]).unwrap();
        let jpeg = encode(&image).unwrap();
        let raw = decode_with_options(&jpeg, false).unwrap();
        let oriented = jpeg_with_exif_orientation(&jpeg, 6);

        // With auto-orient disabled, dimensions and pixels stay raw.
        assert_eq!(
            identify_with_options(&oriented, false)
                .unwrap()
                .stable_line(),
            "format=JPEG width=3 height=2 channels=RGB depth=8"
        );
        assert_eq!(decode_with_options(&oriented, false).unwrap(), raw);

        // With auto-orient enabled, dimensions swap and pixels are normalized.
        assert_eq!(
            identify_with_options(&oriented, true)
                .unwrap()
                .stable_line(),
            "format=JPEG width=2 height=3 channels=RGB depth=8"
        );
        assert_eq!(
            decode_with_options(&oriented, true).unwrap(),
            expected_oriented(&raw, 6)
        );
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
