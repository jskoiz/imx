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

        let ppm = std::panic::catch_unwind(|| imx_codec_ppm::decode(&bytes));
        assert!(ppm.is_ok(), "PPM decode panicked at len {len}");
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
}
