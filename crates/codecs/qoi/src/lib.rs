use imx_core::{
    pixel_count, pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError, PixelFormat,
};

pub const MAGIC: &[u8; 4] = b"qoif";
pub const HEADER_LEN: usize = 14;
pub const END_MARKER: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];
pub const QOI_SRGB: u8 = 0;
pub const QOI_LINEAR: u8 = 1;
pub const QOI_OP_INDEX: u8 = 0x00;
pub const QOI_OP_DIFF: u8 = 0x40;
pub const QOI_OP_LUMA: u8 = 0x80;
pub const QOI_OP_RUN: u8 = 0xc0;
pub const QOI_OP_RGB: u8 = 0xfe;
pub const QOI_OP_RGBA: u8 = 0xff;
pub const QOI_MASK_2: u8 = 0xc0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QoiImage {
    pub width: u32,
    pub height: u32,
    pub channels: u8,
    pub colorspace: u8,
    pub pixels: Vec<u8>,
}

impl QoiImage {
    pub fn into_core_image(self) -> Result<Image, ImageError> {
        let pixel_format = if self.channels == 3 {
            PixelFormat::Rgb8
        } else {
            PixelFormat::Rgba8
        };
        Image::new(self.width, self.height, pixel_format, self.pixels)
    }

    pub fn identify(&self) -> Identify {
        Identify {
            format: Format::Qoi,
            width: self.width,
            height: self.height,
            pixel_format: if self.channels == 3 {
                PixelFormat::Rgb8
            } else {
                PixelFormat::Rgba8
            },
        }
    }
}

pub fn decode_header(input: &[u8]) -> Result<(u32, u32, u8, u8), ImageError> {
    if input.len() < HEADER_LEN {
        return Err(ImageError::UnexpectedEof {
            expected: HEADER_LEN,
            actual: input.len(),
        });
    }
    if !input[..MAGIC.len()].eq_ignore_ascii_case(MAGIC) {
        return Err(ImageError::InvalidHeader("QOI"));
    }

    let width = u32::from_be_bytes(input[4..8].try_into().expect("fixed width slice"));
    let height = u32::from_be_bytes(input[8..12].try_into().expect("fixed width slice"));
    let channels = input[12];
    let colorspace = input[13];
    validate_channels(channels)?;
    validate_colorspace(colorspace)?;
    let _ = pixel_len(width, height, channels as usize)?;
    Ok((width, height, channels, colorspace))
}

pub fn identify(input: &[u8]) -> Result<Identify, ImageError> {
    let (width, height, channels, _) = decode_header(input)?;
    Ok(Identify {
        format: Format::Qoi,
        width,
        height,
        pixel_format: if channels == 3 {
            PixelFormat::Rgb8
        } else {
            PixelFormat::Rgba8
        },
    })
}

