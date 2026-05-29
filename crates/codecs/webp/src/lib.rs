use std::io::Cursor;

use image_webp::{ColorType, DecodingError, EncodingError, WebPDecoder, WebPEncoder};
use imx_core::{
    pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError, PixelFormat,
    MAX_PIXEL_BYTES,
};

pub const RIFF_MAGIC: &[u8; 4] = b"RIFF";
pub const WEBP_MAGIC: &[u8; 4] = b"WEBP";
pub const MAGIC_LEN: usize = 12;

pub fn identify(input: &[u8]) -> Result<Identify, ImageError> {
    let decoder = decoder(input, "identify")?;
    let (width, height) = decoder.dimensions();
    let pixel_format = pixel_format(&decoder);
    let _ = pixel_len(width, height, pixel_format.bytes_per_pixel())?;
    Ok(Identify {
        format: Format::Webp,
        width,
        height,
        pixel_format,
    })
}

pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    let mut decoder = decoder(input, "decode")?;
    let (width, height) = decoder.dimensions();
    let pixel_format = pixel_format(&decoder);
    let expected = pixel_len(width, height, pixel_format.bytes_per_pixel())?;

    let output_len = decoder
        .output_buffer_size()
        .ok_or(ImageError::LengthOverflow)?;
    if output_len != expected {
        return Err(ImageError::InvalidPixelBuffer {
            expected,
            actual: output_len,
        });
    }

    let mut pixels = try_vec_with_capacity(output_len)?;
    pixels.resize(output_len, 0);
    decoder
        .read_image(&mut pixels)
        .map_err(|err| webp_decode_error("decode", err))?;

    Image::new(width, height, pixel_format, pixels)
}

pub fn encode(image: &Image) -> Result<Vec<u8>, ImageError> {
    let (encoded, color_type) = encode_source(image)?;
    let mut out = Vec::new();
    WebPEncoder::new(Cursor::new(&mut out))
        .encode(
            encoded.pixels(),
            encoded.width(),
            encoded.height(),
            color_type,
        )
        .map_err(webp_encode_error)?;
    Ok(out)
}

fn encode_source(image: &Image) -> Result<(Image, ColorType), ImageError> {
    match image.pixel_format() {
        PixelFormat::Bilevel
        | PixelFormat::Gray8
        | PixelFormat::Gray16Be
        | PixelFormat::Rgb8
        | PixelFormat::Rgb16Be => Ok((image.to_rgb8()?, ColorType::Rgb8)),
        PixelFormat::Rgba8 | PixelFormat::Rgba16Be => Ok((image.to_rgba8()?, ColorType::Rgba8)),
    }
}

fn webp_encode_error(err: EncodingError) -> ImageError {
    match err {
        EncodingError::InvalidDimensions => ImageError::UnsupportedFormat(
            "WEBP dimensions are not allowed by the format".to_string(),
        ),
        other => ImageError::UnsupportedFormat(format!("WEBP encode failed: {other}")),
    }
}

fn decoder<'a>(
    input: &'a [u8],
    operation: &'static str,
) -> Result<WebPDecoder<Cursor<&'a [u8]>>, ImageError> {
    if input.len() < MAGIC_LEN {
        return Err(ImageError::UnexpectedEof {
            expected: MAGIC_LEN,
            actual: input.len(),
        });
    }
    if &input[..4] != RIFF_MAGIC || &input[8..12] != WEBP_MAGIC {
        return Err(ImageError::InvalidHeader("WEBP"));
    }

    let mut decoder =
        WebPDecoder::new(Cursor::new(input)).map_err(|err| webp_decode_error(operation, err))?;
    decoder.set_memory_limit(MAX_PIXEL_BYTES);
    Ok(decoder)
}

fn pixel_format(decoder: &WebPDecoder<Cursor<&[u8]>>) -> PixelFormat {
    if decoder.has_alpha() {
        PixelFormat::Rgba8
    } else {
        PixelFormat::Rgb8
    }
}

