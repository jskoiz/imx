//! Bounded GIF input decoding with multi-frame selection.
//!
//! The decoder can report how many frames an animated GIF contains
//! ([`frame_count`]) and decode any individual frame as the fully composited
//! canvas at that point in the animation ([`decode_frame`]). Compositing honors
//! the GIF frame disposal methods (`Keep`/`Any`, `Background`, `Previous`) and
//! per-frame transparency, so frame `N` is the actual displayed canvas after
//! frames `0..=N` have been drawn. The single-frame [`decode`] entry point is
//! preserved and returns frame 0.
//!
//! Frame delays, loop counts, and user-input flags are parsed by the underlying
//! `gif` crate but not interpreted here: this crate extracts still frames, it
//! does not play back animation timing. Every frame is composited onto the
//! logical screen canvas as RGBA8, so decode and identify always agree on the
//! reported dimensions.

use std::io::Cursor;

use gif::{ColorOutput, DecodeOptions, DecodingError, DisposalMethod, MemoryLimit};
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

/// Count the number of image frames in a GIF.
///
/// Walks the GIF block stream without fully decoding pixel data. Returns at
/// least 1 for a well-formed GIF; malformed or truncated input yields a clean
/// [`ImageError`] rather than a panic.
pub fn frame_count(input: &[u8]) -> Result<u32, ImageError> {
    let mut decoder = decoder(input, "frame_count")?;
    let mut count: u32 = 0;
    while let Some(_frame) = decoder
        .next_frame_info()
        .map_err(|err| gif_decode_error("frame_count", err))?
    {
        count = count.saturating_add(1);
    }
    Ok(count)
}

/// Decode frame 0 (the first frame) of a GIF as a composited RGBA8 image.
pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    decode_frame(input, 0)
}

/// Decode the `index`-th (0-based) frame of a GIF as the fully composited RGBA8
/// canvas at that point in the animation.
///
/// Frames `0..=index` are decoded in order and composited onto a logical-screen
/// canvas, applying each frame's disposal method (`Background` clears the
/// frame's region to transparent, `Previous` restores the canvas to its state
/// before that frame, `Keep`/`Any` leave it in place) and per-frame
/// transparency. An out-of-range `index` returns
/// [`ImageError::FrameIndexOutOfRange`].
pub fn decode_frame(input: &[u8], index: u32) -> Result<Image, ImageError> {
    let mut decoder = decoder(input, "decode")?;
    let width = u32::from(decoder.width());
    let height = u32::from(decoder.height());
    let canvas_len = pixel_len(width, height, PixelFormat::Rgba8.bytes_per_pixel())?;
    let canvas_width = width as usize;
    let canvas_height = height as usize;

    let mut canvas = try_vec_with_capacity(canvas_len)?;
    canvas.resize(canvas_len, 0);

    // Disposal of the previous frame must be applied before drawing the next
    // frame. We carry the pending action and the data needed to perform it.
    let mut pending_dispose: Option<PendingDispose> = None;
    let mut decoded: u32 = 0;

    loop {
        let frame = decoder
            .read_next_frame()
            .map_err(|err| gif_decode_error("decode", err))?;
        let Some(frame) = frame else {
            // Ran out of frames before reaching `index`.
            return Err(ImageError::FrameIndexOutOfRange {
                index,
                frame_count: decoded,
            });
        };

        let left = frame.left as usize;
        let top = frame.top as usize;
        let frame_width = frame.width as usize;
        let frame_height = frame.height as usize;

        validate_frame_region(
            canvas_width,
            canvas_height,
            left,
            top,
            frame_width,
            frame_height,
            frame.buffer.len(),
        )?;

        // Apply the previous frame's disposal before drawing this one.
        if let Some(pending) = pending_dispose.take() {
            apply_dispose(&mut canvas, canvas_width, &pending);
        }

        // If this frame uses the `Previous` disposal we must remember the
        // region's current contents so we can restore them afterwards.
        let saved_region = if frame.dispose == DisposalMethod::Previous {
            Some(snapshot_region(
                &canvas,
                canvas_width,
                left,
                top,
                frame_width,
                frame_height,
            ))
        } else {
            None
        };

        composite_frame(
            &mut canvas,
            canvas_width,
            left,
            top,
            frame_width,
            frame_height,
            &frame.buffer,
        );

        if decoded == index {
            return Image::new(width, height, PixelFormat::Rgba8, canvas);
        }

        pending_dispose = Some(PendingDispose {
            method: frame.dispose,
            left,
            top,
            frame_width,
            frame_height,
            saved_region,
        });
        decoded = decoded.saturating_add(1);
    }
}

