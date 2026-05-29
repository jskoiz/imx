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
//!
//! Encoding writes a single still frame with a palette of at most 256 colors.
//! Quantization is deterministic: when the source uses at most 256 distinct
//! colors the exact palette is preserved, otherwise a NeuQuant color map with a
//! fixed sample factor is used. Fully transparent pixels (alpha 0) are mapped to
//! a single reserved transparent palette index; if there are none, no
//! transparency is emitted.
//!
//! Animated GIF output is supported via [`encode_animation`], which writes one
//! image block per frame. Each frame carries its own local palette, quantized
//! independently with the same deterministic strategy as the single-frame path,
//! so frames never share or fight over a single global color table. A Netscape
//! looping extension records the requested repeat count and every frame stores
//! its delay in centiseconds.

use std::io::Cursor;

use color_quant::NeuQuant;
use gif::{
    ColorOutput, DecodeOptions, DecodingError, DisposalMethod, Encoder, EncodingError, Frame,
    MemoryLimit, Repeat,
};
use imx_core::{
    pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError, PixelFormat,
    MAX_PIXEL_BYTES,
};

pub const MAGIC_87A: &[u8; 6] = b"GIF87a";
pub const MAGIC_89A: &[u8; 6] = b"GIF89a";
pub const MAGIC_LEN: usize = 6;

/// Maximum number of palette entries permitted by the GIF format.
const MAX_PALETTE_COLORS: usize = 256;
/// Fixed NeuQuant sample factor; smaller is higher quality and slower. A fixed
/// value keeps quantization deterministic across runs.
const NEUQUANT_SAMPLE_FACTOR: i32 = 10;

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

/// Encodes an image as a single-frame GIF with a deterministic, palette-quantized
/// color table of at most 256 entries.
pub fn encode(image: &Image) -> Result<Vec<u8>, ImageError> {
    let prepared = prepare_frame(image)?;
    let (gif_width, gif_height) = prepared.dimensions;

    let mut out = Vec::new();
    {
        let mut encoder =
            Encoder::new(&mut out, gif_width, gif_height, &[]).map_err(gif_encode_error)?;
        encoder
            .write_frame(&prepared.into_frame(gif_width, gif_height, 0))
            .map_err(gif_encode_error)?;
    }
    Ok(out)
}

/// Encodes a sequence of frames as an animated GIF.
///
/// Every frame must share identical dimensions; the first frame's size defines
/// the logical screen. Each frame is quantized independently with the same
/// deterministic strategy as [`encode`] and written with its own local palette,
/// so frames never compete for a shared global color table. `delay_cs` is the
/// inter-frame delay in centiseconds (1/100 s), applied uniformly to every
/// frame. `loop_count` is written as a Netscape looping extension: `0` means
/// loop forever, any other value loops that many times.
///
/// The output is byte-deterministic: encoding the same frames twice yields
/// identical bytes. Returns a clean [`ImageError`] (never panics) when there are
/// no frames, when frame dimensions disagree, or when dimensions exceed the GIF
/// 16-bit limit.
pub fn encode_animation(
    frames: &[Image],
    delay_cs: u16,
    loop_count: u16,
) -> Result<Vec<u8>, ImageError> {
    let Some((first, rest)) = frames.split_first() else {
        return Err(ImageError::UnsupportedFormat(
            "GIF animation encode failed: at least one frame is required".to_string(),
        ));
    };

    let prepared_first = prepare_frame(first)?;
    let (gif_width, gif_height) = prepared_first.dimensions;

    // Validate every other frame's dimensions before allocating the encoder so a
    // mismatch fails cleanly and deterministically.
    for frame in rest {
        let (w, h) = (frame.width(), frame.height());
        if w != u32::from(gif_width) || h != u32::from(gif_height) {
            return Err(ImageError::UnsupportedFormat(format!(
                "GIF animation encode failed: frame {w}x{h} does not match first frame {gif_width}x{gif_height}"
            )));
        }
    }

    let mut out = Vec::new();
    {
        let mut encoder =
            Encoder::new(&mut out, gif_width, gif_height, &[]).map_err(gif_encode_error)?;
        let repeat = match loop_count {
            0 => Repeat::Infinite,
            n => Repeat::Finite(n),
        };
        encoder.set_repeat(repeat).map_err(gif_encode_error)?;

        // The first frame is already prepared; reuse it before walking the rest.
        encoder
            .write_frame(&prepared_first.into_frame(gif_width, gif_height, delay_cs))
            .map_err(gif_encode_error)?;
        for frame in rest {
            let prepared = prepare_frame(frame)?;
            encoder
                .write_frame(&prepared.into_frame(gif_width, gif_height, delay_cs))
                .map_err(gif_encode_error)?;
        }
    }
    Ok(out)
}

