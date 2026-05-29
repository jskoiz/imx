use std::io::Cursor;

use imx_core::{
    pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError, PixelFormat,
    MAX_PIXEL_BYTES,
};
use tiff::decoder::{Decoder, DecodingResult, Limits};
use tiff::encoder::{colortype, TiffEncoder};
use tiff::{ColorType, TiffError};

/// Little-endian TIFF magic (`II\x2a\x00`).
pub const MAGIC_LE: &[u8; 4] = b"II\x2a\x00";
/// Big-endian TIFF magic (`MM\x00\x2a`).
pub const MAGIC_BE: &[u8; 4] = b"MM\x00\x2a";
/// Number of leading bytes inspected for the TIFF magic.
pub const MAGIC_LEN: usize = 4;

/// Identify a TIFF image without fully decoding its pixels.
///
/// Only the first IFD is inspected; additional pages are ignored.
pub fn identify(input: &[u8]) -> Result<Identify, ImageError> {
    let mut decoder = decoder(input, "identify")?;
    let (width, height) = decoder
        .dimensions()
        .map_err(|err| tiff_error("identify", err))?;
    let color = decoder
        .colortype()
        .map_err(|err| tiff_error("identify", err))?;
    let pixel_format = supported_pixel_format(color)?;
    let _ = pixel_len(width, height, pixel_format.bytes_per_pixel())?;
    Ok(Identify {
        format: Format::Tiff,
        width,
        height,
        pixel_format,
    })
}

/// Decode the first image in a TIFF stream into an [`Image`].
///
/// Only the first IFD is decoded; additional pages are ignored. Malformed
/// input is rejected with an [`ImageError`]; decoding never panics.
pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    let mut decoder = decoder(input, "decode")?;
    let (width, height) = decoder
        .dimensions()
        .map_err(|err| tiff_error("decode", err))?;
    let color = decoder
        .colortype()
        .map_err(|err| tiff_error("decode", err))?;
    let pixel_format = supported_pixel_format(color)?;
    let expected = pixel_len(width, height, pixel_format.bytes_per_pixel())?;

    let result = decoder
        .read_image()
        .map_err(|err| tiff_error("decode", err))?;

    let pixels = match (pixel_format, result) {
        (PixelFormat::Gray8 | PixelFormat::Rgb8 | PixelFormat::Rgba8, DecodingResult::U8(data)) => {
            data
        }
        (PixelFormat::Gray16Be | PixelFormat::Rgb16Be, DecodingResult::U16(data)) => {
            u16_samples_to_be_bytes(&data)?
        }
        _ => {
            return Err(ImageError::UnsupportedFormat(
                "TIFF sample layout is not supported".to_string(),
            ));
        }
    };

    if pixels.len() != expected {
        return Err(ImageError::InvalidPixelBuffer {
            expected,
            actual: pixels.len(),
        });
    }

    Image::new(width, height, pixel_format, pixels)
}

/// Encode an [`Image`] as a deterministic little-endian baseline TIFF.
///
/// Output is byte-identical for identical input: uncompressed, no timestamps,
/// single image. Pixel formats without a native TIFF mapping are converted to
/// the nearest supported layout before encoding.
pub fn encode(image: &Image) -> Result<Vec<u8>, ImageError> {
    let mut out = Vec::new();
    let mut encoder = TiffEncoder::new(Cursor::new(&mut out)).map_err(tiff_encode_error)?;
    let width = image.width();
    let height = image.height();

    match image.pixel_format() {
        PixelFormat::Bilevel | PixelFormat::Gray8 => {
            let source = image.to_gray8()?;
            encoder
                .write_image::<colortype::Gray8>(width, height, source.pixels())
                .map_err(tiff_encode_error)?;
        }
        PixelFormat::Gray16Be => {
            let samples = be_bytes_to_u16_samples(image.pixels());
            encoder
                .write_image::<colortype::Gray16>(width, height, &samples)
                .map_err(tiff_encode_error)?;
        }
        PixelFormat::Rgb8 => {
            encoder
                .write_image::<colortype::RGB8>(width, height, image.pixels())
                .map_err(tiff_encode_error)?;
        }
        PixelFormat::Rgb16Be => {
            let samples = be_bytes_to_u16_samples(image.pixels());
            encoder
                .write_image::<colortype::RGB16>(width, height, &samples)
                .map_err(tiff_encode_error)?;
        }
        PixelFormat::Rgba8 => {
            encoder
                .write_image::<colortype::RGBA8>(width, height, image.pixels())
                .map_err(tiff_encode_error)?;
        }
        PixelFormat::Rgba16Be => {
            let source = image.to_rgba8()?;
            encoder
                .write_image::<colortype::RGBA8>(width, height, source.pixels())
                .map_err(tiff_encode_error)?;
        }
    }

    drop(encoder);
    Ok(out)
}

