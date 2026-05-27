use imx_core::{
    pixel_len, try_vec_with_capacity, Format, Identify, Image, ImageError, PixelFormat,
};

pub const MAGIC: &[u8; 2] = b"BM";

const FILE_HEADER_LEN: usize = 14;
const BITMAPINFOHEADER_LEN: usize = 40;
const BI_RGB: u32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BmpHeader {
    width: u32,
    height: u32,
    top_down: bool,
    bits_per_pixel: u16,
    pixel_offset: usize,
    row_stride: usize,
    pixel_format: PixelFormat,
}

pub fn identify(input: &[u8]) -> Result<Identify, ImageError> {
    let header = decode_header(input)?;
    Ok(Identify {
        format: Format::Bmp,
        width: header.width,
        height: header.height,
        pixel_format: header.pixel_format,
    })
}

pub fn decode(input: &[u8]) -> Result<Image, ImageError> {
    let header = decode_header(input)?;
    let output_len = pixel_len(
        header.width,
        header.height,
        header.pixel_format.bytes_per_pixel(),
    )?;
    let mut out = try_vec_with_capacity(output_len)?;

    let width = usize::try_from(header.width).map_err(|_| ImageError::LengthOverflow)?;
    let height = usize::try_from(header.height).map_err(|_| ImageError::LengthOverflow)?;
    let bytes_per_disk_pixel = usize::from(header.bits_per_pixel / 8);

    for y in 0..height {
        let source_y = if header.top_down { y } else { height - 1 - y };
        let row_offset = header
            .pixel_offset
            .checked_add(
                source_y
                    .checked_mul(header.row_stride)
                    .ok_or(ImageError::LengthOverflow)?,
            )
            .ok_or(ImageError::LengthOverflow)?;
        for x in 0..width {
            let pixel_offset = row_offset
                .checked_add(
                    x.checked_mul(bytes_per_disk_pixel)
                        .ok_or(ImageError::LengthOverflow)?,
                )
                .ok_or(ImageError::LengthOverflow)?;
            match header.pixel_format {
                PixelFormat::Rgb8 => {
                    let blue = input[pixel_offset];
                    let green = input[pixel_offset + 1];
                    let red = input[pixel_offset + 2];
                    out.extend_from_slice(&[red, green, blue]);
                }
                PixelFormat::Rgba8 => {
                    let blue = input[pixel_offset];
                    let green = input[pixel_offset + 1];
                    let red = input[pixel_offset + 2];
                    let alpha = input[pixel_offset + 3];
                    out.extend_from_slice(&[red, green, blue, alpha]);
                }
                _ => unreachable!("BMP decoder only produces RGB8/RGBA8"),
            }
        }
    }

    Image::new(header.width, header.height, header.pixel_format, out)
}

pub fn encode(image: &Image) -> Result<Vec<u8>, ImageError> {
    match image.pixel_format() {
        PixelFormat::Rgba8 | PixelFormat::Rgba16Be => encode_bgra32(&image.to_rgba8()?),
        PixelFormat::Bilevel
        | PixelFormat::Gray8
        | PixelFormat::Gray16Be
        | PixelFormat::Rgb8
        | PixelFormat::Rgb16Be => encode_bgr24(&image.to_rgb8()?),
    }
}

