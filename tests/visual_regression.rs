use imx_core::{Image, PixelFormat, ResizeFilter};

fn asymmetric_rgb8() -> Image {
    Image::new(
        5,
        3,
        PixelFormat::Rgb8,
        (0..45)
            .map(|value| ((value * 37 + 11) % 256) as u8)
            .collect(),
    )
    .unwrap()
}

#[test]
fn resize_filters_match_golden_pixels_for_gray_ramp() {
    let image = Image::new(4, 1, PixelFormat::Gray8, vec![0, 50, 200, 250]).unwrap();

    for (filter, expected) in [
        (ResizeFilter::Point, &[50, 250][..]),
        (ResizeFilter::Box, &[25, 225][..]),
        (ResizeFilter::Triangle, &[50, 200][..]),
        (ResizeFilter::CatmullRom, &[38, 212][..]),
        (ResizeFilter::Lanczos3, &[34, 216][..]),
    ] {
        let resized = image.resize_filtered(2, 1, filter).unwrap();
        assert_eq!(resized.pixel_format(), PixelFormat::Gray8, "{filter:?}");
        assert_eq!(resized.pixels(), expected, "{filter:?}");
    }
}

#[test]
fn resize_filters_match_golden_pixels_for_asymmetric_rgb_image() {
    let image = asymmetric_rgb8();

    for (filter, expected) in [
        (
            ResizeFilter::Point,
            &[
                11, 48, 85, 233, 14, 51, 199, 236, 17, 97, 134, 171, 63, 100, 137, 29, 66, 103,
            ][..],
        ),
        (
            ResizeFilter::Box,
            &[
                88, 125, 162, 127, 36, 73, 165, 138, 111, 131, 168, 141, 42, 79, 116, 144, 117, 154,
            ][..],
        ),
        (
            ResizeFilter::Triangle,
            &[
                75, 112, 149, 139, 87, 124, 168, 150, 95, 128, 165, 134, 104, 141, 142, 130, 112,
                149,
            ][..],
        ),
        (
            ResizeFilter::CatmullRom,
            &[
                71, 108, 150, 140, 84, 130, 171, 150, 89, 131, 168, 131, 103, 150, 146, 127, 106,
                154,
            ][..],
        ),
        (
            ResizeFilter::Lanczos3,
            &[
                73, 108, 150, 139, 85, 139, 172, 146, 83, 132, 167, 126, 105, 160, 150, 125, 99,
                159,
            ][..],
        ),
    ] {
        let resized = image.resize_filtered(3, 2, filter).unwrap();
        assert_eq!(resized.width(), 3, "{filter:?}");
        assert_eq!(resized.height(), 2, "{filter:?}");
        assert_eq!(resized.pixel_format(), PixelFormat::Rgb8, "{filter:?}");
        assert_eq!(resized.pixels(), expected, "{filter:?}");
    }
}

#[test]
fn color_tone_ops_match_golden_rgba_pixels() {
    let image = Image::new(
        2,
        2,
        PixelFormat::Rgba8,
        vec![
            10, 20, 30, 40, 100, 120, 140, 160, 200, 180, 160, 140, 250, 128, 0, 64,
        ],
    )
    .unwrap();

    assert_eq!(
        image.grayscale().unwrap().pixels(),
        &[19, 19, 19, 40, 117, 117, 117, 160, 183, 183, 183, 140, 145, 145, 145, 64,]
    );
    assert_eq!(
        image.invert().unwrap().pixels(),
        &[245, 235, 225, 40, 155, 135, 115, 160, 55, 75, 95, 140, 5, 127, 255, 64]
    );
    assert_eq!(
        image.brightness(20).unwrap().pixels(),
        &[30, 40, 50, 40, 120, 140, 160, 160, 220, 200, 180, 140, 255, 148, 20, 64]
    );
    assert_eq!(
        image.contrast(1.5).unwrap().pixels(),
        &[0, 0, 0, 40, 86, 116, 146, 160, 236, 206, 176, 140, 255, 128, 0, 64]
    );
}

#[test]
fn scalar_color_ops_match_golden_gray_pixels() {
    let image = Image::new(4, 1, PixelFormat::Gray8, vec![0, 64, 128, 255]).unwrap();

    assert_eq!(image.gamma(2.0).unwrap().pixels(), &[0, 128, 181, 255]);
    assert_eq!(image.threshold(128).unwrap().pixels(), &[0, 0, 255, 255]);
    assert_eq!(
        image.levels(64, 192, 1.0).unwrap().pixels(),
        &[0, 0, 128, 255]
    );
}

#[test]
fn geometry_pipeline_matches_golden_pixels() {
    let image = asymmetric_rgb8();
    let output = image
        .crop(1, 0, 3, 3)
        .unwrap()
        .rotate_90()
        .unwrap()
        .flip_vertical()
        .unwrap()
        .flop_horizontal()
        .unwrap();

    assert_eq!(output.width(), 3);
    assert_eq!(output.height(), 3);
    assert_eq!(output.pixel_format(), PixelFormat::Rgb8);
    assert_eq!(
        output.pixels(),
        &[
            88, 125, 162, 131, 168, 205, 174, 211, 248, 233, 14, 51, 20, 57, 94, 63, 100, 137, 122,
            159, 196, 165, 202, 239, 208, 245, 26,
        ]
    );
}
