use std::io::Cursor;

use imx_core::{
    pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError, PixelFormat,
    MAX_PIXEL_BYTES,
};
use std::borrow::Cow;

use png::{
    BitDepth, ColorType, Compression, Decoder, DecodingError, Encoder, EncodingError, Filter, Info,
    Limits, Transformations,
};

pub const MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub fn identify(input: &[u8]) -> Result<Identify, ImageError> {
    let reader = png_reader(input, "identify")?;
    let info = reader.info();
    let pixel_format = supported_pixel_format(info.color_type, info.bit_depth)?;
    Ok(Identify {
        format: Format::Png,
        width: info.width,
        height: info.height,
        pixel_format,
    })
}

pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    let mut reader = png_reader(input, "decode")?;
    let output_len = reader
        .output_buffer_size()
        .ok_or(ImageError::LengthOverflow)?;
    if output_len > MAX_PIXEL_BYTES {
        return Err(ImageError::ImageTooLarge {
            required: output_len,
            limit: MAX_PIXEL_BYTES,
        });
    }
    let mut pixels = try_vec_with_capacity(output_len)?;
    pixels.resize(output_len, 0);
    let output = reader
        .next_frame(&mut pixels)
        .map_err(|err| png_decode_error("decode", err))?;
    pixels.truncate(output.buffer_size());
    let pixel_format = supported_pixel_format(output.color_type, output.bit_depth)?;

    if output.color_type == ColorType::GrayscaleAlpha {
        pixels = expand_gray_alpha(output.bit_depth, &pixels)?;
    }

    // Preserve the embedded ICC profile (iCCP chunk) verbatim, if present.
    let icc = reader
        .info()
        .icc_profile
        .as_ref()
        .map(|profile| profile.to_vec());

    Ok(Image::new(output.width, output.height, pixel_format, pixels)?.with_icc(icc))
}

pub fn encode(image: &Image) -> Result<Vec<u8>, ImageError> {
    let (encoded, color_type, bit_depth) = match image.pixel_format() {
        PixelFormat::Bilevel | PixelFormat::Gray8 => {
            let gray = image.to_gray8()?;
            (gray, ColorType::Grayscale, BitDepth::Eight)
        }
        PixelFormat::Gray16Be => (image.clone(), ColorType::Grayscale, BitDepth::Sixteen),
        PixelFormat::Rgb8 => (image.clone(), ColorType::Rgb, BitDepth::Eight),
        PixelFormat::Rgb16Be => (image.clone(), ColorType::Rgb, BitDepth::Sixteen),
        PixelFormat::Rgba8 => (image.clone(), ColorType::Rgba, BitDepth::Eight),
        PixelFormat::Rgba16Be => (image.clone(), ColorType::Rgba, BitDepth::Sixteen),
    };

    let mut out = Vec::new();
    // Build the header through `Info` so the embedded ICC profile (if any) can
    // be written back as an `iCCP` chunk; the `png` 0.18 `Encoder` exposes no
    // direct `set_icc_profile` setter, so the profile must travel via `Info`.
    let mut info = Info::with_size(encoded.width(), encoded.height());
    info.color_type = color_type;
    info.bit_depth = bit_depth;
    info.icc_profile = image.icc().map(|profile| Cow::Owned(profile.to_vec()));
    let mut encoder = Encoder::with_info(&mut out, info).map_err(png_encode_error)?;
    encoder.set_compression(Compression::Fast);
    encoder.set_filter(Filter::NoFilter);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(encoded.pixels()))
        .map_err(png_encode_error)?;
    Ok(out)
}

fn png_reader<'a>(
    input: &'a [u8],
    operation: &'static str,
) -> Result<png::Reader<Cursor<&'a [u8]>>, ImageError> {
    if input.len() < MAGIC.len() {
        return Err(ImageError::UnexpectedEof {
            expected: MAGIC.len(),
            actual: input.len(),
        });
    }
    if &input[..MAGIC.len()] != MAGIC {
        return Err(ImageError::InvalidHeader("PNG"));
    }

    let mut decoder = Decoder::new_with_limits(
        Cursor::new(input),
        Limits {
            bytes: MAX_PIXEL_BYTES,
        },
    );
    decoder.set_transformations(Transformations::IDENTITY);
    let reader = decoder
        .read_info()
        .map_err(|err| png_decode_error(operation, err))?;
    validate_info(reader.info())?;
    Ok(reader)
}

