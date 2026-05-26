use imx_core::{Image, PixelFormat};

fn hex_fixture(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

#[test]
fn decodes_checked_in_golden_fixture_files() {
    let farbfeld = hex_fixture(include_str!("fixtures/farbfeld-1x1-red-half-alpha.hex"));
    let qoi = hex_fixture(include_str!("fixtures/qoi-1x1-red-half-alpha.hex"));
    let pbm = hex_fixture(include_str!("fixtures/pbm-1x1-black.hex"));
    let ppm = hex_fixture(include_str!("fixtures/ppm-1x1-red.hex"));
    let pgm = hex_fixture(include_str!("fixtures/pgm-1x1-gray.hex"));

    assert_eq!(
        imx_codec_farbfeld::decode(&farbfeld).unwrap().pixels(),
        &[0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00]
    );
    assert_eq!(
        imx_codec_qoi::decode(&qoi).unwrap().pixels,
        &[0xff, 0x00, 0x00, 0x80]
    );
    assert_eq!(
        imx_codec_pnm::decode_ppm(&ppm).unwrap().pixels(),
        &[0xff, 0x00, 0x00]
    );
    assert_eq!(
        imx_codec_pnm::decode_ppm(b"P3\n# red pixel\n1 1\n255\n255 0 0")
            .unwrap()
            .pixels(),
        &[0xff, 0x00, 0x00]
    );
    let ppm16 = imx_codec_pnm::decode_ppm(b"P6\n1 1\n65535\n\x12\x34\x56\x78\x9a\xbc").unwrap();
    assert_eq!(ppm16.pixel_format(), PixelFormat::Rgb16Be);
    assert_eq!(ppm16.pixels(), &[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]);
    assert_eq!(imx_codec_pnm::decode_pbm(&pbm).unwrap().pixels(), &[0]);
    assert_eq!(
        imx_codec_pnm::decode_pbm(b"P1\n# black pixel\n1 1\n1")
            .unwrap()
            .pixels(),
        &[0]
    );
    assert_eq!(imx_codec_pnm::decode_pgm(&pgm).unwrap().pixels(), &[0x80]);
    assert_eq!(
        imx_codec_pnm::decode_pgm(b"P2\n# gray pixel\n1 1\n31\n15")
            .unwrap()
            .pixels(),
        &[0x7b]
    );
    let png =
        imx_codec_png::encode(&Image::new(1, 1, PixelFormat::Rgb8, vec![0xff, 0, 0]).unwrap())
            .unwrap();
    assert_eq!(imx_codec_png::decode(&png).unwrap().pixels(), &[0xff, 0, 0]);
    let jpeg = imx_codec_jpeg::encode(
        &Image::new(8, 8, PixelFormat::Rgb8, vec![0xff; 8 * 8 * 3]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        imx_codec_jpeg::decode(&jpeg).unwrap().pixel_format(),
        PixelFormat::Rgb8
    );
}

#[test]
fn farbfeld_and_qoi_codecs_round_trip_shared_core_images() {
    let image = Image::new(
        2,
        2,
        PixelFormat::Rgba16Be,
        vec![
            0x00, 0x00, 0x11, 0x11, 0x22, 0x22, 0xff, 0xff, 0x33, 0x33, 0x44, 0x44, 0x55, 0x55,
            0x66, 0x66, 0x77, 0x77, 0x88, 0x88, 0x99, 0x99, 0xaa, 0xaa, 0xbb, 0xbb, 0xcc, 0xcc,
            0xdd, 0xdd, 0xee, 0xee,
        ],
    )
    .unwrap();

    let ff = imx_codec_farbfeld::encode(&image).unwrap();
    assert_eq!(imx_codec_farbfeld::decode(&ff).unwrap(), image);

    let qoi = imx_codec_qoi::encode_image(&image, imx_codec_qoi::QOI_SRGB).unwrap();
    let decoded_qoi = imx_codec_qoi::decode(&qoi)
        .unwrap()
        .into_core_image()
        .unwrap();
    assert_eq!(decoded_qoi.pixel_format(), PixelFormat::Rgba8);
    assert_eq!(decoded_qoi.to_rgba16be().unwrap(), image);
}

#[test]
fn png_codec_round_trips_shared_core_images() {
    let image = Image::new(
        2,
        1,
        PixelFormat::Rgba16Be,
        vec![
            0x00, 0x00, 0x11, 0x11, 0x22, 0x22, 0xff, 0xff, 0x33, 0x33, 0x44, 0x44, 0x55, 0x55,
            0x66, 0x66,
        ],
    )
    .unwrap();

    let png = imx_codec_png::encode(&image).unwrap();
    assert_eq!(imx_codec_png::decode(&png).unwrap(), image);
}

#[test]
fn identify_metadata_is_stable_for_supported_fields() {
    let image = Image::new(
        1,
        1,
        PixelFormat::Rgba16Be,
        vec![0, 0, 0xff, 0xff, 0, 0, 0xff, 0xff],
    )
    .unwrap();
    let ff = imx_codec_farbfeld::encode(&image).unwrap();
    let qoi = imx_codec_qoi::encode_image(&image, imx_codec_qoi::QOI_SRGB).unwrap();
    let black = Image::new(1, 1, PixelFormat::Bilevel, vec![0]).unwrap();
    let pbm = imx_codec_pnm::encode_pbm(&black).unwrap();
    let ppm = imx_codec_pnm::encode_ppm(&image).unwrap();
    let gray = Image::new(1, 1, PixelFormat::Gray8, vec![0x80]).unwrap();
    let pgm = imx_codec_pnm::encode_pgm(&gray).unwrap();
    let png = imx_codec_png::encode(&image.to_rgba8().unwrap()).unwrap();
    let jpeg = imx_codec_jpeg::encode(&image.to_rgb8().unwrap()).unwrap();

    assert_eq!(
        imx_codec_farbfeld::identify(&ff).unwrap().stable_line(),
        "format=FARBFELD width=1 height=1 channels=RGBA depth=16"
    );
    assert_eq!(
        imx_codec_qoi::identify(&qoi).unwrap().stable_line(),
        "format=QOI width=1 height=1 channels=RGBA depth=8"
    );
    assert_eq!(
        imx_codec_pnm::identify_pbm(&pbm).unwrap().stable_line(),
        "format=PBM width=1 height=1 channels=GRAY depth=1"
    );
    assert_eq!(
        imx_codec_pnm::identify_ppm(&ppm).unwrap().stable_line(),
        "format=PPM width=1 height=1 channels=RGB depth=16"
    );
    assert_eq!(
        imx_codec_pnm::identify_pgm(&pgm).unwrap().stable_line(),
        "format=PGM width=1 height=1 channels=GRAY depth=8"
    );
    assert_eq!(
        imx_codec_png::identify(&png).unwrap().stable_line(),
        "format=PNG width=1 height=1 channels=RGBA depth=8"
    );
    assert_eq!(
        imx_codec_jpeg::identify(&jpeg).unwrap().stable_line(),
        "format=JPEG width=1 height=1 channels=RGB depth=8"
    );
}