fn webp_decode_error(operation: &'static str, err: DecodingError) -> ImageError {
    match err {
        DecodingError::ImageTooLarge | DecodingError::MemoryLimitExceeded => {
            ImageError::ImageTooLarge {
                required: MAX_PIXEL_BYTES.saturating_add(1),
                limit: MAX_PIXEL_BYTES,
            }
        }
        other => ImageError::UnsupportedFormat(format!("WEBP {operation} failed: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image_webp::{ColorType, WebPEncoder};

    fn webp_fixture(width: u32, height: u32, color: ColorType, pixels: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        WebPEncoder::new(Cursor::new(&mut out))
            .encode(pixels, width, height, color)
            .unwrap();
        out
    }

    #[test]
    fn decodes_rgb8_webp() {
        let pixels = vec![255, 0, 0, 0, 255, 0];
        let webp = webp_fixture(2, 1, ColorType::Rgb8, &pixels);
        assert_eq!(&webp[..4], RIFF_MAGIC);
        assert_eq!(&webp[8..12], WEBP_MAGIC);
        assert_eq!(
            identify(&webp).unwrap().stable_line(),
            "format=WEBP width=2 height=1 channels=RGB depth=8"
        );
        let decoded = decode(&webp).unwrap();
        assert_eq!(decoded.width(), 2);
        assert_eq!(decoded.height(), 1);
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(decoded.pixels(), pixels.as_slice());
    }

    #[test]
    fn decodes_rgba8_webp() {
        let pixels = vec![10, 20, 30, 255, 40, 50, 60, 128];
        let webp = webp_fixture(2, 1, ColorType::Rgba8, &pixels);
        assert_eq!(
            identify(&webp).unwrap().stable_line(),
            "format=WEBP width=2 height=1 channels=RGBA depth=8"
        );
        let decoded = decode(&webp).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgba8);
        assert_eq!(decoded.pixels(), pixels.as_slice());
    }

    #[test]
    fn rejects_short_input() {
        assert_eq!(
            decode(b"RIFF"),
            Err(ImageError::UnexpectedEof {
                expected: MAGIC_LEN,
                actual: 4,
            })
        );
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = vec![0u8; MAGIC_LEN];
        bytes[..4].copy_from_slice(b"RIFX");
        assert_eq!(decode(&bytes), Err(ImageError::InvalidHeader("WEBP")));

        let mut bytes = vec![0u8; MAGIC_LEN];
        bytes[..4].copy_from_slice(RIFF_MAGIC);
        bytes[8..12].copy_from_slice(b"PNG ");
        assert_eq!(decode(&bytes), Err(ImageError::InvalidHeader("WEBP")));
    }

    #[test]
    fn encodes_rgb8_round_trips() {
        let pixels = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 10, 20, 30];
        let image = Image::new(2, 2, PixelFormat::Rgb8, pixels.clone()).unwrap();
        let webp = encode(&image).unwrap();
        assert_eq!(&webp[..4], RIFF_MAGIC);
        assert_eq!(&webp[8..12], WEBP_MAGIC);
        assert_eq!(
            identify(&webp).unwrap().stable_line(),
            "format=WEBP width=2 height=2 channels=RGB depth=8"
        );
        let decoded = decode(&webp).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(decoded.pixels(), pixels.as_slice());
    }

    #[test]
    fn encodes_rgba8_round_trips() {
        let pixels = vec![10, 20, 30, 255, 40, 50, 60, 128];
        let image = Image::new(2, 1, PixelFormat::Rgba8, pixels.clone()).unwrap();
        let webp = encode(&image).unwrap();
        assert_eq!(
            identify(&webp).unwrap().stable_line(),
            "format=WEBP width=2 height=1 channels=RGBA depth=8"
        );
        let decoded = decode(&webp).unwrap();
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgba8);
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
    fn encodes_gray8_as_rgb() {
        let image = Image::new(2, 1, PixelFormat::Gray8, vec![0, 255]).unwrap();
        let webp = encode(&image).unwrap();
        assert_eq!(
            identify(&webp).unwrap().stable_line(),
            "format=WEBP width=2 height=1 channels=RGB depth=8"
        );
        let decoded = decode(&webp).unwrap();
        assert_eq!(decoded.pixels(), &[0, 0, 0, 255, 255, 255]);
    }

    #[test]
    fn rejects_truncated_webp() {
        let webp = webp_fixture(2, 1, ColorType::Rgb8, &[255, 0, 0, 0, 255, 0]);
        let err = decode(&webp[..MAGIC_LEN + 1]).unwrap_err().to_string();
        assert!(err.contains("WEBP decode failed"), "{err}");
    }
}