fn validate_info(info: &png::Info<'_>) -> Result<(), ImageError> {
    if info.interlaced {
        return Err(ImageError::UnsupportedFormat(
            "PNG interlacing is not supported".to_string(),
        ));
    }
    if info.animation_control.is_some() || info.frame_control.is_some() {
        return Err(ImageError::UnsupportedFormat(
            "PNG animation is not supported".to_string(),
        ));
    }
    if info.trns.is_some() {
        return Err(ImageError::UnsupportedFormat(
            "PNG tRNS transparency is not supported".to_string(),
        ));
    }
    let pixel_format = supported_pixel_format(info.color_type, info.bit_depth)?;
    let _ = pixel_len(info.width, info.height, pixel_format.bytes_per_pixel())?;
    Ok(())
}

fn supported_pixel_format(
    color_type: ColorType,
    bit_depth: BitDepth,
) -> Result<PixelFormat, ImageError> {
    match (color_type, bit_depth) {
        (ColorType::Grayscale, BitDepth::Eight) => Ok(PixelFormat::Gray8),
        (ColorType::Grayscale, BitDepth::Sixteen) => Ok(PixelFormat::Gray16Be),
        (ColorType::Rgb, BitDepth::Eight) => Ok(PixelFormat::Rgb8),
        (ColorType::Rgb, BitDepth::Sixteen) => Ok(PixelFormat::Rgb16Be),
        (ColorType::Rgba, BitDepth::Eight) => Ok(PixelFormat::Rgba8),
        (ColorType::Rgba, BitDepth::Sixteen) => Ok(PixelFormat::Rgba16Be),
        (ColorType::GrayscaleAlpha, BitDepth::Eight) => Ok(PixelFormat::Rgba8),
        (ColorType::GrayscaleAlpha, BitDepth::Sixteen) => Ok(PixelFormat::Rgba16Be),
        (ColorType::Indexed, _) => Err(ImageError::UnsupportedFormat(
            "PNG indexed color is not supported".to_string(),
        )),
        (_, BitDepth::One | BitDepth::Two | BitDepth::Four) => Err(ImageError::UnsupportedFormat(
            "PNG sub-8-bit samples are not supported".to_string(),
        )),
    }
}

fn expand_gray_alpha(bit_depth: BitDepth, input: &[u8]) -> Result<Vec<u8>, ImageError> {
    match bit_depth {
        BitDepth::Eight => {
            let mut out = try_vec_with_capacity(
                input
                    .len()
                    .checked_mul(2)
                    .ok_or(ImageError::LengthOverflow)?,
            )?;
            for px in input.chunks_exact(2) {
                out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
            }
            Ok(out)
        }
        BitDepth::Sixteen => {
            let mut out = try_vec_with_capacity(
                input
                    .len()
                    .checked_mul(2)
                    .ok_or(ImageError::LengthOverflow)?,
            )?;
            for px in input.chunks_exact(4) {
                out.extend_from_slice(&px[0..2]);
                out.extend_from_slice(&px[0..2]);
                out.extend_from_slice(&px[0..2]);
                out.extend_from_slice(&px[2..4]);
            }
            Ok(out)
        }
        _ => Err(ImageError::UnsupportedFormat(
            "PNG grayscale-alpha bit depth is not supported".to_string(),
        )),
    }
}

fn png_decode_error(operation: &'static str, err: DecodingError) -> ImageError {
    if matches!(err, DecodingError::LimitsExceeded) {
        return ImageError::ImageTooLarge {
            required: MAX_PIXEL_BYTES.saturating_add(1),
            limit: MAX_PIXEL_BYTES,
        };
    }
    ImageError::UnsupportedFormat(format!("PNG {operation} failed: {err}"))
}

