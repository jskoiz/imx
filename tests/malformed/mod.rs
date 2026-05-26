use imx_core::{Image, ImageError, PixelFormat};

#[path = "../../crates/cli/src/progressive_jpeg_fixtures.rs"]
#[allow(dead_code)]
mod progressive_jpeg_fixtures;

fn qoi_header(width: u32, height: u32, channels: u8, colorspace: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(imx_codec_qoi::MAGIC);
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.push(channels);
    bytes.push(colorspace);
    bytes
}

fn png_fixture(color_type: png::ColorType, bit_depth: png::BitDepth, pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 1, 1);
    encoder.set_color(color_type);
    encoder.set_depth(bit_depth);
    if color_type == png::ColorType::Indexed {
        encoder.set_palette(&[0, 0, 0, 255, 255, 255]);
    }
    encoder
        .write_header()
        .unwrap()
        .write_image_data(pixels)
        .unwrap();
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn rewrite_chunk_crc(png: &mut [u8], chunk_start: usize) {
    let len = u32::from_be_bytes(png[chunk_start..chunk_start + 4].try_into().unwrap()) as usize;
    let chunk_type_start = chunk_start + 4;
    let crc_start = chunk_type_start + 4 + len;
    let crc = crc32(&png[chunk_type_start..crc_start]);
    png[crc_start..crc_start + 4].copy_from_slice(&crc.to_be_bytes());
}

fn insert_png_chunk(png: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    let insert_at = imx_codec_png::MAGIC.len() + 4 + 4 + 13 + 4;
    let mut chunk = Vec::new();
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(chunk_type);
    chunk.extend_from_slice(data);
    let crc = crc32(&chunk[4..]);
    chunk.extend_from_slice(&crc.to_be_bytes());
    png.splice(insert_at..insert_at, chunk);
}

fn set_ihdr_dimensions(png: &mut [u8], width: u32, height: u32) {
    let ihdr_data = imx_codec_png::MAGIC.len() + 8;
    png[ihdr_data..ihdr_data + 4].copy_from_slice(&width.to_be_bytes());
    png[ihdr_data + 4..ihdr_data + 8].copy_from_slice(&height.to_be_bytes());
    rewrite_chunk_crc(png, imx_codec_png::MAGIC.len());
}

fn set_ihdr_interlaced(png: &mut [u8]) {
    let ihdr_interlace = imx_codec_png::MAGIC.len() + 8 + 12;
    png[ihdr_interlace] = 1;
    rewrite_chunk_crc(png, imx_codec_png::MAGIC.len());
}

fn jpeg_with_exif_app1(jpeg: &[u8], app1_data: &[u8]) -> Vec<u8> {
    let segment_len = u16::try_from(app1_data.len() + 2).unwrap();
    let mut out = Vec::new();
    out.extend_from_slice(&jpeg[..2]);
    out.extend_from_slice(&[0xff, 0xe1]);
    out.extend_from_slice(&segment_len.to_be_bytes());
    out.extend_from_slice(app1_data);
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn jpeg_with_exif_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    let mut app1 = Vec::from(b"Exif\0\0MM\0*\0\0\0\x08".as_slice());
    app1.extend_from_slice(&1_u16.to_be_bytes());
    app1.extend_from_slice(&0x0112_u16.to_be_bytes());
    app1.extend_from_slice(&3_u16.to_be_bytes());
    app1.extend_from_slice(&1_u32.to_be_bytes());
    app1.extend_from_slice(&orientation.to_be_bytes());
    app1.extend_from_slice(&[0, 0]);
    app1.extend_from_slice(&0_u32.to_be_bytes());
    jpeg_with_exif_app1(jpeg, &app1)
}

