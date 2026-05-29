//! farbfeld decoding and encoding for the `imx` image toolkit.
//!
//! `imx-codec-farbfeld` reads and writes the suckless [farbfeld] format: a
//! trivially simple, lossless 16-bit-per-channel RGBA container. It produces
//! and consumes the format-agnostic [`imx_core::Image`] type shared across the
//! workspace. Decoding is memory-safe and deterministic: the fixed-size header
//! is validated and the pixel buffer is bounded by the `imx-core` allocation
//! limits, so malformed or hostile inputs cannot trigger uncontrolled
//! allocation. Round-trips are differentially verified against the real
//! ImageMagick binary as an oracle.
//!
//! [farbfeld]: https://tools.suckless.org/farbfeld/

use imx_core::{pixel_len, try_vec_with_capacity, Format, Image, ImageError, PixelFormat};

pub const MAGIC: &[u8; 8] = b"farbfeld";
pub const HEADER_LEN: usize = 16;
pub const BYTES_PER_PIXEL: usize = 8;

pub fn decode_header(input: &[u8]) -> Result<(u32, u32), ImageError> {
    if input.len() < HEADER_LEN {
        return Err(ImageError::UnexpectedEof {
            expected: HEADER_LEN,
            actual: input.len(),
        });
    }
    if &input[..MAGIC.len()] != MAGIC {
        return Err(ImageError::InvalidHeader("FARBFELD"));
    }

    let width = u32::from_be_bytes(input[8..12].try_into().expect("fixed width slice"));
    let height = u32::from_be_bytes(input[12..16].try_into().expect("fixed width slice"));
    let _ = pixel_len(width, height, BYTES_PER_PIXEL)?;
    Ok((width, height))
}

pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    let (width, height) = decode_header(input)?;
    let payload_len = pixel_len(width, height, BYTES_PER_PIXEL)?;
    let expected_len = HEADER_LEN
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    if input.len() < expected_len {
        return Err(ImageError::UnexpectedEof {
            expected: expected_len,
            actual: input.len(),
        });
    }
    Image::new(
        width,
        height,
        PixelFormat::Rgba16Be,
        input[HEADER_LEN..expected_len].to_vec(),
    )
}

pub fn identify(input: &[u8]) -> Result<imx_core::Identify, ImageError> {
    let (width, height) = decode_header(input)?;
    Ok(imx_core::Identify {
        format: Format::Farbfeld,
        width,
        height,
        pixel_format: PixelFormat::Rgba16Be,
    })
}

pub fn encode(image: &Image) -> Result<Vec<u8>, ImageError> {
    let farbfeld = image.to_rgba16be()?;
    let payload_len = pixel_len(farbfeld.width(), farbfeld.height(), BYTES_PER_PIXEL)?;
    if farbfeld.pixels().len() != payload_len {
        return Err(ImageError::InvalidPixelBuffer {
            expected: payload_len,
            actual: farbfeld.pixels().len(),
        });
    }

    let capacity = HEADER_LEN
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = try_vec_with_capacity(capacity)?;
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&farbfeld.width().to_be_bytes());
    out.extend_from_slice(&farbfeld.height().to_be_bytes());
    out.extend_from_slice(farbfeld.pixels());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_by_one_red_half_alpha() -> Vec<u8> {
        [
            MAGIC.as_slice(),
            &1_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
            &[0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00],
        ]
        .concat()
    }

    #[test]
    fn decodes_golden_rgba16be_fixture() {
        let image = decode(&one_by_one_red_half_alpha()).unwrap();
        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
        assert_eq!(image.pixel_format(), PixelFormat::Rgba16Be);
        assert_eq!(
            image.pixels(),
            &[0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00]
        );
    }

    #[test]
    fn encodes_golden_rgba16be_fixture() {
        let image = Image::new(
            1,
            1,
            PixelFormat::Rgba16Be,
            vec![0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00],
        )
        .unwrap();
        assert_eq!(encode(&image).unwrap(), one_by_one_red_half_alpha());
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = one_by_one_red_half_alpha();
        bytes[0] = b'F';
        assert_eq!(decode(&bytes), Err(ImageError::InvalidHeader("FARBFELD")));
    }

    #[test]
    fn rejects_truncated_payload() {
        let mut bytes = one_by_one_red_half_alpha();
        bytes.pop();
        assert_eq!(
            decode(&bytes),
            Err(ImageError::UnexpectedEof {
                expected: 24,
                actual: 23
            })
        );
    }
}