fn png_encode_error(err: EncodingError) -> ImageError {
    if matches!(err, EncodingError::LimitsExceeded) {
        return ImageError::ImageTooLarge {
            required: MAX_PIXEL_BYTES.saturating_add(1),
            limit: MAX_PIXEL_BYTES,
        };
    }
    ImageError::UnsupportedFormat(format!("PNG encode failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_fixture(
        width: u32,
        height: u32,
        color_type: ColorType,
        bit_depth: BitDepth,
        pixels: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = Encoder::new(&mut out, width, height);
        encoder.set_color(color_type);
        encoder.set_depth(bit_depth);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(pixels)
            .unwrap();
        out
    }

    #[test]
    fn encodes_and_decodes_rgb8_png() {
        let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 128, 255]).unwrap();
        let png = encode(&image).unwrap();
        assert_eq!(&png[..MAGIC.len()], MAGIC);
        assert_eq!(
            identify(&png).unwrap().stable_line(),
            "format=PNG width=2 height=1 channels=RGB depth=8"
        );
        assert_eq!(decode(&png).unwrap(), image);
    }

    #[test]
    fn encodes_and_decodes_rgba16_png() {
        let image = Image::new(
            1,
            1,
            PixelFormat::Rgba16Be,
            vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0],
        )
        .unwrap();
        let png = encode(&image).unwrap();
        assert_eq!(
            identify(&png).unwrap().stable_line(),
            "format=PNG width=1 height=1 channels=RGBA depth=16"
        );
        assert_eq!(decode(&png).unwrap(), image);
    }

    #[test]
    fn decodes_grayscale_alpha_png_to_rgba8() {
        let png = png_fixture(
            2,
            1,
            ColorType::GrayscaleAlpha,
            BitDepth::Eight,
            &[0x20, 0x80, 0xff, 0x40],
        );
        assert_eq!(
            identify(&png).unwrap().stable_line(),
            "format=PNG width=2 height=1 channels=RGBA depth=8"
        );
        assert_eq!(
            decode(&png).unwrap(),
            Image::new(
                2,
                1,
                PixelFormat::Rgba8,
                vec![0x20, 0x20, 0x20, 0x80, 0xff, 0xff, 0xff, 0x40],
            )
            .unwrap()
        );
    }

    #[test]
    fn decodes_grayscale_alpha_png_to_rgba16() {
        let png = png_fixture(
            2,
            1,
            ColorType::GrayscaleAlpha,
            BitDepth::Sixteen,
            &[0x12, 0x34, 0x80, 0x00, 0xff, 0xff, 0x00, 0x01],
        );
        assert_eq!(
            identify(&png).unwrap().stable_line(),
            "format=PNG width=2 height=1 channels=RGBA depth=16"
        );
        assert_eq!(
            decode(&png).unwrap(),
            Image::new(
                2,
                1,
                PixelFormat::Rgba16Be,
                vec![
                    0x12, 0x34, 0x12, 0x34, 0x12, 0x34, 0x80, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0x00, 0x01,
                ],
            )
            .unwrap()
        );
    }

    #[test]
    fn round_trips_icc_profile() {
        // A non-trivial blob so chunking/compression has something to do.
        let profile: Vec<u8> = (0..512u32).map(|i| (i % 251) as u8).collect();
        let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 128, 255])
            .unwrap()
            .with_icc(Some(profile.clone()));
        let png = encode(&image).unwrap();
        let decoded = decode(&png).unwrap();
        assert_eq!(decoded.icc(), Some(profile.as_slice()));
        assert_eq!(decoded.pixels(), image.pixels());
    }

    #[test]
    fn encodes_without_icc_when_absent() {
        let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 128, 255]).unwrap();
        let png = encode(&image).unwrap();
        assert_eq!(decode(&png).unwrap().icc(), None);
    }

    #[test]
    fn rejects_truncated_png() {
        let err = decode(MAGIC).unwrap_err().to_string();
        assert!(err.contains("PNG decode failed"), "{err}");
    }

    #[test]
    fn truncated_identify_reports_identify_operation() {
        let err = identify(MAGIC).unwrap_err().to_string();
        assert!(err.contains("PNG identify failed"), "{err}");
    }
}
