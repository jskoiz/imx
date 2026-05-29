//! Bounded GIF input decoding.
//!
//! Only the first frame is decoded. Animation and multi-frame GIFs are out of
//! scope for this crate: any frames after the first are ignored, and frame
//! delays, disposal methods, and loop counts are not interpreted. The first
//! frame is composited onto the logical screen canvas as RGBA8, so decode and
//! identify always agree on the reported dimensions.

use std::io::Cursor;

use gif::{ColorOutput, DecodeOptions, DecodingError, MemoryLimit};
use imx_core::{
    pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError, PixelFormat,
    MAX_PIXEL_BYTES,
};

pub const MAGIC_87A: &[u8; 6] = b"GIF87a";
pub const MAGIC_89A: &[u8; 6] = b"GIF89a";
pub const MAGIC_LEN: usize = 6;

pub fn identify(input: &[u8]) -> Result<Identify, ImageError> {
    let decoder = decoder(input, "identify")?;
    let width = u32::from(decoder.width());
    let height = u32::from(decoder.height());
    let _ = pixel_len(width, height, PixelFormat::Rgba8.bytes_per_pixel())?;
    Ok(Identify {
        format: Format::Gif,
        width,
        height,
        pixel_format: PixelFormat::Rgba8,
    })
}

pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    let mut decoder = decoder(input, "decode")?;
    let width = u32::from(decoder.width());
    let height = u32::from(decoder.height());
    let canvas_len = pixel_len(width, height, PixelFormat::Rgba8.bytes_per_pixel())?;

    let mut canvas = try_vec_with_capacity(canvas_len)?;
    canvas.resize(canvas_len, 0);

    let frame = decoder
        .read_next_frame()
        .map_err(|err| gif_decode_error("decode", err))?
        .ok_or_else(|| {
            ImageError::UnsupportedFormat("GIF decode failed: no image frame".to_string())
        })?;

    composite_first_frame(
        &mut canvas,
        width as usize,
        height as usize,
        frame.left as usize,
        frame.top as usize,
        frame.width as usize,
        frame.height as usize,
        &frame.buffer,
    )?;

    Image::new(width, height, PixelFormat::Rgba8, canvas)
}

#[allow(clippy::too_many_arguments)]
fn composite_first_frame(
    canvas: &mut [u8],
    canvas_width: usize,
    canvas_height: usize,
    frame_left: usize,
    frame_top: usize,
    frame_width: usize,
    frame_height: usize,
    frame_buffer: &[u8],
) -> Result<(), ImageError> {
    let expected = frame_width
        .checked_mul(frame_height)
        .and_then(|count| count.checked_mul(4))
        .ok_or(ImageError::LengthOverflow)?;
    if frame_buffer.len() != expected {
        return Err(ImageError::InvalidPixelBuffer {
            expected,
            actual: frame_buffer.len(),
        });
    }
    if frame_left + frame_width > canvas_width || frame_top + frame_height > canvas_height {
        return Err(ImageError::UnsupportedFormat(
            "GIF frame extends beyond the logical screen".to_string(),
        ));
    }

    for row in 0..frame_height {
        let canvas_offset = ((frame_top + row) * canvas_width + frame_left) * 4;
        let frame_offset = row * frame_width * 4;
        canvas[canvas_offset..canvas_offset + frame_width * 4]
            .copy_from_slice(&frame_buffer[frame_offset..frame_offset + frame_width * 4]);
    }
    Ok(())
}

fn decoder<'a>(
    input: &'a [u8],
    operation: &'static str,
) -> Result<gif::Decoder<Cursor<&'a [u8]>>, ImageError> {
    if input.len() < MAGIC_LEN {
        return Err(ImageError::UnexpectedEof {
            expected: MAGIC_LEN,
            actual: input.len(),
        });
    }
    if &input[..MAGIC_LEN] != MAGIC_87A && &input[..MAGIC_LEN] != MAGIC_89A {
        return Err(ImageError::InvalidHeader("GIF"));
    }

    let mut options = DecodeOptions::new();
    options.set_color_output(ColorOutput::RGBA);
    options.set_memory_limit(memory_limit());
    options
        .read_info(Cursor::new(input))
        .map_err(|err| gif_decode_error(operation, err))
}