pub fn decode(input: &[u8]) -> Result<QoiImage, ImageError> {
    let (width, height, channels, colorspace) = decode_header(input)?;
    let pixels = pixel_count(width, height)?;
    let minimum_payload = minimum_payload_len(pixels);
    if input.len() - HEADER_LEN < minimum_payload {
        return Err(ImageError::UnexpectedEof {
            expected: HEADER_LEN
                .checked_add(minimum_payload)
                .ok_or(ImageError::LengthOverflow)?,
            actual: input.len(),
        });
    }

    let capacity = pixel_len(width, height, channels as usize)?;
    let mut decoded = try_vec_with_capacity(capacity)?;
    let mut index = [[0_u8; 4]; 64];
    let mut px = [0_u8, 0, 0, 255];
    let mut offset = HEADER_LEN;
    let mut written = 0_usize;

    while written < pixels {
        let b = read_byte(input, &mut offset)?;
        let mut run = 0_usize;

        if b == QOI_OP_RGB {
            px[0] = read_byte(input, &mut offset)?;
            px[1] = read_byte(input, &mut offset)?;
            px[2] = read_byte(input, &mut offset)?;
        } else if b == QOI_OP_RGBA {
            px[0] = read_byte(input, &mut offset)?;
            px[1] = read_byte(input, &mut offset)?;
            px[2] = read_byte(input, &mut offset)?;
            px[3] = read_byte(input, &mut offset)?;
        } else if (b & QOI_MASK_2) == QOI_OP_INDEX {
            px = index[(b & !QOI_MASK_2) as usize];
        } else if (b & QOI_MASK_2) == QOI_OP_DIFF {
            px[0] = px[0].wrapping_add(((b >> 4) & 0x03).wrapping_sub(2));
            px[1] = px[1].wrapping_add(((b >> 2) & 0x03).wrapping_sub(2));
            px[2] = px[2].wrapping_add((b & 0x03).wrapping_sub(2));
        } else if (b & QOI_MASK_2) == QOI_OP_LUMA {
            let b2 = read_byte(input, &mut offset)?;
            let vg = (b & !QOI_MASK_2).wrapping_sub(32);
            px[0] = px[0].wrapping_add(vg.wrapping_sub(8).wrapping_add((b2 >> 4) & 0x0f));
            px[1] = px[1].wrapping_add(vg);
            px[2] = px[2].wrapping_add(vg.wrapping_sub(8).wrapping_add(b2 & 0x0f));
        } else if (b & QOI_MASK_2) == QOI_OP_RUN {
            run = (b & !QOI_MASK_2) as usize;
        }

        index[color_hash(px)] = px;
        for _ in 0..=run {
            if written < pixels {
                decoded.extend_from_slice(&px[..channels as usize]);
            }
            written += 1;
        }
    }

    Ok(QoiImage {
        width,
        height,
        channels,
        colorspace,
        pixels: decoded,
    })
}

pub fn encode_image(image: &Image, colorspace: u8) -> Result<Vec<u8>, ImageError> {
    let rgba = image.to_rgba8()?;
    encode(rgba.width(), rgba.height(), 4, colorspace, rgba.pixels())
}

pub fn encode(
    width: u32,
    height: u32,
    channels: u8,
    colorspace: u8,
    pixels: &[u8],
) -> Result<Vec<u8>, ImageError> {
    validate_channels(channels)?;
    validate_colorspace(colorspace)?;
    let pixels_len = pixel_len(width, height, channels as usize)?;
    if pixels.len() != pixels_len {
        return Err(ImageError::InvalidPixelBuffer {
            expected: pixels_len,
            actual: pixels.len(),
        });
    }
    let pixel_count = pixel_count(width, height)?;
    let capacity = HEADER_LEN
        .checked_add(pixels_len)
        .and_then(|value| value.checked_add(END_MARKER.len()))
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = try_vec_with_capacity(capacity)?;
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&height.to_be_bytes());
    out.push(channels);
    out.push(colorspace);

    let mut index = [[0_u8; 4]; 64];
    let mut px = [0_u8, 0, 0, 255];
    let mut run = 0_u8;
    let channels_len = channels as usize;

    for i in 0..pixel_count {
        let previous = px;
        let start = i * channels_len;
        px[0] = pixels[start];
        px[1] = pixels[start + 1];
        px[2] = pixels[start + 2];
        if channels == 4 {
            px[3] = pixels[start + 3];
        }

        if previous == px {
            run += 1;
            if run == 62 {
                out.push(QOI_OP_RUN | (run - 1));
                run = 0;
            }
            continue;
        }

        if run > 0 {
            out.push(QOI_OP_RUN | (run - 1));
            run = 0;
        }

        let index_pos = color_hash(px);
        if index[index_pos] == px {
            out.push(QOI_OP_INDEX | index_pos as u8);
            continue;
        }
        index[index_pos] = px;

        if previous[3] == px[3] {
            let vr = px[0].wrapping_sub(previous[0]) as i8;
            let vg = px[1].wrapping_sub(previous[1]) as i8;
            let vb = px[2].wrapping_sub(previous[2]) as i8;
            let vg_r = vr.wrapping_sub(vg);
            let vg_b = vb.wrapping_sub(vg);

            if (-2..=1).contains(&vr) && (-2..=1).contains(&vg) && (-2..=1).contains(&vb) {
                out.push(
                    QOI_OP_DIFF
                        | (((vr + 2) as u8) << 4)
                        | (((vg + 2) as u8) << 2)
                        | ((vb + 2) as u8),
                );
            } else if (-8..=7).contains(&vg_r)
                && (-32..=31).contains(&vg)
                && (-8..=7).contains(&vg_b)
            {
                out.push(QOI_OP_LUMA | ((vg + 32) as u8));
                out.push((((vg_r + 8) as u8) << 4) | ((vg_b + 8) as u8));
            } else {
                out.push(QOI_OP_RGB);
                out.extend_from_slice(&px[..3]);
            }
        } else {
            out.push(QOI_OP_RGBA);
            out.extend_from_slice(&px);
        }
    }

    if run > 0 {
        out.push(QOI_OP_RUN | (run - 1));
    }
    out.extend_from_slice(&END_MARKER);
    Ok(out)
}