fn supported_pixel_format(color: ColorType) -> Result<PixelFormat, ImageError> {
    match color {
        ColorType::Gray(8) => Ok(PixelFormat::Gray8),
        ColorType::Gray(16) => Ok(PixelFormat::Gray16Be),
        ColorType::RGB(8) => Ok(PixelFormat::Rgb8),
        ColorType::RGB(16) => Ok(PixelFormat::Rgb16Be),
        ColorType::RGBA(8) => Ok(PixelFormat::Rgba8),
        other => Err(ImageError::UnsupportedFormat(format!(
            "TIFF color type {other:?} is not supported"
        ))),
    }
}

fn decoder<'a>(
    input: &'a [u8],
    operation: &'static str,
) -> Result<Decoder<Cursor<&'a [u8]>>, ImageError> {
    if input.len() < MAGIC_LEN {
        return Err(ImageError::UnexpectedEof {
            expected: MAGIC_LEN,
            actual: input.len(),
        });
    }
    let magic = &input[..MAGIC_LEN];
    if magic != MAGIC_LE && magic != MAGIC_BE {
        return Err(ImageError::InvalidHeader("TIFF"));
    }

    let mut limits = Limits::unlimited();
    limits.decoding_buffer_size = MAX_PIXEL_BYTES;
    limits.intermediate_buffer_size = MAX_PIXEL_BYTES;
    Decoder::new(Cursor::new(input))
        .map(|decoder| decoder.with_limits(limits))
        .map_err(|err| tiff_error(operation, err))
}

fn u16_samples_to_be_bytes(samples: &[u16]) -> Result<Vec<u8>, ImageError> {
    let len = samples
        .len()
        .checked_mul(2)
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = try_vec_with_capacity(len)?;
    for sample in samples {
        out.extend_from_slice(&sample.to_be_bytes());
    }
    Ok(out)
}

fn be_bytes_to_u16_samples(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect()
}

fn tiff_error(operation: &'static str, err: TiffError) -> ImageError {
    match err {
        TiffError::LimitsExceeded => ImageError::ImageTooLarge {
            required: MAX_PIXEL_BYTES.saturating_add(1),
            limit: MAX_PIXEL_BYTES,
        },
        other => ImageError::UnsupportedFormat(format!("TIFF {operation} failed: {other}")),
    }
}