fn memory_limit() -> MemoryLimit {
    match u64::try_from(MAX_PIXEL_BYTES)
        .ok()
        .and_then(std::num::NonZeroU64::new)
    {
        Some(bytes) => MemoryLimit::Bytes(bytes),
        None => MemoryLimit::Unlimited,
    }
}

fn gif_decode_error(operation: &'static str, err: DecodingError) -> ImageError {
    if matches!(err, DecodingError::OutOfMemory) {
        return ImageError::ImageTooLarge {
            required: MAX_PIXEL_BYTES.saturating_add(1),
            limit: MAX_PIXEL_BYTES,
        };
    }
    ImageError::UnsupportedFormat(format!("GIF {operation} failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gif::{Encoder, Frame};

    fn gif_fixture(width: u16, height: u16, frames: &[(u16, u16, u16, u16, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = Encoder::new(&mut out, width, height, &[]).unwrap();
            for (left, top, fw, fh, rgba) in frames {
                let mut pixels = rgba.clone();
                let mut frame = Frame::from_rgba_speed(*fw, *fh, &mut pixels, 10);
                frame.left = *left;
                frame.top = *top;
                encoder.write_frame(&frame).unwrap();
            }
        }
        out
    }

    #[test]
    fn decodes_first_frame_rgba8() {
        let rgba = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let gif = gif_fixture(2, 1, &[(0, 0, 2, 1, rgba)]);
        assert_eq!(&gif[..MAGIC_LEN], MAGIC_89A);
        assert_eq!(
            identify(&gif).unwrap().stable_line(),
            "format=GIF width=2 height=1 channels=RGBA depth=8"
        );
        let decoded = decode(&gif).unwrap();
        assert_eq!(decoded.width(), 2);
        assert_eq!(decoded.height(), 1);
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgba8);
        assert_eq!(decoded.pixels(), &[255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn ignores_frames_after_the_first() {
        let first = vec![10, 20, 30, 255];
        let second = vec![200, 100, 50, 255];
        let gif = gif_fixture(1, 1, &[(0, 0, 1, 1, first), (0, 0, 1, 1, second)]);
        let decoded = decode(&gif).unwrap();
        assert_eq!(decoded.pixels(), &[10, 20, 30, 255]);
    }

    #[test]
    fn composites_offset_first_frame_onto_canvas() {
        let pixel = vec![5, 6, 7, 255];
        let gif = gif_fixture(2, 2, &[(1, 1, 1, 1, pixel)]);
        let decoded = decode(&gif).unwrap();
        assert_eq!(decoded.width(), 2);
        assert_eq!(decoded.height(), 2);
        let mut expected = vec![0u8; 16];
        expected[12..16].copy_from_slice(&[5, 6, 7, 255]);
        assert_eq!(decoded.pixels(), expected.as_slice());
    }

    #[test]
    fn rejects_short_input() {
        assert_eq!(
            decode(b"GIF"),
            Err(ImageError::UnexpectedEof {
                expected: MAGIC_LEN,
                actual: 3,
            })
        );
    }

    #[test]
    fn rejects_bad_magic() {
        assert_eq!(decode(b"NOTGIF"), Err(ImageError::InvalidHeader("GIF")));
    }

    #[test]
    fn rejects_truncated_gif() {
        let gif = gif_fixture(2, 1, &[(0, 0, 2, 1, vec![1, 2, 3, 255, 4, 5, 6, 255])]);
        let err = decode(&gif[..MAGIC_LEN + 4]).unwrap_err().to_string();
        assert!(err.contains("GIF decode failed"), "{err}");
    }
}