struct PendingDispose {
    method: DisposalMethod,
    left: usize,
    top: usize,
    frame_width: usize,
    frame_height: usize,
    /// Snapshot of the frame region taken before drawing, used only for the
    /// `Previous` disposal method.
    saved_region: Option<Vec<u8>>,
}

fn validate_frame_region(
    canvas_width: usize,
    canvas_height: usize,
    left: usize,
    top: usize,
    frame_width: usize,
    frame_height: usize,
    buffer_len: usize,
) -> Result<(), ImageError> {
    let expected = frame_width
        .checked_mul(frame_height)
        .and_then(|count| count.checked_mul(4))
        .ok_or(ImageError::LengthOverflow)?;
    if buffer_len != expected {
        return Err(ImageError::InvalidPixelBuffer {
            expected,
            actual: buffer_len,
        });
    }
    let right = left
        .checked_add(frame_width)
        .ok_or(ImageError::LengthOverflow)?;
    let bottom = top
        .checked_add(frame_height)
        .ok_or(ImageError::LengthOverflow)?;
    if right > canvas_width || bottom > canvas_height {
        return Err(ImageError::UnsupportedFormat(
            "GIF frame extends beyond the logical screen".to_string(),
        ));
    }
    Ok(())
}

/// Composite a frame buffer (RGBA8, transparent pixels already have alpha 0)
/// onto the canvas. Fully transparent source pixels do not overwrite the
/// canvas, matching GIF transparent-index semantics.
fn composite_frame(
    canvas: &mut [u8],
    canvas_width: usize,
    left: usize,
    top: usize,
    frame_width: usize,
    frame_height: usize,
    frame_buffer: &[u8],
) {
    for row in 0..frame_height {
        let canvas_row = ((top + row) * canvas_width + left) * 4;
        let frame_row = row * frame_width * 4;
        for col in 0..frame_width {
            let src = frame_row + col * 4;
            // Transparent index pixels carry alpha 0; leave the canvas untouched.
            if frame_buffer[src + 3] == 0 {
                continue;
            }
            let dst = canvas_row + col * 4;
            canvas[dst..dst + 4].copy_from_slice(&frame_buffer[src..src + 4]);
        }
    }
}

/// Capture the current canvas contents of a frame region as a contiguous RGBA8
/// buffer, used to restore the region for the `Previous` disposal method.
fn snapshot_region(
    canvas: &[u8],
    canvas_width: usize,
    left: usize,
    top: usize,
    frame_width: usize,
    frame_height: usize,
) -> Vec<u8> {
    let mut region = Vec::with_capacity(frame_width * frame_height * 4);
    for row in 0..frame_height {
        let canvas_row = ((top + row) * canvas_width + left) * 4;
        region.extend_from_slice(&canvas[canvas_row..canvas_row + frame_width * 4]);
    }
    region
}

