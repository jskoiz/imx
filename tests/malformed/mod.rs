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
    assert!(imx_codec_pnm::decode_ppm(b"P3\n1 1\n255\n255 0 0").is_ok());
    assert_eq!(
        imx_codec_pnm::decode_ppm(b"P2\n1 1\n255\n255"),
        Err(ImageError::InvalidHeader("PPM"))
    );
    assert_eq!(
        imx_codec_pnm::decode_ppm(b"P6\n1 1\n65535\n\0\0\0\0\0\0"),
        Err(ImageError::InvalidMaxValue {
            format: "PPM",
            max_value: 65535,
            max_supported: 255,
        })
    );
    assert!(matches!(
        imx_codec_pnm::decode_ppm(b"P6\n2 1\n255\n\xff\x00\x00"),
        Err(ImageError::UnexpectedEof { .. })
    ));
}

#[test]
fn pgm_rejects_malformed_inputs() {
    assert!(imx_codec_pnm::decode_pgm(b"P2\n1 1\n255\n255").is_ok());
    assert_eq!(
        imx_codec_pnm::decode_pgm(b"P3\n1 1\n255\n255 0 0"),
        Err(ImageError::InvalidHeader("PGM"))
    );
    assert_eq!(
        imx_codec_pnm::decode_pgm(b"P2\n1 1\n0\n0\n"),
        Err(ImageError::InvalidMaxValue {
            format: "PGM",
            max_value: 0,
            max_supported: 65535,
        })
    );
    assert_eq!(
        imx_codec_pnm::decode_pgm(b"P2\n1 1\n65536\n0\n"),
        Err(ImageError::InvalidMaxValue {
            format: "PGM",
            max_value: 65536,
            max_supported: 65535,
        })
    );
    assert_eq!(
        imx_codec_pnm::decode_pgm(b"P2\n1 1\n10\n11\n"),
        Err(ImageError::InvalidHeader("PGM"))
    );
    assert!(matches!(
        imx_codec_pnm::decode_pgm(b"P2\n2 1\n255\n0\n"),
        Err(ImageError::UnexpectedEof { .. })
    ));
    assert!(matches!(
        imx_codec_pnm::decode_pgm(b"P5\n2 1\n255\n\x00"),
        Err(ImageError::UnexpectedEof { .. })
    ));
    assert!(matches!(
        imx_codec_pnm::decode_pgm(b"P5\n1 1\n65535\n\x12"),
        Err(ImageError::UnexpectedEof { .. })
    ));
    assert_eq!(
        imx_codec_pnm::decode_pgm(b"P5\n1 1\n255X"),
        Err(ImageError::InvalidHeader("PGM"))
    );
    assert_eq!(
        imx_codec_pnm::decode_pgm(b"P2\n0 1\n255\n0\n"),
        Err(ImageError::InvalidDimensions)
    );
    assert_eq!(
        imx_codec_pnm::decode_pgm(b"P2\n1 0\n255\n0\n"),
        Err(ImageError::InvalidDimensions)
    );
    assert!(matches!(
        imx_codec_pnm::decode_pgm(b"P2\n# unterminated comment"),
        Err(ImageError::UnexpectedEof { .. })
    ));
}

#[test]
fn pbm_rejects_malformed_inputs() {
    assert!(imx_codec_pnm::decode_pbm(b"P1\n1 1\n1").is_ok());
    assert!(imx_codec_pnm::decode_pbm(b"P4\n1 1\n\x80").is_ok());
    assert_eq!(
        imx_codec_pnm::decode_pbm(b"P2\n1 1\n255\n255"),
        Err(ImageError::InvalidHeader("PBM"))
    );
    assert_eq!(
        imx_codec_pnm::decode_pbm(b"P1\n2 1\n0 2\n"),
        Err(ImageError::InvalidHeader("PBM"))
    );
    assert_eq!(
        imx_codec_pnm::decode_pbm(b"P1\n2 1\n0 x\n"),
        Err(ImageError::InvalidHeader("PBM"))
    );
    assert_eq!(
        imx_codec_pnm::decode_pbm(b"P1\n1 1\n255\n"),
        Err(ImageError::InvalidHeader("PBM"))
    );
    assert!(matches!(
        imx_codec_pnm::decode_pbm(b"P1\n2 1\n0\n"),
        Err(ImageError::UnexpectedEof { .. })
    ));
    assert!(matches!(
        imx_codec_pnm::decode_pbm(b"P4\n1 1\n"),
        Err(ImageError::UnexpectedEof { .. })
    ));
    assert!(matches!(
        imx_codec_pnm::decode_pbm(b"P4\n9 2\n\x80\x00\x80"),
        Err(ImageError::UnexpectedEof { .. })
    ));
    assert_eq!(
        imx_codec_pnm::decode_pbm(b"P4\n1 1\x80"),
        Err(ImageError::InvalidHeader("PBM"))
    );
    assert_eq!(
        imx_codec_pnm::decode_pbm(b"P1\n0 1\n0\n"),
        Err(ImageError::InvalidDimensions)
    );
    assert_eq!(
        imx_codec_pnm::decode_pbm(b"P1\n1 0\n0\n"),
        Err(ImageError::InvalidDimensions)
    );
    assert!(matches!(
        imx_codec_pnm::decode_pbm(b"P1\n# unterminated comment"),
        Err(ImageError::UnexpectedEof { .. })
    ));
    assert!(matches!(
        imx_codec_pnm::decode_pbm(b"P1\n4294967296 1\n0\n"),
        Err(ImageError::InvalidHeader("PBM"))
    ));
    assert!(matches!(
        imx_codec_pnm::decode_pbm(b"P4\n4294967295 4294967295\n"),
        Err(ImageError::LengthOverflow) | Err(ImageError::ImageTooLarge { .. })
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
        imx_codec_pnm::decode_ppm_header(b"P6\n100000 100000\n255\n"),
        Err(ImageError::ImageTooLarge { .. })
    ));
    assert!(matches!(
        imx_codec_pnm::decode_pgm_header(b"P5\n100000 100000\n255\n"),
        Err(ImageError::ImageTooLarge { .. })
    ));
    assert!(matches!(
        imx_codec_pnm::decode_pbm_header(b"P4\n100000 100000\n"),
        Err(ImageError::ImageTooLarge { .. })
    ));
}
