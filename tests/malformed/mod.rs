use imx_core::{Image, ImageError, PixelFormat};

fn qoi_header(width: u32, height: u32, channels: u8, colorspace: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(imx_codec_qoi::MAGIC);
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.push(channels);
    bytes.push(colorspace);
    bytes
}

#[test]
fn farbfeld_rejects_bad_headers_truncation_and_extreme_dimensions() {
    assert!(matches!(
        imx_codec_farbfeld::decode(b"not farbfeld"),
        Err(ImageError::UnexpectedEof { .. }) | Err(ImageError::InvalidHeader("farbfeld"))
    ));

    let mut truncated = Vec::from(imx_codec_farbfeld::MAGIC.as_slice());
    truncated.extend_from_slice(&2_u32.to_be_bytes());
    truncated.extend_from_slice(&2_u32.to_be_bytes());
    truncated.extend_from_slice(&[0; 31]);
    assert!(matches!(
        imx_codec_farbfeld::decode(&truncated),
        Err(ImageError::UnexpectedEof { .. })
    ));

    let mut huge = Vec::from(imx_codec_farbfeld::MAGIC.as_slice());
    huge.extend_from_slice(&u32::MAX.to_be_bytes());
    huge.extend_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(
        imx_codec_farbfeld::decode(&huge),
        Err(ImageError::LengthOverflow)
            | Err(ImageError::ImageTooLarge { .. })
            | Err(ImageError::UnexpectedEof { .. })
    ));
}

#[test]
fn qoi_rejects_bad_headers_and_truncated_opcode_payloads() {
    assert_eq!(
        imx_codec_qoi::decode_header(b"noif\0\0\0\x01\0\0\0\x01\x03\0"),
        Err(ImageError::InvalidHeader("QOI"))
    );
    assert_eq!(
        imx_codec_qoi::decode_header(&qoi_header(1, 1, 2, imx_codec_qoi::QOI_SRGB)),
        Err(ImageError::InvalidChannels { channels: 2 })
    );
    assert_eq!(
        imx_codec_qoi::decode_header(&qoi_header(1, 1, 3, 9)),
        Err(ImageError::InvalidColorspace { colorspace: 9 })
    );

    let mut truncated = qoi_header(1, 1, 3, imx_codec_qoi::QOI_SRGB);
    truncated.extend_from_slice(&[imx_codec_qoi::QOI_OP_RGB, 0x10, 0x20]);
    assert!(matches!(
        imx_codec_qoi::decode(&truncated),
        Err(ImageError::UnexpectedEof { .. })
    ));
}

#[test]
fn ppm_rejects_out_of_scope_and_truncated_inputs() {
    assert!(imx_codec_ppm::decode(b"P3\n1 1\n255\n255 0 0").is_ok());
    assert_eq!(
        imx_codec_ppm::decode(b"P2\n1 1\n255\n255"),
        Err(ImageError::InvalidHeader("PPM"))
    );
    assert_eq!(
        imx_codec_ppm::decode(b"P6\n1 1\n65535\n\0\0\0\0\0\0"),
        Err(ImageError::InvalidMaxValue { max_value: 65535 })
    );
    assert!(matches!(
        imx_codec_ppm::decode(b"P6\n2 1\n255\n\xff\x00\x00"),
        Err(ImageError::UnexpectedEof { .. })
    ));
}

#[test]
fn shared_pixel_buffers_reject_wrong_lengths() {
    assert!(matches!(
        Image::new(2, 2, PixelFormat::Rgba16Be, vec![0; 31]),
        Err(ImageError::InvalidPixelBuffer {
            expected: 32,
            actual: 31
        })
    ));
}

#[test]
fn excessive_dimensions_fail_before_allocation() {
    let mut farbfeld = Vec::from(imx_codec_farbfeld::MAGIC.as_slice());
    farbfeld.extend_from_slice(&100_000_u32.to_be_bytes());
    farbfeld.extend_from_slice(&100_000_u32.to_be_bytes());
    assert!(matches!(
        imx_codec_farbfeld::decode_header(&farbfeld),
        Err(ImageError::ImageTooLarge { .. })
    ));

    assert!(matches!(
        imx_codec_qoi::decode_header(&qoi_header(100_000, 100_000, 4, imx_codec_qoi::QOI_SRGB)),
        Err(ImageError::ImageTooLarge { .. })
    ));
    assert!(matches!(
        imx_codec_ppm::decode_header(b"P6\n100000 100000\n255\n"),
        Err(ImageError::ImageTooLarge { .. })
    ));
}
