use imx_core::{pixel_len, Format, Identify, Image, ImageError, PixelFormat, MAX_PIXEL_BYTES};

#[path = "../crates/cli/src/progressive_jpeg_fixtures.rs"]
#[allow(dead_code)]
mod progressive_jpeg_fixtures;

fn png_fixture(
    width: u32,
    height: u32,
    color_type: png::ColorType,
    bit_depth: png::BitDepth,
    pixels: &[u8],
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(color_type);
    encoder.set_depth(bit_depth);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(pixels)
        .unwrap();
    out
}

fn jpeg_with_camera_exif_orientation_le(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    let app0 = b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0";
    let mut app1 = Vec::from(b"Exif\0\0II*\0\x08\0\0\0".as_slice());
    app1.extend_from_slice(&1_u16.to_le_bytes());
    app1.extend_from_slice(&0x0112_u16.to_le_bytes());
    app1.extend_from_slice(&3_u16.to_le_bytes());
    app1.extend_from_slice(&1_u32.to_le_bytes());
    app1.extend_from_slice(&orientation.to_le_bytes());
    app1.extend_from_slice(&[0, 0]);
    app1.extend_from_slice(&0_u32.to_le_bytes());

    let mut out = Vec::new();
    out.extend_from_slice(&jpeg[..2]);
    out.extend_from_slice(&[0xff, 0xe0]);
    out.extend_from_slice(&u16::try_from(app0.len() + 2).unwrap().to_be_bytes());
    out.extend_from_slice(app0);
    out.extend_from_slice(&[0xff, 0xe1]);
    out.extend_from_slice(&u16::try_from(app1.len() + 2).unwrap().to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn top_down_bmp(mut bmp: Vec<u8>, width: usize, height: usize, bytes_per_pixel: usize) -> Vec<u8> {
    let pixel_offset = u32::from_le_bytes(bmp[10..14].try_into().unwrap()) as usize;
    let row_stride = (width * bytes_per_pixel).div_ceil(4) * 4;
    let raster_len = row_stride * height;
    let raster = bmp[pixel_offset..pixel_offset + raster_len].to_vec();
    for row in 0..height {
        let dst = pixel_offset + row * row_stride;
        let src = (height - 1 - row) * row_stride;
        bmp[dst..dst + row_stride].copy_from_slice(&raster[src..src + row_stride]);
    }
    bmp[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
    bmp
}

fn identify(format: Format, input: &[u8]) -> Result<Identify, ImageError> {
    match format {
        Format::Bmp => imx_codec_bmp::identify(input),
        Format::Farbfeld => imx_codec_farbfeld::identify(input),
        Format::Jpeg => imx_codec_jpeg::identify(input),
        Format::Pbm => imx_codec_pnm::identify_pbm(input),
        Format::Pgm => imx_codec_pnm::identify_pgm(input),
        Format::Png => imx_codec_png::identify(input),
        Format::Ppm => imx_codec_pnm::identify_ppm(input),
        Format::Qoi => imx_codec_qoi::identify(input),
    }
}

fn decode(format: Format, input: &[u8]) -> Result<Image, ImageError> {
    match format {
        Format::Bmp => imx_codec_bmp::decode(input),
        Format::Farbfeld => imx_codec_farbfeld::decode(input),
        Format::Jpeg => imx_codec_jpeg::decode(input),
        Format::Pbm => imx_codec_pnm::decode_pbm(input),
        Format::Pgm => imx_codec_pnm::decode_pgm(input),
        Format::Png => imx_codec_png::decode(input),
        Format::Ppm => imx_codec_pnm::decode_ppm(input),
        Format::Qoi => imx_codec_qoi::decode(input).and_then(|image| image.into_core_image()),
    }
}

#[test]
fn representative_intake_corpus_identifies_and_decodes() {
    let rgba16 = Image::new(
        2,
        2,
        PixelFormat::Rgba16Be,
        vec![
            0x00, 0x01, 0x12, 0x34, 0x7f, 0xff, 0xff, 0xfe, 0x01, 0x00, 0x80, 0x01, 0xaa, 0x55,
            0x40, 0x00, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0x11, 0x11, 0x22, 0x22,
            0x33, 0x33, 0x44, 0x44,
        ],
    )
    .unwrap();
    let qoi_rgb = imx_codec_qoi::encode(
        2,
        2,
        3,
        imx_codec_qoi::QOI_LINEAR,
        &[0, 255, 0, 255, 0, 0, 18, 52, 86, 255, 255, 255],
    )
    .unwrap();
    let gray_alpha_png = png_fixture(
        2,
        1,
        png::ColorType::GrayscaleAlpha,
        png::BitDepth::Eight,
        &[0x20, 0x80, 0xff, 0x40],
    );
    let gray_png = imx_codec_png::encode(
        &Image::new(3, 1, PixelFormat::Gray8, vec![0x00, 0x80, 0xff]).unwrap(),
    )
    .unwrap();
    let rgb16_png = imx_codec_png::encode(
        &Image::new(
            2,
            1,
            PixelFormat::Rgb16Be,
            vec![
                0x00, 0x01, 0x12, 0x34, 0xff, 0xfe, 0xab, 0xcd, 0x80, 0x00, 0x01, 0x23,
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let rgba16_png = imx_codec_png::encode(
        &Image::new(
            1,
            1,
            PixelFormat::Rgba16Be,
            vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0],
        )
        .unwrap(),
    )
    .unwrap();
    let bmp_rgb24 = imx_codec_bmp::encode(
        &Image::new(
            3,
            2,
            PixelFormat::Rgb8,
            vec![
                255, 0, 0, 0, 255, 0, 0, 0, 255, 12, 34, 56, 78, 90, 123, 222, 111, 3,
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let bmp_top_down_rgb24 = top_down_bmp(bmp_rgb24.clone(), 3, 2, 3);
    let bmp_rgba32 = imx_codec_bmp::encode(
        &Image::new(
            2,
            2,
            PixelFormat::Rgba8,
            vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 12, 34, 56, 0],
        )
        .unwrap(),
    )
    .unwrap();
    let bmp_top_down_rgba32 = top_down_bmp(bmp_rgba32.clone(), 2, 2, 4);
    let camera_jpeg = jpeg_with_camera_exif_orientation_le(
        &imx_codec_jpeg::encode(
            &Image::new(
                3,
                2,
                PixelFormat::Rgb8,
                vec![
                    32, 64, 96, 96, 128, 160, 160, 192, 224, 224, 192, 160, 160, 128, 96, 96, 64,
                    32,
                ],
            )
            .unwrap(),
        )
        .unwrap(),
        6,
    );
    let progressive_camera_jpeg =
        jpeg_with_camera_exif_orientation_le(&progressive_jpeg_fixtures::progressive_rgb_jpeg(), 6);

    let cases = vec![
        (
            "bmp-rgb24",
            Format::Bmp,
            bmp_rgb24,
            "format=BMP width=3 height=2 channels=RGB depth=8",
        ),
        (
            "bmp-rgba32",
            Format::Bmp,
            bmp_rgba32,
            "format=BMP width=2 height=2 channels=RGBA depth=8",
        ),
        (
            "bmp-top-down-rgb24",
            Format::Bmp,
            bmp_top_down_rgb24,
            "format=BMP width=3 height=2 channels=RGB depth=8",
        ),
        (
            "bmp-top-down-rgba32",
            Format::Bmp,
            bmp_top_down_rgba32,
            "format=BMP width=2 height=2 channels=RGBA depth=8",
        ),
        (
            "farbfeld-rgba16",
            Format::Farbfeld,
            imx_codec_farbfeld::encode(&rgba16).unwrap(),
            "format=FARBFELD width=2 height=2 channels=RGBA depth=16",
        ),
        (
            "progressive-jpeg-gray",
            Format::Jpeg,
            progressive_jpeg_fixtures::progressive_gray_jpeg(),
            "format=JPEG width=4 height=2 channels=GRAY depth=8",
        ),
        (
            "jpeg-camera-exif-le-o6",
            Format::Jpeg,
            camera_jpeg,
            "format=JPEG width=2 height=3 channels=RGB depth=8",
        ),
        (
            "progressive-jpeg-camera-exif-le-o6",
            Format::Jpeg,
            progressive_camera_jpeg,
            "format=JPEG width=3 height=4 channels=RGB depth=8",
        ),
        (
            "qoi-rgb-linear",
            Format::Qoi,
            qoi_rgb,
            "format=QOI width=2 height=2 channels=RGB depth=8",
        ),
        (
            "pbm-ascii-comments",
            Format::Pbm,
            b"P1\n# checker\n3 2\n0 1 0\n1 # inline\n0 1\n".to_vec(),
            "format=PBM width=3 height=2 channels=GRAY depth=1",
        ),
        (
            "pgm-ascii-scaled",
            Format::Pgm,
            b"P2\n# gray ramp\n3 1\n31\n0 15 31\n".to_vec(),
            "format=PGM width=3 height=1 channels=GRAY depth=8",
        ),
        (
            "pgm-binary-16",
            Format::Pgm,
            b"P5\n2 1\n65535\n\x12\x34\xff\xff".to_vec(),
            "format=PGM width=2 height=1 channels=GRAY depth=16",
        ),
        (
            "pgm-binary-comments-crlf",
            Format::Pgm,
            b"P5\r\n# binary comments and CRLF\r\n3\t1\r\n255\r\n\x00\x80\xff".to_vec(),
            "format=PGM width=3 height=1 channels=GRAY depth=8",
        ),
        (
            "png-gray-alpha",
            Format::Png,
            gray_alpha_png,
            "format=PNG width=2 height=1 channels=RGBA depth=8",
        ),
        (
            "png-gray8",
            Format::Png,
            gray_png,
            "format=PNG width=3 height=1 channels=GRAY depth=8",
        ),
        (
            "png-rgba16",
            Format::Png,
            rgba16_png,
            "format=PNG width=1 height=1 channels=RGBA depth=16",
        ),
        (
            "png-rgb16",
            Format::Png,
            rgb16_png,
            "format=PNG width=2 height=1 channels=RGB depth=16",
        ),
        (
            "ppm-ascii-high-max",
            Format::Ppm,
            b"P3\n# high max\n2 1\n1023\n0 512 1023\n1023 256 128\n".to_vec(),
            "format=PPM width=2 height=1 channels=RGB depth=16",
        ),
        (
            "ppm-binary-comments-crlf",
            Format::Ppm,
            b"P6\r\n# binary comments and CRLF\r\n2\t1\r\n255\r\n\x00\x80\xff\xff\x40\x00".to_vec(),
            "format=PPM width=2 height=1 channels=RGB depth=8",
        ),
    ];

    for (name, format, bytes, expected) in cases {
        let info = identify(format, &bytes).unwrap_or_else(|err| {
            panic!("{name} identify failed unexpectedly: {err}");
        });
        assert_eq!(info.stable_line(), expected, "{name}");
        decode(format, &bytes).unwrap_or_else(|err| {
            panic!("{name} decode failed unexpectedly: {err}");
        });
    }
}

#[test]
fn adversarial_intake_corpus_fails_with_clear_errors() {
    let bad_farbfeld = [
        b"notfield".as_slice(),
        &1_u32.to_be_bytes(),
        &1_u32.to_be_bytes(),
    ]
    .concat();
    let mut bad_qoi = Vec::from(imx_codec_qoi::MAGIC.as_slice());
    bad_qoi.extend_from_slice(&1_u32.to_be_bytes());
    bad_qoi.extend_from_slice(&1_u32.to_be_bytes());
    bad_qoi.extend_from_slice(&[2, imx_codec_qoi::QOI_SRGB]);
    let mut truncated_qoi = Vec::from(imx_codec_qoi::MAGIC.as_slice());
    truncated_qoi.extend_from_slice(&1_u32.to_be_bytes());
    truncated_qoi.extend_from_slice(&1_u32.to_be_bytes());
    truncated_qoi.extend_from_slice(&[3, imx_codec_qoi::QOI_SRGB, imx_codec_qoi::QOI_OP_RGB, 1]);
    let mut bad_bmp =
        imx_codec_bmp::encode(&Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap())
            .unwrap();
    bad_bmp[30..34].copy_from_slice(&1_u32.to_le_bytes());

    let cases = vec![
        (
            "bmp-unsupported-compression",
            decode(Format::Bmp, &bad_bmp).unwrap_err(),
            "BMP compression is not supported",
        ),
        (
            "farbfeld-invalid-magic",
            decode(Format::Farbfeld, &bad_farbfeld).unwrap_err(),
            "invalid FARBFELD header",
        ),
        (
            "qoi-invalid-channels",
            decode(Format::Qoi, &bad_qoi).unwrap_err(),
            "QOI channels must be 3 or 4, got 2",
        ),
        (
            "qoi-truncated-rgb-op",
            decode(Format::Qoi, &truncated_qoi).unwrap_err(),
            "unexpected end of file: expected 17 bytes, got 16",
        ),
        (
            "pbm-invalid-sample",
            decode(Format::Pbm, b"P1\n2 1\n0 x\n").unwrap_err(),
            "PBM samples must be ASCII 0 or 1",
        ),
        (
            "pgm-over-max-sample",
            decode(Format::Pgm, b"P2\n1 1\n10\n11\n").unwrap_err(),
            "PGM sample value must be <= 10, got 11",
        ),
        (
            "ppm-over-max-sample",
            decode(Format::Ppm, b"P3\n1 1\n256\n0 257 1\n").unwrap_err(),
            "PPM sample value must be <= 256, got 257",
        ),
        (
            "png-truncated",
            decode(Format::Png, imx_codec_png::MAGIC).unwrap_err(),
            "PNG decode failed",
        ),
        (
            "jpeg-truncated",
            decode(Format::Jpeg, imx_codec_jpeg::MAGIC).unwrap_err(),
            "JPEG decode failed",
        ),
    ];

    for (name, err, expected) in cases {
        assert!(
            err.to_string().contains(expected),
            "{name} expected {expected:?}, got {err}"
        );
    }
}

#[test]
fn resource_boundaries_are_checked_without_large_allocations() {
    let at_limit_width = u32::try_from(MAX_PIXEL_BYTES / 4).unwrap();
    assert_eq!(pixel_len(at_limit_width, 1, 4).unwrap(), MAX_PIXEL_BYTES);
    assert!(matches!(
        pixel_len(at_limit_width + 1, 1, 4),
        Err(ImageError::ImageTooLarge { .. })
    ));

    let farbfeld_at_limit = [
        imx_codec_farbfeld::MAGIC.as_slice(),
        &(u32::try_from(MAX_PIXEL_BYTES / 8).unwrap()).to_be_bytes(),
        &1_u32.to_be_bytes(),
    ]
    .concat();
    assert!(imx_codec_farbfeld::decode_header(&farbfeld_at_limit).is_ok());

    let farbfeld_over_limit = [
        imx_codec_farbfeld::MAGIC.as_slice(),
        &(u32::try_from(MAX_PIXEL_BYTES / 8).unwrap() + 1).to_be_bytes(),
        &1_u32.to_be_bytes(),
    ]
    .concat();
    assert!(matches!(
        imx_codec_farbfeld::decode_header(&farbfeld_over_limit),
        Err(ImageError::ImageTooLarge { .. })
    ));

    assert!(matches!(
        imx_codec_qoi::decode_header(
            &[
                imx_codec_qoi::MAGIC.as_slice(),
                &100_000_u32.to_be_bytes(),
                &100_000_u32.to_be_bytes(),
                &[4, imx_codec_qoi::QOI_SRGB],
            ]
            .concat()
        ),
        Err(ImageError::ImageTooLarge { .. })
    ));
    let mut huge_bmp =
        imx_codec_bmp::encode(&Image::new(1, 1, PixelFormat::Rgb8, vec![0, 0, 0]).unwrap())
            .unwrap();
    huge_bmp[18..22].copy_from_slice(&100_000_i32.to_le_bytes());
    huge_bmp[22..26].copy_from_slice(&100_000_i32.to_le_bytes());
    assert!(matches!(
        imx_codec_bmp::decode(&huge_bmp),
        Err(ImageError::ImageTooLarge { .. }) | Err(ImageError::UnexpectedEof { .. })
    ));
    assert!(matches!(
        imx_codec_pnm::decode_ppm_header(b"P6\n100000 100000\n255\n"),
        Err(ImageError::ImageTooLarge { .. })
    ));
}