pub fn minimum_payload_len(pixel_count: usize) -> usize {
    debug_assert!(pixel_count > 0);
    ((pixel_count - 1) / 62) + 1
}

fn validate_channels(channels: u8) -> Result<(), ImageError> {
    match channels {
        3 | 4 => Ok(()),
        _ => Err(ImageError::InvalidChannels { channels }),
    }
}

fn validate_colorspace(colorspace: u8) -> Result<(), ImageError> {
    match colorspace {
        QOI_SRGB | QOI_LINEAR => Ok(()),
        _ => Err(ImageError::InvalidColorspace { colorspace }),
    }
}

fn read_byte(input: &[u8], offset: &mut usize) -> Result<u8, ImageError> {
    if *offset >= input.len() {
        return Err(ImageError::UnexpectedEof {
            expected: (*offset).checked_add(1).ok_or(ImageError::LengthOverflow)?,
            actual: input.len(),
        });
    }
    let byte = input[*offset];
    *offset += 1;
    Ok(byte)
}

fn color_hash(px: [u8; 4]) -> usize {
    ((px[0] as usize * 3) + (px[1] as usize * 5) + (px[2] as usize * 7) + (px[3] as usize * 11))
        % 64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_minimal_rgb_fixture_without_requiring_end_marker() {
        let bytes = [
            MAGIC.as_slice(),
            &1_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
            &[3, QOI_SRGB, QOI_OP_RGB, 0x10, 0x20, 0x30],
        ]
        .concat();

        let image = decode(&bytes).unwrap();
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.channels, 3);
        assert_eq!(image.colorspace, QOI_SRGB);
        assert_eq!(image.pixels, [0x10, 0x20, 0x30]);
    }

    #[test]
    fn encodes_known_rgba_fixture_deterministically() {
        let pixels = [
            0xff, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0xff, 0xfe, 0x01, 0x02, 0x03, 0xfe, 0x01,
            0x02, 0x04,
        ];
        let encoded = encode(4, 1, 4, QOI_LINEAR, &pixels).unwrap();
        let expected = [
            MAGIC.as_slice(),
            &4_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
            &[4, QOI_LINEAR],
            &[0x5a],
            &[0xc0],
            &[0xff, 0xfe, 0x01, 0x02, 0x03],
            &[0xff, 0xfe, 0x01, 0x02, 0x04],
            &END_MARKER,
        ]
        .concat();
        assert_eq!(encoded, expected);
        assert_eq!(decode(&encoded).unwrap().pixels, pixels);
    }

    #[test]
    fn round_trips_rgb_and_rgba_channels() {
        let rgb = [0, 0, 0, 1, 2, 3, 1, 2, 3, 255, 128, 64];
        let rgba = [0, 0, 0, 0, 1, 2, 3, 4, 1, 2, 3, 4, 255, 128, 64, 32];

        for (channels, pixels) in [(3, rgb.as_slice()), (4, rgba.as_slice())] {
            let encoded = encode(2, 2, channels, QOI_SRGB, pixels).unwrap();
            let decoded = decode(&encoded).unwrap();
            assert_eq!(decoded.channels, channels);
            assert_eq!(decoded.pixels, pixels);
        }
    }

    #[test]
    fn encode_rejects_invalid_channels_without_panicking() {
        for channels in [0, 1, 2, 5] {
            assert_eq!(
                encode(1, 1, channels, QOI_SRGB, &[]),
                Err(ImageError::InvalidChannels { channels })
            );
        }
    }
}