fn decode_header(input: &[u8]) -> Result<BmpHeader, ImageError> {
    if input.len() < FILE_HEADER_LEN {
        return Err(ImageError::UnexpectedEof {
            expected: FILE_HEADER_LEN,
            actual: input.len(),
        });
    }
    if &input[..MAGIC.len()] != MAGIC {
        return Err(ImageError::InvalidHeader("BMP"));
    }

    let declared_file_size = match read_u32(input, 2)? {
        0 => None,
        size => Some(usize::try_from(size).map_err(|_| ImageError::LengthOverflow)?),
    };
    if let Some(file_size) = declared_file_size {
        if file_size > input.len() {
            return Err(ImageError::UnexpectedEof {
                expected: file_size,
                actual: input.len(),
            });
        }
    }

    let pixel_offset =
        usize::try_from(read_u32(input, 10)?).map_err(|_| ImageError::LengthOverflow)?;
    let dib_header_len =
        usize::try_from(read_u32(input, 14)?).map_err(|_| ImageError::LengthOverflow)?;
    if dib_header_len != BITMAPINFOHEADER_LEN {
        return Err(ImageError::UnsupportedFormat(
            "BMP DIB header must be BITMAPINFOHEADER".to_string(),
        ));
    }
    let header_end = FILE_HEADER_LEN
        .checked_add(dib_header_len)
        .ok_or(ImageError::LengthOverflow)?;
    if input.len() < header_end {
        return Err(ImageError::UnexpectedEof {
            expected: header_end,
            actual: input.len(),
        });
    }
    if pixel_offset < header_end {
        return Err(ImageError::InvalidHeader("BMP"));
    }

    let width = read_i32(input, 18)?;
    let height = read_i32(input, 22)?;
    if width <= 0 || height == 0 || height == i32::MIN {
        return Err(ImageError::InvalidDimensions);
    }
    let width = u32::try_from(width).map_err(|_| ImageError::InvalidDimensions)?;
    let top_down = height < 0;
    let height = height.unsigned_abs();

    let planes = read_u16(input, 26)?;
    if planes != 1 {
        return Err(ImageError::UnsupportedFormat(
            "BMP planes must be 1".to_string(),
        ));
    }
    let bits_per_pixel = read_u16(input, 28)?;
    let compression = read_u32(input, 30)?;
    if compression != BI_RGB {
        return Err(ImageError::UnsupportedFormat(
            "BMP compression is not supported".to_string(),
        ));
    }
    let colors_used = read_u32(input, 46)?;
    if colors_used != 0 {
        return Err(ImageError::UnsupportedFormat(
            "BMP color tables are not supported".to_string(),
        ));
    }

    let pixel_format = match bits_per_pixel {
        24 => PixelFormat::Rgb8,
        32 => PixelFormat::Rgba8,
        _ => {
            return Err(ImageError::UnsupportedFormat(format!(
                "BMP bit depth {bits_per_pixel} is not supported"
            )))
        }
    };
    let _ = pixel_len(width, height, pixel_format.bytes_per_pixel())?;
    let row_stride = bmp_row_stride(width, usize::from(bits_per_pixel / 8))?;
    let rows = usize::try_from(height).map_err(|_| ImageError::LengthOverflow)?;
    let payload_len = row_stride
        .checked_mul(rows)
        .ok_or(ImageError::LengthOverflow)?;
    let required_len = pixel_offset
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    if let Some(file_size) = declared_file_size {
        if file_size < required_len {
            return Err(ImageError::UnexpectedEof {
                expected: required_len,
                actual: file_size,
            });
        }
    }
    if input.len() < required_len {
        return Err(ImageError::UnexpectedEof {
            expected: required_len,
            actual: input.len(),
        });
    }

    Ok(BmpHeader {
        width,
        height,
        top_down,
        bits_per_pixel,
        pixel_offset,
        row_stride,
        pixel_format,
    })
}

fn encode_bgr24(image: &Image) -> Result<Vec<u8>, ImageError> {
    let row_stride = bmp_row_stride(image.width(), 3)?;
    let rows = usize::try_from(image.height()).map_err(|_| ImageError::LengthOverflow)?;
    let payload_len = row_stride
        .checked_mul(rows)
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = bmp_header(image.width(), image.height(), 24, payload_len)?;
    out.resize(out.len() + payload_len, 0);
    let pixel_offset = FILE_HEADER_LEN + BITMAPINFOHEADER_LEN;
    let width = usize::try_from(image.width()).map_err(|_| ImageError::LengthOverflow)?;

    for y in 0..rows {
        let source_y = rows - 1 - y;
        let row_offset = pixel_offset + y * row_stride;
        for x in 0..width {
            let source_offset = (source_y * width + x) * 3;
            let target_offset = row_offset + x * 3;
            let pixel = &image.pixels()[source_offset..source_offset + 3];
            out[target_offset] = pixel[2];
            out[target_offset + 1] = pixel[1];
            out[target_offset + 2] = pixel[0];
        }
    }

    Ok(out)
}

fn encode_bgra32(image: &Image) -> Result<Vec<u8>, ImageError> {
    let row_stride = bmp_row_stride(image.width(), 4)?;
    let rows = usize::try_from(image.height()).map_err(|_| ImageError::LengthOverflow)?;
    let payload_len = row_stride
        .checked_mul(rows)
        .ok_or(ImageError::LengthOverflow)?;
    let mut out = bmp_header(image.width(), image.height(), 32, payload_len)?;
    let pixel_offset = FILE_HEADER_LEN + BITMAPINFOHEADER_LEN;
    out.resize(pixel_offset + payload_len, 0);
    let width = usize::try_from(image.width()).map_err(|_| ImageError::LengthOverflow)?;

    for y in 0..rows {
        let source_y = rows - 1 - y;
        let row_offset = pixel_offset + y * row_stride;
        for x in 0..width {
            let source_offset = (source_y * width + x) * 4;
            let target_offset = row_offset + x * 4;
            let pixel = &image.pixels()[source_offset..source_offset + 4];
            out[target_offset] = pixel[2];
            out[target_offset + 1] = pixel[1];
            out[target_offset + 2] = pixel[0];
            out[target_offset + 3] = pixel[3];
        }
    }

    Ok(out)
}

