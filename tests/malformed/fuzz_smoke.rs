fn next(seed: &mut u64) -> u8 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed >> 24) as u8
}

#[test]
fn decode_fuzz_smoke_does_not_panic_or_allocate_unboundedly() {
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    for len in 0..2048 {
        let mut bytes = vec![0_u8; len];
        for byte in &mut bytes {
            *byte = next(&mut seed);
        }
        let farbfeld = std::panic::catch_unwind(|| imx_codec_farbfeld::decode(&bytes));
        assert!(farbfeld.is_ok(), "farbfeld decode panicked at len {len}");

        let qoi = std::panic::catch_unwind(|| imx_codec_qoi::decode(&bytes));
        assert!(qoi.is_ok(), "QOI decode panicked at len {len}");

        let ppm = std::panic::catch_unwind(|| imx_codec_pnm::decode_ppm(&bytes));
        assert!(ppm.is_ok(), "PPM decode panicked at len {len}");

        let pbm = std::panic::catch_unwind(|| imx_codec_pnm::decode_pbm(&bytes));
        assert!(pbm.is_ok(), "PBM decode panicked at len {len}");

        let pgm = std::panic::catch_unwind(|| imx_codec_pnm::decode_pgm(&bytes));
        assert!(pgm.is_ok(), "PGM decode panicked at len {len}");
    }
}

#[test]
fn structured_truncation_fuzz_smoke_does_not_panic() {
    let pixels = vec![0x7f; 8 * 8 * 4];
    let qoi = imx_codec_qoi::encode(8, 8, 4, imx_codec_qoi::QOI_SRGB, &pixels).unwrap();
    for len in 0..qoi.len() {
        let result = std::panic::catch_unwind(|| imx_codec_qoi::decode(&qoi[..len]));
        assert!(result.is_ok(), "QOI truncation panicked at len {len}");
    }

    let image =
        imx_core::Image::new(8, 8, imx_core::PixelFormat::Rgba16Be, vec![0x7f; 8 * 8 * 8]).unwrap();
    let farbfeld = imx_codec_farbfeld::encode(&image).unwrap();
    for len in 0..farbfeld.len() {
        let result = std::panic::catch_unwind(|| imx_codec_farbfeld::decode(&farbfeld[..len]));
        assert!(result.is_ok(), "farbfeld truncation panicked at len {len}");
    }

    for pgm in [
        b"P2\n2 2\n15\n0 7 15 3\n".as_slice(),
        b"P5\n2 2\n255\n\x00\x7f\x80\xff".as_slice(),
        b"P5\n2 2\n65535\n\x00\x00\x7f\xff\x80\x00\xff\xff".as_slice(),
    ] {
        for len in 0..pgm.len() {
            let result = std::panic::catch_unwind(|| imx_codec_pnm::decode_pgm(&pgm[..len]));
            assert!(result.is_ok(), "PGM truncation panicked at len {len}");
        }
    }

    for ppm in [
        b"P3\n2 1\n1023\n0 512 1023 1023 256 128\n".as_slice(),
        b"P6\n2 1\n65535\n\x00\x00\x80\x00\xff\xff\xff\xff\x40\x00\x20\x00".as_slice(),
        b"P6\n1 1\n256\n\x00\x00\x01".as_slice(),
    ] {
        for len in 0..ppm.len() {
            let result = std::panic::catch_unwind(|| imx_codec_pnm::decode_ppm(&ppm[..len]));
            assert!(result.is_ok(), "PPM truncation panicked at len {len}");
        }
    }

    for pbm in [
        b"P1\n4 2\n0110\n1001\n".as_slice(),
        b"P4\n9 2\n\xaa\x80\x55\x00".as_slice(),
    ] {
        for len in 0..pbm.len() {
            let result = std::panic::catch_unwind(|| imx_codec_pnm::decode_pbm(&pbm[..len]));
            assert!(result.is_ok(), "PBM truncation panicked at len {len}");
        }
    }
}
