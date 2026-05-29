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

/// Count the number of frames in a WebP.
///
/// `image-webp` 0.2.4 exposes WebP animation via `WebPDecoder::num_frames`,
/// so animated WebP files report their true frame count. Still images report
/// 1. The frame count is always at least 1 for a well-formed WebP.
pub fn frame_count(input: &[u8]) -> Result<u32, ImageError> {
    let decoder = decoder(input, "frame_count")?;
    Ok(frame_total(&decoder))
}

/// Decode frame 0 (the first frame) of a WebP. For still images this is the
/// single image; for animated WebP this is the first composited frame.
pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    decode_frame(input, 0)
}

/// Decode the `index`-th (0-based) frame of a WebP as the fully composited
/// RGBA8/RGB8 canvas at that point in the animation.
///
/// `image-webp` composites animation frames internally (honoring per-frame
/// disposal, blending, and the canvas background), so the returned image is the
/// displayed canvas after frames `0..=index`. For still images only index 0 is
/// valid. An out-of-range `index` returns [`ImageError::FrameIndexOutOfRange`].
pub fn decode_frame(input: &[u8], index: u32) -> Result<Image, ImageError> {
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

    let total = frame_total(&decoder);
    if index >= total {
        return Err(ImageError::FrameIndexOutOfRange {
            index,
            frame_count: total,
        });
    }

    let mut pixels = try_vec_with_capacity(output_len)?;
    pixels.resize(output_len, 0);

    if decoder.is_animated() {
        // `read_frame` advances one composited frame at a time; read up to and
        // including the requested index, keeping only the last buffer.
        for _ in 0..=index {
            decoder
                .read_frame(&mut pixels)
                .map_err(|err| webp_decode_error("decode", err))?;
        }
    } else {
        // Non-animated: index is guaranteed to be 0 by the range check above.
        decoder
            .read_image(&mut pixels)
            .map_err(|err| webp_decode_error("decode", err))?;
    }

    Image::new(width, height, pixel_format, pixels)
}

/// Total frame count, normalized so a well-formed still image reports 1.
fn frame_total(decoder: &WebPDecoder<Cursor<&[u8]>>) -> u32 {
    if decoder.is_animated() {
        decoder.num_frames().max(1)
    } else {
        1
    }
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

    // `image-webp` 0.2.4 has no animation encoder, so we hand-assemble a minimal
    // animated WebP from per-frame lossless VP8L bitstreams. Each frame is a
    // 1x1 RGBA pixel encoded as a still VP8L chunk whose payload we lift into an
    // ANMF chunk on a VP8X+ANIM animation container.
    fn vp8l_payload(rgba: &[u8; 4]) -> Vec<u8> {
        let still = webp_fixture(1, 1, ColorType::Rgba8, rgba);
        let mut pos = MAGIC_LEN;
        loop {
            let name = &still[pos..pos + 4];
            let size = u32::from_le_bytes([
                still[pos + 4],
                still[pos + 5],
                still[pos + 6],
                still[pos + 7],
            ]) as usize;
            let data = still[pos + 8..pos + 8 + size].to_vec();
            if name == b"VP8L" {
                return data;
            }
            pos += 8 + size + (size & 1);
        }
    }

    fn write_chunk(out: &mut Vec<u8>, name: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(name);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
        if data.len() % 2 == 1 {
            out.push(0);
        }
    }

    fn put3(out: &mut Vec<u8>, value: u32) {
        out.push((value & 0xff) as u8);
        out.push(((value >> 8) & 0xff) as u8);
        out.push(((value >> 16) & 0xff) as u8);
    }

    fn animated_webp_fixture(frames: &[[u8; 4]]) -> Vec<u8> {
        let mut chunks = Vec::new();

        // VP8X with the animation flag set, 1x1 canvas.
        let mut vp8x = Vec::new();
        vp8x.push(0b0000_0010);
        vp8x.extend_from_slice(&[0, 0, 0]);
        put3(&mut vp8x, 0); // width - 1
        put3(&mut vp8x, 0); // height - 1
        write_chunk(&mut chunks, b"VP8X", &vp8x);

        // ANIM: transparent background, loop forever.
        let mut anim = Vec::new();
        anim.extend_from_slice(&[0, 0, 0, 0]);
        anim.extend_from_slice(&0u16.to_le_bytes());
        write_chunk(&mut chunks, b"ANIM", &anim);

        for (index, rgba) in frames.iter().enumerate() {
            let payload = vp8l_payload(rgba);
            let mut body = Vec::new();
            put3(&mut body, 0); // x / 2
            put3(&mut body, 0); // y / 2
            put3(&mut body, 0); // width - 1
            put3(&mut body, 0); // height - 1
            put3(&mut body, 100 + index as u32); // duration ms
            body.push(0); // blend on, no dispose
            write_chunk(&mut body, b"VP8L", &payload);
            write_chunk(&mut chunks, b"ANMF", &body);
        }

        let mut out = Vec::new();
        out.extend_from_slice(RIFF_MAGIC);
        out.extend_from_slice(&((4 + chunks.len()) as u32).to_le_bytes());
        out.extend_from_slice(WEBP_MAGIC);
        out.extend_from_slice(&chunks);
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

    #[test]
    fn still_webp_reports_single_frame() {
        let webp = webp_fixture(2, 1, ColorType::Rgb8, &[255, 0, 0, 0, 255, 0]);
        assert_eq!(frame_count(&webp).unwrap(), 1);
        // Frame 0 is the still image.
        assert_eq!(
            decode_frame(&webp, 0).unwrap().pixels(),
            &[255, 0, 0, 0, 255, 0]
        );
        // Any index beyond 0 is rejected cleanly.
        assert_eq!(
            decode_frame(&webp, 1),
            Err(ImageError::FrameIndexOutOfRange {
                index: 1,
                frame_count: 1,
            })
        );
    }

    #[test]
    fn animated_webp_reports_frame_count() {
        let webp = animated_webp_fixture(&[[255, 0, 0, 255], [0, 255, 0, 255]]);
        assert_eq!(frame_count(&webp).unwrap(), 2);
    }

    #[test]
    fn animated_webp_decodes_selected_frame() {
        let webp = animated_webp_fixture(&[[255, 0, 0, 255], [0, 255, 0, 255]]);
        // The animation has no alpha flag on the canvas, so frames composite to
        // RGB8. Lossless VP8L round-trips 255 as 254 for these primaries.
        let frame0 = decode_frame(&webp, 0).unwrap();
        assert_eq!(frame0.pixel_format(), PixelFormat::Rgb8);
        assert_eq!(frame0.pixels(), &[254, 0, 0]);
        let frame1 = decode_frame(&webp, 1).unwrap();
        assert_eq!(frame1.pixels(), &[0, 254, 0]);
    }

    #[test]
    fn animated_webp_rejects_out_of_range_frame() {
        let webp = animated_webp_fixture(&[[255, 0, 0, 255], [0, 255, 0, 255]]);
        assert_eq!(
            decode_frame(&webp, 2),
            Err(ImageError::FrameIndexOutOfRange {
                index: 2,
                frame_count: 2,
            })
        );
    }

    #[test]
    fn animated_webp_frame_selection_is_deterministic() {
        let webp = animated_webp_fixture(&[[255, 0, 0, 255], [0, 255, 0, 255]]);
        assert_eq!(
            decode_frame(&webp, 1).unwrap().pixels(),
            decode_frame(&webp, 1).unwrap().pixels()
        );
    }
}