fn apply_dispose(canvas: &mut [u8], canvas_width: usize, pending: &PendingDispose) {
    match pending.method {
        // No action required; the frame stays on the canvas.
        DisposalMethod::Any | DisposalMethod::Keep => {}
        // Restore the frame's region to the background, which for a composited
        // RGBA canvas means transparent (alpha 0).
        DisposalMethod::Background => {
            for row in 0..pending.frame_height {
                let canvas_row = ((pending.top + row) * canvas_width + pending.left) * 4;
                for col in 0..pending.frame_width {
                    let dst = canvas_row + col * 4;
                    canvas[dst..dst + 4].copy_from_slice(&[0, 0, 0, 0]);
                }
            }
        }
        // Restore the region to the contents captured before the frame drew.
        DisposalMethod::Previous => {
            if let Some(region) = &pending.saved_region {
                for row in 0..pending.frame_height {
                    let canvas_row = ((pending.top + row) * canvas_width + pending.left) * 4;
                    let region_row = row * pending.frame_width * 4;
                    canvas[canvas_row..canvas_row + pending.frame_width * 4]
                        .copy_from_slice(&region[region_row..region_row + pending.frame_width * 4]);
                }
            }
        }
    }
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

    type RgbaFrameSpec = (u16, u16, u16, u16, Vec<u8>);
    type DisposingFrameSpec = (u16, u16, u16, u16, DisposalMethod, Vec<u8>);

    fn gif_fixture(width: u16, height: u16, frames: &[RgbaFrameSpec]) -> Vec<u8> {
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

    fn gif_fixture_with_dispose(width: u16, height: u16, frames: &[DisposingFrameSpec]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = Encoder::new(&mut out, width, height, &[]).unwrap();
            for (left, top, fw, fh, dispose, rgba) in frames {
                let mut pixels = rgba.clone();
                let mut frame = Frame::from_rgba_speed(*fw, *fh, &mut pixels, 10);
                frame.left = *left;
                frame.top = *top;
                frame.dispose = *dispose;
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
    fn decode_returns_first_frame_when_multiple_present() {
        let first = vec![10, 20, 30, 255];
        let second = vec![200, 100, 50, 255];
        let gif = gif_fixture(1, 1, &[(0, 0, 1, 1, first), (0, 0, 1, 1, second)]);
        let decoded = decode(&gif).unwrap();
        assert_eq!(decoded.pixels(), &[10, 20, 30, 255]);
    }

    #[test]
    fn frame_count_counts_all_frames() {
        let first = vec![10, 20, 30, 255];
        let second = vec![200, 100, 50, 255];
        let third = vec![1, 2, 3, 255];
        let single = gif_fixture(1, 1, &[(0, 0, 1, 1, first.clone())]);
        assert_eq!(frame_count(&single).unwrap(), 1);
        let triple = gif_fixture(
            1,
            1,
            &[
                (0, 0, 1, 1, first),
                (0, 0, 1, 1, second),
                (0, 0, 1, 1, third),
            ],
        );
        assert_eq!(frame_count(&triple).unwrap(), 3);
    }

    #[test]
    fn decode_frame_selects_requested_index() {
        // Full-canvas opaque frames with the default Keep disposal: each frame
        // replaces the visible canvas.
        let first = vec![10, 20, 30, 255];
        let second = vec![200, 100, 50, 255];
        let third = vec![1, 2, 3, 255];
        let gif = gif_fixture(
            1,
            1,
            &[
                (0, 0, 1, 1, first),
                (0, 0, 1, 1, second),
                (0, 0, 1, 1, third),
            ],
        );
        assert_eq!(decode_frame(&gif, 0).unwrap().pixels(), &[10, 20, 30, 255]);
        assert_eq!(
            decode_frame(&gif, 1).unwrap().pixels(),
            &[200, 100, 50, 255]
        );
        assert_eq!(decode_frame(&gif, 2).unwrap().pixels(), &[1, 2, 3, 255]);
    }

    #[test]
    fn decode_frame_composites_partial_keep_frame_over_previous() {
        // 2x1 canvas. Frame 0 fills both pixels red with Keep disposal. Frame 1
        // draws only the right pixel green. The composited frame 1 keeps the
        // left red pixel.
        let frame0 = vec![255, 0, 0, 255, 255, 0, 0, 255];
        let frame1 = vec![0, 255, 0, 255];
        let gif = gif_fixture_with_dispose(
            2,
            1,
            &[
                (0, 0, 2, 1, DisposalMethod::Keep, frame0),
                (1, 0, 1, 1, DisposalMethod::Keep, frame1),
            ],
        );
        assert_eq!(
            decode_frame(&gif, 1).unwrap().pixels(),
            &[255, 0, 0, 255, 0, 255, 0, 255]
        );
    }

    #[test]
    fn decode_frame_applies_background_disposal() {
        // 2x1 canvas. Frame 0 fills both pixels red but disposes to background.
        // Frame 1 draws only the right pixel green; after frame 0's background
        // disposal the left pixel is transparent again.
        let frame0 = vec![255, 0, 0, 255, 255, 0, 0, 255];
        let frame1 = vec![0, 255, 0, 255];
        let gif = gif_fixture_with_dispose(
            2,
            1,
            &[
                (0, 0, 2, 1, DisposalMethod::Background, frame0),
                (1, 0, 1, 1, DisposalMethod::Keep, frame1),
            ],
        );
        assert_eq!(
            decode_frame(&gif, 1).unwrap().pixels(),
            &[0, 0, 0, 0, 0, 255, 0, 255]
        );
    }

    #[test]
    fn decode_frame_rejects_out_of_range_index() {
        let gif = gif_fixture(1, 1, &[(0, 0, 1, 1, vec![10, 20, 30, 255])]);
        assert_eq!(
            decode_frame(&gif, 1),
            Err(ImageError::FrameIndexOutOfRange {
                index: 1,
                frame_count: 1,
            })
        );
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
