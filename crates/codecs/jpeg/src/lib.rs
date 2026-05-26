use std::io::Cursor;

use imx_core::{pixel_count, Format, Identify, Image, ImageError, PixelFormat, MAX_PIXEL_BYTES};
use jpeg_decoder::{Decoder, PixelFormat as JpegPixelFormat};
use jpeg_encoder::{ColorType, Encoder, EncodingError};

pub const MAGIC: &[u8; 3] = b"\xff\xd8\xff";
pub const DEFAULT_QUALITY: u8 = 90;
pub const MAX_JPEG_DECODE_BYTES: usize = MAX_PIXEL_BYTES / 4;

pub fn identify(input: &[u8]) -> Result<Identify, ImageError> {
    let mut decoder = decoder(input)?;
    let (width, height, pixel_format) = checked_info(&mut decoder, "identify")?;
    Ok(Identify {
        format: Format::Jpeg,
        width,
        height,
        pixel_format,
    })
}

pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    let mut decoder = decoder(input)?;
    let (width, height, pixel_format) = checked_info(&mut decoder, "decode")?;
    let pixels = decoder
        .decode()
        .map_err(|err| jpeg_decode_error("decode", err))?;
    Image::new(width, height, pixel_format, pixels)
}

pub fn encode(image: &Image) -> Result<Vec<u8>, ImageError> {
    let (encoded, color_type) = encode_source(image)?;
    let width = u16::try_from(encoded.width()).map_err(|_| {
        ImageError::UnsupportedFormat("JPEG dimensions exceed 65535 pixels".to_string())
    })?;
    let height = u16::try_from(encoded.height()).map_err(|_| {
        ImageError::UnsupportedFormat("JPEG dimensions exceed 65535 pixels".to_string())
    })?;

    let mut out = Vec::new();
    Encoder::new(&mut out, DEFAULT_QUALITY)
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