/// A single frame quantized to a local palette and ready to hand to the `gif`
/// encoder, alongside its validated GIF dimensions.
struct PreparedFrame {
    dimensions: (u16, u16),
    quantized: Quantized,
}

impl PreparedFrame {
    fn into_frame(self, width: u16, height: u16, delay: u16) -> Frame<'static> {
        Frame {
            width,
            height,
            delay,
            transparent: self.quantized.transparent_index,
            palette: Some(self.quantized.palette),
            buffer: self.quantized.indices.into(),
            ..Frame::default()
        }
    }
}

/// Converts an image to RGBA8, validates its GIF dimensions, and quantizes it to
/// a deterministic local palette. Shared by the single-frame and animation paths.
fn prepare_frame(image: &Image) -> Result<PreparedFrame, ImageError> {
    let rgba = image.to_rgba8()?;
    let width = rgba.width();
    let height = rgba.height();
    let pixel_count = pixel_len(width, height, 1)?;
    let pixels = rgba.pixels();

    // GIF dimensions are u16; reject anything that would not fit.
    let gif_width = u16::try_from(width).map_err(|_| {
        ImageError::UnsupportedFormat(format!(
            "GIF encode failed: width {width} exceeds the 65535 pixel limit"
        ))
    })?;
    let gif_height = u16::try_from(height).map_err(|_| {
        ImageError::UnsupportedFormat(format!(
            "GIF encode failed: height {height} exceeds the 65535 pixel limit"
        ))
    })?;

    let has_transparency = pixels.chunks_exact(4).any(|px| px[3] == 0);
    let quantized = quantize(pixels, pixel_count, has_transparency)?;

    Ok(PreparedFrame {
        dimensions: (gif_width, gif_height),
        quantized,
    })
}

/// The result of mapping RGBA8 pixels onto a bounded GIF palette.
struct Quantized {
    /// Flattened `[r, g, b, ...]` palette, at most 256 colors.
    palette: Vec<u8>,
    /// One palette index per pixel.
    indices: Vec<u8>,
    /// Palette index reserved for fully transparent pixels, if any.
    transparent_index: Option<u8>,
}

fn quantize(
    pixels: &[u8],
    pixel_count: usize,
    has_transparency: bool,
) -> Result<Quantized, ImageError> {
    // The transparent index, when present, occupies one palette slot, leaving
    // the rest for opaque colors.
    let opaque_capacity = if has_transparency {
        MAX_PALETTE_COLORS - 1
    } else {
        MAX_PALETTE_COLORS
    };

    match exact_palette(pixels, pixel_count, has_transparency, opaque_capacity)? {
        Some(quantized) => Ok(quantized),
        None => neuquant_palette(pixels, pixel_count, has_transparency),
    }
}

/// Builds an exact, lossless palette when the source uses few enough distinct
/// opaque colors. Returns `None` when the color count exceeds the palette
/// budget, signalling that quantization is required.
fn exact_palette(
    pixels: &[u8],
    pixel_count: usize,
    has_transparency: bool,
    opaque_capacity: usize,
) -> Result<Option<Quantized>, ImageError> {
    // Collect distinct opaque colors in deterministic sorted order.
    let mut colors: Vec<[u8; 3]> = Vec::new();
    for px in pixels.chunks_exact(4) {
        if px[3] == 0 {
            continue;
        }
        let rgb = [px[0], px[1], px[2]];
        match colors.binary_search(&rgb) {
            Ok(_) => {}
            Err(insert_at) => {
                if colors.len() >= opaque_capacity {
                    return Ok(None);
                }
                colors.insert(insert_at, rgb);
            }
        }
    }

    // The transparent index (when present) is placed last so opaque indices are
    // stable regardless of where transparent pixels appear.
    let transparent_index = if has_transparency {
        Some(u8::try_from(colors.len()).expect("at most 255 opaque colors when transparent"))
    } else {
        None
    };

    let mut palette = try_vec_with_capacity(colors.len().saturating_mul(3).saturating_add(3))?;
    for rgb in &colors {
        palette.extend_from_slice(rgb);
    }
    if has_transparency {
        // A defined RGB triple for the transparent slot; value is irrelevant
        // because the index is flagged transparent.
        palette.extend_from_slice(&[0, 0, 0]);
    }

    let mut indices = try_vec_with_capacity(pixel_count)?;
    for px in pixels.chunks_exact(4) {
        if px[3] == 0 {
            indices.push(transparent_index.expect("transparent index present for alpha-0 pixel"));
        } else {
            let rgb = [px[0], px[1], px[2]];
            let index = colors
                .binary_search(&rgb)
                .expect("color was collected into the palette");
            indices.push(u8::try_from(index).expect("palette index fits in u8"));
        }
    }

    Ok(Some(Quantized {
        palette,
        indices,
        transparent_index,
    }))
}