fn tiff_encode_error(err: TiffError) -> ImageError {
    match err {
        TiffError::LimitsExceeded => ImageError::ImageTooLarge {
            required: MAX_PIXEL_BYTES.saturating_add(1),
            limit: MAX_PIXEL_BYTES,
        },
        other => ImageError::UnsupportedFormat(format!("TIFF encode failed: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_gray8() {
        let pixels = vec![0, 64, 128, 255];
        let image = Image::new(2, 2, PixelFormat::Gray8, pixels.clone()).unwrap();
        let tiff = encode(&image).unwrap();
        assert_eq!(&tiff[..MAGIC_LEN], MAGIC_LE);
        assert_eq!(
            identify(&tiff).unwrap().stable_line(),
            "format=TIFF width=2 height=2 channels=GRAY depth=8"
        );
        let decoded = decode(&tiff).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Gray8);
        assert_eq!(decoded.pixels(), pixels.as_slice());
    }

    #[test]
    fn round_trips_rgb8() {
        let pixels = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 10, 20, 30];
        let image = Image::new(2, 2, PixelFormat::Rgb8, pixels.clone()).unwrap();
        let tiff = encode(&image).unwrap();
        assert_eq!(
            identify(&tiff).unwrap().stable_line(),
            "format=TIFF width=2 height=2 channels=RGB depth=8"
        );
        let decoded = decode(&tiff).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(decoded.pixels(), pixels.as_slice());
    }

    #[test]
    fn round_trips_rgba8() {
        let pixels = vec![10, 20, 30, 255, 40, 50, 60, 128];
        let image = Image::new(2, 1, PixelFormat::Rgba8, pixels.clone()).unwrap();
        let tiff = encode(&image).unwrap();
        assert_eq!(
            identify(&tiff).unwrap().stable_line(),
            "format=TIFF width=2 height=1 channels=RGBA depth=8"
        );
        let decoded = decode(&tiff).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgba8);
        assert_eq!(decoded.pixels(), pixels.as_slice());
    }

    #[test]
    fn round_trips_rgb16be() {
        let pixels = vec![
            0x00, 0x01, 0x12, 0x34, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff,
        ];
        let image = Image::new(2, 1, PixelFormat::Rgb16Be, pixels.clone()).unwrap();
        let tiff = encode(&image).unwrap();
        assert_eq!(
            identify(&tiff).unwrap().stable_line(),
            "format=TIFF width=2 height=1 channels=RGB depth=16"
        );
        let decoded = decode(&tiff).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgb16Be);
        assert_eq!(decoded.pixels(), pixels.as_slice());
    }

    #[test]
    fn round_trips_gray16be() {
        let pixels = vec![0x00, 0x00, 0x12, 0x34, 0x80, 0x00, 0xff, 0xff];
        let image = Image::new(2, 2, PixelFormat::Gray16Be, pixels.clone()).unwrap();
        let tiff = encode(&image).unwrap();
        assert_eq!(
            identify(&tiff).unwrap().stable_line(),
            "format=TIFF width=2 height=2 channels=GRAY depth=16"
        );
        let decoded = decode(&tiff).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Gray16Be);
        assert_eq!(decoded.pixels(), pixels.as_slice());
    }

    #[test]
    fn encode_is_deterministic() {
        let image = Image::new(
            2,
            2,
            PixelFormat::Rgb8,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        )
        .unwrap();
        assert_eq!(encode(&image).unwrap(), encode(&image).unwrap());
    }

    #[test]
    fn encodes_rgba16_as_rgba8() {
        let image = Image::new(
            1,
            1,
            PixelFormat::Rgba16Be,
            vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xff, 0xff],
        )
        .unwrap();
        let tiff = encode(&image).unwrap();
        assert_eq!(
            identify(&tiff).unwrap().stable_line(),
            "format=TIFF width=1 height=1 channels=RGBA depth=8"
        );
        let decoded = decode(&tiff).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgba8);
        assert_eq!(decoded.pixels(), &[0x12, 0x56, 0x9a, 0xff]);
    }

    #[test]
    fn encodes_bilevel_as_gray8() {
        let image = Image::new(2, 1, PixelFormat::Bilevel, vec![0, 255]).unwrap();
        let tiff = encode(&image).unwrap();
        let decoded = decode(&tiff).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Gray8);
        assert_eq!(decoded.pixels(), &[0, 255]);
    }

    #[test]
    fn rejects_short_input() {
        assert_eq!(
            decode(b"II"),
            Err(ImageError::UnexpectedEof {
                expected: MAGIC_LEN,
                actual: 2,
            })
        );
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = vec![0u8; 16];
        bytes[..4].copy_from_slice(b"RIFF");
        assert_eq!(decode(&bytes), Err(ImageError::InvalidHeader("TIFF")));
    }

    #[test]
    fn rejects_truncated_tiff() {
        let image = Image::new(2, 2, PixelFormat::Rgb8, vec![0x7f; 12]).unwrap();
        let tiff = encode(&image).unwrap();
        let err = decode(&tiff[..MAGIC_LEN + 2]).unwrap_err();
        assert!(matches!(
            err,
            ImageError::UnsupportedFormat(_) | ImageError::ImageTooLarge { .. }
        ));
    }

    #[test]
    fn identify_truncated_reports_identify_operation() {
        let image = Image::new(2, 2, PixelFormat::Rgb8, vec![0x7f; 12]).unwrap();
        let tiff = encode(&image).unwrap();
        let err = identify(&tiff[..MAGIC_LEN + 2]).unwrap_err();
        if let ImageError::UnsupportedFormat(message) = err {
            assert!(message.contains("identify"), "{message}");
        }
    }
}
