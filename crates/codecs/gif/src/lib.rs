//! Bounded GIF input decoding and single-frame output encoding.
//!
//! Decoding reads only the first frame. Animation and multi-frame GIFs are out
//! of scope for this crate: any frames after the first are ignored, and frame
//! delays, disposal methods, and loop counts are not interpreted. The first
//! frame is composited onto the logical screen canvas as RGBA8, so decode and
//! identify always agree on the reported dimensions.
//!
//! Encoding writes a single still frame with a palette of at most 256 colors.
//! Quantization is deterministic: when the source uses at most 256 distinct
//! colors the exact palette is preserved, otherwise a NeuQuant color map with a
//! fixed sample factor is used. Fully transparent pixels (alpha 0) are mapped to
//! a single reserved transparent palette index; if there are none, no
//! transparency is emitted. Animation output is explicitly out of scope.

use std::io::Cursor;

use color_quant::NeuQuant;
use gif::{ColorOutput, DecodeOptions, DecodingError, Encoder, EncodingError, Frame, MemoryLimit};
use imx_core::{
    pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError, PixelFormat,
    MAX_PIXEL_BYTES,
};

/// Maximum number of palette entries permitted by the GIF format.
const MAX_PALETTE_COLORS: usize = 256;
/// Fixed NeuQuant sample factor; smaller is higher quality and slower. A fixed
/// value keeps quantization deterministic across runs.
const NEUQUANT_SAMPLE_FACTOR: i32 = 10;

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

/// Encodes an image as a single-frame GIF with a deterministic, palette-quantized
/// color table of at most 256 entries.
pub fn encode(image: &Image) -> Result<Vec<u8>, ImageError> {
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

    let mut out = Vec::new();
    {
        let mut encoder = Encoder::new(&mut out, gif_width, gif_height, &quantized.palette)
            .map_err(gif_encode_error)?;
        let frame = Frame {
            width: gif_width,
            height: gif_height,
            transparent: quantized.transparent_index,
            buffer: quantized.indices.into(),
            ..Frame::default()
        };
        encoder.write_frame(&frame).map_err(gif_encode_error)?;
    }
    Ok(out)
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

    #[test]
    fn encodes_rgb8_round_trips_exactly() {
        // A small known palette (< 256 colors) is preserved losslessly.
        let pixels = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 10, 20, 30];
        let image = Image::new(2, 2, PixelFormat::Rgb8, pixels).unwrap();
        let gif = encode(&image).unwrap();
        assert_eq!(&gif[..MAGIC_LEN], MAGIC_89A);
        assert_eq!(
            identify(&gif).unwrap().stable_line(),
            "format=GIF width=2 height=2 channels=RGBA depth=8"
        );
        let decoded = decode(&gif).unwrap();
        assert_eq!(decoded.width(), 2);
        assert_eq!(decoded.height(), 2);
        assert_eq!(decoded.pixel_format(), PixelFormat::Rgba8);
        assert_eq!(
            decoded.pixels(),
            &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 10, 20, 30, 255]
        );
    }

    #[test]
    fn encodes_rgba8_opaque_round_trips_exactly() {
        let pixels = vec![10, 20, 30, 255, 40, 50, 60, 255];
        let image = Image::new(2, 1, PixelFormat::Rgba8, pixels.clone()).unwrap();
        let gif = encode(&image).unwrap();
        let decoded = decode(&gif).unwrap();
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
    fn encode_maps_transparent_pixels_to_a_palette_index() {
        // One fully transparent pixel, two opaque colors.
        let pixels = vec![255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 255, 255];
        let image = Image::new(3, 1, PixelFormat::Rgba8, pixels).unwrap();
        let gif = encode(&image).unwrap();
        let decoded = decode(&gif).unwrap();
        assert_eq!(decoded.width(), 3);
        let out = decoded.pixels();
        // Opaque colors survive exactly.
        assert_eq!(&out[0..4], &[255, 0, 0, 255]);
        assert_eq!(&out[8..12], &[0, 0, 255, 255]);
        // The transparent pixel decodes back to alpha 0.
        assert_eq!(out[7], 0);
    }

    #[test]
    fn encode_without_transparency_has_no_transparent_index() {
        let image = Image::new(2, 1, PixelFormat::Rgb8, vec![1, 2, 3, 4, 5, 6]).unwrap();
        let gif = encode(&image).unwrap();
        let decoded = decode(&gif).unwrap();
        // Every output pixel is opaque.
        assert!(decoded.pixels().chunks_exact(4).all(|px| px[3] == 255));
    }

    #[test]
    fn encode_quantizes_many_colors_deterministically() {
        // 400 distinct colors forces the NeuQuant path.
        let mut pixels = Vec::new();
        for i in 0..400u32 {
            pixels.push((i & 0xff) as u8);
            pixels.push(((i >> 1) & 0xff) as u8);
            pixels.push(((i >> 2) & 0xff) as u8);
        }
        let image = Image::new(20, 20, PixelFormat::Rgb8, pixels).unwrap();
        let gif = encode(&image).unwrap();
        assert_eq!(
            identify(&gif).unwrap().stable_line(),
            "format=GIF width=20 height=20 channels=RGBA depth=8"
        );
        // Same input, byte-identical output.
        assert_eq!(gif, encode(&image).unwrap());
    }
}