/// Quantizes with NeuQuant using a fixed sample factor (deterministic) when the
/// source exceeds the exact-palette budget.
fn neuquant_palette(
    pixels: &[u8],
    pixel_count: usize,
    has_transparency: bool,
) -> Result<Quantized, ImageError> {
    // NeuQuant trains on opaque pixels only; transparent pixels are excluded so
    // they do not pull palette colors toward the placeholder.
    let mut training = try_vec_with_capacity(pixel_count.saturating_mul(4))?;
    for px in pixels.chunks_exact(4) {
        if px[3] != 0 {
            training.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }
    // NeuQuant requires a non-empty sample; if every pixel is transparent, fall
    // back to a single black training pixel.
    if training.is_empty() {
        training.extend_from_slice(&[0, 0, 0, 255]);
    }

    let color_budget = if has_transparency {
        MAX_PALETTE_COLORS - 1
    } else {
        MAX_PALETTE_COLORS
    };
    let nq = NeuQuant::new(NEUQUANT_SAMPLE_FACTOR, color_budget, &training);

    let color_map = nq.color_map_rgb();
    let opaque_colors = color_map.len() / 3;
    let transparent_index = if has_transparency {
        Some(u8::try_from(opaque_colors).expect("at most 255 opaque colors when transparent"))
    } else {
        None
    };

    let mut palette = try_vec_with_capacity(color_map.len().saturating_add(3))?;
    palette.extend_from_slice(&color_map);
    if has_transparency {
        palette.extend_from_slice(&[0, 0, 0]);
    }

    let mut indices = try_vec_with_capacity(pixel_count)?;
    for px in pixels.chunks_exact(4) {
        if px[3] == 0 {
            indices.push(transparent_index.expect("transparent index present for alpha-0 pixel"));
        } else {
            let index = nq.index_of(&[px[0], px[1], px[2], 255]);
            indices.push(u8::try_from(index).expect("NeuQuant index fits in u8"));
        }
    }

    Ok(Quantized {
        palette,
        indices,
        transparent_index,
    })
}

fn gif_encode_error(err: EncodingError) -> ImageError {
    ImageError::UnsupportedFormat(format!("GIF encode failed: {err}"))
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

    fn solid_image(width: u32, height: u32, rgba: [u8; 4]) -> Image {
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            pixels.extend_from_slice(&rgba);
        }
        Image::new(width, height, PixelFormat::Rgba8, pixels).unwrap()
    }

    #[test]
    fn encode_animation_round_trips_three_frames() {
        let frames = [
            solid_image(2, 2, [255, 0, 0, 255]),
            solid_image(2, 2, [0, 255, 0, 255]),
            solid_image(2, 2, [0, 0, 255, 255]),
        ];
        let gif = encode_animation(&frames, 50, 0).unwrap();
        assert_eq!(&gif[..MAGIC_LEN], MAGIC_89A);
        assert_eq!(frame_count(&gif).unwrap(), 3);

        assert_eq!(
            decode_frame(&gif, 0).unwrap().pixels(),
            &[255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255]
        );
        assert_eq!(
            decode_frame(&gif, 1).unwrap().pixels(),
            &[0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255]
        );
        assert_eq!(
            decode_frame(&gif, 2).unwrap().pixels(),
            &[0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255]
        );
    }

    #[test]
    fn encode_animation_is_deterministic() {
        let frames = [
            solid_image(3, 1, [10, 20, 30, 255]),
            solid_image(3, 1, [40, 50, 60, 255]),
        ];
        let a = encode_animation(&frames, 25, 3).unwrap();
        let b = encode_animation(&frames, 25, 3).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn encode_animation_single_frame() {
        let frames = [solid_image(2, 2, [1, 2, 3, 255])];
        let gif = encode_animation(&frames, 10, 0).unwrap();
        assert_eq!(frame_count(&gif).unwrap(), 1);
        assert_eq!(
            decode_frame(&gif, 0).unwrap().pixels(),
            &[1, 2, 3, 255, 1, 2, 3, 255, 1, 2, 3, 255, 1, 2, 3, 255]
        );
    }

    #[test]
    fn encode_animation_rejects_empty() {
        let err = encode_animation(&[], 10, 0).unwrap_err().to_string();
        assert!(err.contains("at least one frame"), "{err}");
    }

    #[test]
    fn encode_animation_rejects_dimension_mismatch() {
        let frames = [
            solid_image(2, 2, [255, 0, 0, 255]),
            solid_image(3, 2, [0, 255, 0, 255]),
        ];
        let err = encode_animation(&frames, 10, 0).unwrap_err().to_string();
        assert!(err.contains("does not match first frame"), "{err}");
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