fn bmp_header(
    width: u32,
    height: u32,
    bits_per_pixel: u16,
    payload_len: usize,
) -> Result<Vec<u8>, ImageError> {
    let pixel_offset = FILE_HEADER_LEN + BITMAPINFOHEADER_LEN;
    let file_size = pixel_offset
        .checked_add(payload_len)
        .ok_or(ImageError::LengthOverflow)?;
    let file_size = u32::try_from(file_size).map_err(|_| ImageError::LengthOverflow)?;
    let signed_width = i32::try_from(width).map_err(|_| ImageError::LengthOverflow)?;
    let signed_height = i32::try_from(height).map_err(|_| ImageError::LengthOverflow)?;

    let mut out = try_vec_with_capacity(file_size as usize)?;
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
    out.extend_from_slice(&(BITMAPINFOHEADER_LEN as u32).to_le_bytes());
    out.extend_from_slice(&signed_width.to_le_bytes());
    out.extend_from_slice(&signed_height.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&bits_per_pixel.to_le_bytes());
    out.extend_from_slice(&BI_RGB.to_le_bytes());
    out.extend_from_slice(&(payload_len as u32).to_le_bytes());
    out.extend_from_slice(&0_i32.to_le_bytes());
    out.extend_from_slice(&0_i32.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    Ok(out)
}

fn bmp_row_stride(width: u32, bytes_per_pixel: usize) -> Result<usize, ImageError> {
    let width = usize::try_from(width).map_err(|_| ImageError::LengthOverflow)?;
    let row_bytes = width
        .checked_mul(bytes_per_pixel)
        .ok_or(ImageError::LengthOverflow)?;
    row_bytes
        .checked_add(3)
        .map(|len| len & !3)
        .ok_or(ImageError::LengthOverflow)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, ImageError> {
    let end = offset.checked_add(2).ok_or(ImageError::LengthOverflow)?;
    let bytes = input.get(offset..end).ok_or(ImageError::UnexpectedEof {
        expected: end,
        actual: input.len(),
    })?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32, ImageError> {
    let end = offset.checked_add(4).ok_or(ImageError::LengthOverflow)?;
    let bytes = input.get(offset..end).ok_or(ImageError::UnexpectedEof {
        expected: end,
        actual: input.len(),
    })?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_i32(input: &[u8], offset: usize) -> Result<i32, ImageError> {
    let end = offset.checked_add(4).ok_or(ImageError::LengthOverflow)?;
    let bytes = input.get(offset..end).ok_or(ImageError::UnexpectedEof {
        expected: end,
        actual: input.len(),
    })?;
    Ok(i32::from_le_bytes(bytes.try_into().unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_rgb24_bmp_with_padding() {
        let image = Image::new(
            3,
            2,
            PixelFormat::Rgb8,
            vec![
                255, 0, 0, 0, 255, 0, 0, 0, 255, 12, 34, 56, 78, 90, 123, 222, 111, 3,
            ],
        )
        .unwrap();
        let bmp = encode(&image).unwrap();
        assert_eq!(&bmp[..MAGIC.len()], MAGIC);
        assert_eq!(
            identify(&bmp).unwrap().stable_line(),
            "format=BMP width=3 height=2 channels=RGB depth=8"
        );
        assert_eq!(decode(&bmp).unwrap(), image);
    }

    #[test]
    fn encodes_and_decodes_bgra32_bmp_with_alpha() {
        let image = Image::new(
            2,
            1,
            PixelFormat::Rgba8,
            vec![255, 0, 0, 128, 0, 64, 255, 12],
        )
        .unwrap();
        let bmp = encode(&image).unwrap();
        assert_eq!(
            identify(&bmp).unwrap().stable_line(),
            "format=BMP width=2 height=1 channels=RGBA depth=8"
        );
        assert_eq!(decode(&bmp).unwrap(), image);
    }

    #[test]
    fn decodes_top_down_rgb24_bmp() {
        let image = Image::new(
            2,
            2,
            PixelFormat::Rgb8,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 1, 2, 3],
        )
        .unwrap();
        let mut bmp = encode(&image).unwrap();
        bmp[22..26].copy_from_slice(&(-2_i32).to_le_bytes());
        let pixel_offset = FILE_HEADER_LEN + BITMAPINFOHEADER_LEN;
        let stride = bmp_row_stride(2, 3).unwrap();
        let mut top_down = bmp[pixel_offset..].to_vec();
        top_down[..stride].copy_from_slice(&bmp[pixel_offset + stride..pixel_offset + stride * 2]);
        top_down[stride..stride * 2].copy_from_slice(&bmp[pixel_offset..pixel_offset + stride]);
        bmp[pixel_offset..].copy_from_slice(&top_down);
        assert_eq!(decode(&bmp).unwrap(), image);
    }

    #[test]
    fn rejects_unsupported_compression_and_bit_depth() {
        let image = Image::new(1, 1, PixelFormat::Rgb8, vec![1, 2, 3]).unwrap();
        let mut bmp = encode(&image).unwrap();
        bmp[30..34].copy_from_slice(&1_u32.to_le_bytes());
        assert!(decode(&bmp)
            .unwrap_err()
            .to_string()
            .contains("compression"));

        let mut bmp = encode(&image).unwrap();
        bmp[28..30].copy_from_slice(&8_u16.to_le_bytes());
        assert!(decode(&bmp)
            .unwrap_err()
            .to_string()
            .contains("bit depth 8"));
    }
}