#[test]
fn farbfeld_rejects_bad_headers_truncation_and_extreme_dimensions() {
    assert!(matches!(
        imx_codec_farbfeld::decode(b"not farbfeld"),
        Err(ImageError::UnexpectedEof { .. }) | Err(ImageError::InvalidHeader("FARBFELD"))
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

    let mut truncated_rgb = qoi_header(1, 1, 3, imx_codec_qoi::QOI_SRGB);
    truncated_rgb.extend_from_slice(&[imx_codec_qoi::QOI_OP_RGB, 0x10, 0x20]);
    assert_eq!(
        imx_codec_qoi::decode(&truncated_rgb),
        Err(ImageError::UnexpectedEof {
            expected: 18,
            actual: 17,
        })
    );

    let mut truncated_rgba = qoi_header(1, 1, 4, imx_codec_qoi::QOI_SRGB);
    truncated_rgba.extend_from_slice(&[imx_codec_qoi::QOI_OP_RGBA, 0x10, 0x20, 0x30]);
    assert_eq!(
        imx_codec_qoi::decode(&truncated_rgba),
        Err(ImageError::UnexpectedEof {
            expected: 19,
            actual: 18,
        })
    );

    let mut truncated_luma = qoi_header(1, 1, 3, imx_codec_qoi::QOI_SRGB);
    truncated_luma.push(imx_codec_qoi::QOI_OP_LUMA);
    assert!(matches!(
        imx_codec_qoi::decode(&truncated_luma),
        Err(ImageError::UnexpectedEof {
            expected: 16,
            actual: 15,
        })
    ));
}

#[test]
fn png_rejects_malformed_and_unsupported_inputs() {
    assert_eq!(
        imx_codec_png::decode(b"not png"),
        Err(ImageError::UnexpectedEof {
            expected: imx_codec_png::MAGIC.len(),
            actual: 7
        })
    );
    assert!(imx_codec_png::decode(imx_codec_png::MAGIC)
        .unwrap_err()
        .to_string()
        .contains("PNG decode failed"));

    let indexed = png_fixture(png::ColorType::Indexed, png::BitDepth::Eight, &[0]);
    assert!(imx_codec_png::decode(&indexed)
        .unwrap_err()
        .to_string()
        .contains("indexed color"));

    let subbyte_gray = png_fixture(png::ColorType::Grayscale, png::BitDepth::One, &[0x80]);
    assert!(imx_codec_png::decode(&subbyte_gray)
        .unwrap_err()
        .to_string()
        .contains("sub-8-bit"));

    let mut interlaced = png_fixture(png::ColorType::Rgb, png::BitDepth::Eight, &[255, 0, 0]);
    set_ihdr_interlaced(&mut interlaced);
    assert!(imx_codec_png::decode(&interlaced)
        .unwrap_err()
        .to_string()
        .contains("interlacing"));

    let mut trns = png_fixture(png::ColorType::Grayscale, png::BitDepth::Eight, &[0]);
    insert_png_chunk(&mut trns, b"tRNS", &[0, 0]);
    assert!(imx_codec_png::decode(&trns)
        .unwrap_err()
        .to_string()
        .contains("tRNS transparency"));

    let mut apng = png_fixture(png::ColorType::Rgb, png::BitDepth::Eight, &[255, 0, 0]);
    insert_png_chunk(&mut apng, b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]);
    assert!(imx_codec_png::decode(&apng)
        .unwrap_err()
        .to_string()
        .contains("animation"));

    let mut huge = png_fixture(png::ColorType::Rgba, png::BitDepth::Sixteen, &[0; 8]);
    set_ihdr_dimensions(&mut huge, 100_000, 100_000);
    assert!(matches!(
        imx_codec_png::decode(&huge),
        Err(ImageError::ImageTooLarge { .. })
    ));

    let mut corrupted =
        imx_codec_png::encode(&Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap())
            .unwrap();
    corrupted[32] ^= 0xff;
    assert!(imx_codec_png::decode(&corrupted)
        .unwrap_err()
        .to_string()
        .contains("PNG decode failed"));
}

#[test]
fn jpeg_rejects_malformed_and_unsupported_inputs() {
    assert_eq!(
        imx_codec_jpeg::decode(b"x"),
        Err(ImageError::UnexpectedEof {
            expected: imx_codec_jpeg::MAGIC.len(),
            actual: 1
        })
    );
    assert_eq!(
        imx_codec_jpeg::decode(b"not jpeg"),
        Err(ImageError::InvalidHeader("JPEG"))
    );
    assert!(imx_codec_jpeg::decode(imx_codec_jpeg::MAGIC)
        .unwrap_err()
        .to_string()
        .contains("JPEG decode failed"));
    assert!(imx_codec_jpeg::decode(b"\xff\xd8\xff\xd9")
        .unwrap_err()
        .to_string()
        .contains("JPEG decode failed"));

    let image = Image::new(8, 8, PixelFormat::Rgb8, vec![0x80; 8 * 8 * 3]).unwrap();
    let jpeg = imx_codec_jpeg::encode(&image).unwrap();
    for len in 0..jpeg.len() {
        let result = std::panic::catch_unwind(|| imx_codec_jpeg::decode(&jpeg[..len]));
        assert!(result.is_ok(), "JPEG truncation panicked at len {len}");
    }
    let progressive = progressive_jpeg_fixtures::progressive_rgb_jpeg();
    assert!(progressive_jpeg_fixtures::is_progressive_jpeg(&progressive));
    for len in 0..progressive.len() {
        let result = std::panic::catch_unwind(|| imx_codec_jpeg::decode(&progressive[..len]));
        assert!(
            result.is_ok(),
            "progressive JPEG truncation panicked at len {len}"
        );
    }

    let mut cmyk = Vec::new();
    jpeg_encoder::Encoder::new(&mut cmyk, imx_codec_jpeg::DEFAULT_QUALITY)
        .encode(&[0, 255, 255, 0], 1, 1, jpeg_encoder::ColorType::Cmyk)
        .unwrap();
    assert!(imx_codec_jpeg::identify(&cmyk)
        .unwrap_err()
        .to_string()
        .contains("JPEG CMYK is not supported"));

    let invalid_orientation = jpeg_with_exif_orientation(&jpeg, 9);
    assert!(imx_codec_jpeg::identify(&invalid_orientation)
        .unwrap_err()
        .to_string()
        .contains("JPEG EXIF Orientation value 9 is not supported"));

    let bad_endian = jpeg_with_exif_app1(&jpeg, b"Exif\0\0ZZ\0*\0\0\0\x08");
    assert!(imx_codec_jpeg::identify(&bad_endian)
        .unwrap_err()
        .to_string()
        .contains("JPEG EXIF Orientation metadata is malformed"));

    let bad_offset = jpeg_with_exif_app1(&jpeg, b"Exif\0\0MM\0*\xff\xff\xff\xf0");
    assert!(imx_codec_jpeg::decode(&bad_offset)
        .unwrap_err()
        .to_string()
        .contains("JPEG EXIF Orientation metadata is malformed"));

    let rgba = Image::new(1, 1, PixelFormat::Rgba8, vec![255, 0, 0, 128]).unwrap();
    assert!(imx_codec_jpeg::encode(&rgba)
        .unwrap_err()
        .to_string()
        .contains("alpha is not supported"));
}

#[test]
fn ppm_rejects_out_of_scope_and_truncated_inputs() {
    assert!(imx_codec_pnm::decode_ppm(b"P3\n1 1\n255\n255 0 0").is_ok());
    assert!(imx_codec_pnm::decode_ppm(b"P3\n1 1\n65535\n65535 32768 0").is_ok());
    assert!(imx_codec_pnm::decode_ppm(b"P6\n1 1\n256\n\x00\x00\x01\x00\x01\x00").is_ok());
    assert_eq!(
        imx_codec_pnm::decode_ppm(b"P2\n1 1\n255\n255"),
        Err(ImageError::InvalidHeader("PPM"))
    );
    assert_eq!(
        imx_codec_pnm::decode_ppm(b"P6\n1 1\n0\n\0\0\0\0\0\0"),
        Err(ImageError::InvalidMaxValue {
            format: "PPM",
            max_value: 0,
            max_supported: 65535,
        })
    );
    assert_eq!(
        imx_codec_pnm::decode_ppm(b"P6\n1 1\n65536\n\0\0\0\0\0\0"),
        Err(ImageError::InvalidMaxValue {
            format: "PPM",
            max_value: 65536,
            max_supported: 65535,
        })
    );
    assert_eq!(
        imx_codec_pnm::decode_ppm(b"P3\n1 1\n256\n0 257 1\n"),
        Err(ImageError::InvalidSampleValue {
            format: "PPM",
            sample_value: 257,
            max_value: 256,
        })
    );
    assert_eq!(
        imx_codec_pnm::decode_ppm(b"P6\n1 1\n256\n\x00\x00\x01\x01\x00\x00"),
        Err(ImageError::InvalidSampleValue {
            format: "PPM",
            sample_value: 257,
            max_value: 256,
        })
    );
    assert!(matches!(
        imx_codec_pnm::decode_ppm(b"P6\n2 1\n255\n\xff\x00\x00"),
        Err(ImageError::UnexpectedEof { .. })
    ));
    assert!(matches!(
        imx_codec_pnm::decode_ppm(b"P6\n1 1\n65535\n\x12"),
        Err(ImageError::UnexpectedEof { .. })
    ));
    assert!(matches!(
        imx_codec_pnm::decode_ppm(b"P3\n2 1\n65535\n0 1 2\n"),
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
        Err(ImageError::InvalidSampleValue {
            format: "PGM",
            sample_value: 11,
            max_value: 10,
        })
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
        Err(ImageError::InvalidPbmSample { byte: b'2' })
    );
    assert_eq!(
        imx_codec_pnm::decode_pbm(b"P1\n2 1\n0 x\n"),
        Err(ImageError::InvalidPbmSample { byte: b'x' })
    );
    assert_eq!(
        imx_codec_pnm::decode_pbm(b"P1\n1 1\n255\n"),
        Err(ImageError::InvalidPbmSample { byte: b'2' })
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
