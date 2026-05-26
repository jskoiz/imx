use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use imx_core::{Image, PixelFormat};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    dir.push(format!("imx_cli_{name}_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn imx() -> &'static str {
    env!("CARGO_BIN_EXE_imx")
}

fn prefixed(prefix: &str, path: &Path) -> String {
    format!("{prefix}:{}", path.to_str().unwrap())
}

fn png_fixture(
    path: &Path,
    width: u32,
    height: u32,
    color_type: png::ColorType,
    bit_depth: png::BitDepth,
    pixels: &[u8],
) {
    let file = File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(color_type);
    encoder.set_depth(bit_depth);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(pixels)
        .unwrap();
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

fn write_supported_fixtures(dir: &Path) -> Vec<(&'static str, PathBuf, &'static str)> {
    let ff = dir.join("input.ff");
    let jpeg = dir.join("input.jpg");
    let qoi = dir.join("input.qoi");
    let pbm = dir.join("input.pbm");
    let pgm = dir.join("input.pgm");
    let png = dir.join("input.png");
    let ppm = dir.join("input.ppm");
    let rgba16 = Image::new(
        2,
        1,
        PixelFormat::Rgba16Be,
        vec![
            0x00, 0x00, 0x80, 0x80, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x80, 0x80,
            0xff, 0xff,
        ],
    )
    .unwrap();

    fs::write(&ff, imx_codec_farbfeld::encode(&rgba16).unwrap()).unwrap();
    fs::write(
        &jpeg,
        imx_codec_jpeg::encode(
            &Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    fs::write(
        &qoi,
        imx_codec_qoi::encode_image(&rgba16, imx_codec_qoi::QOI_SRGB).unwrap(),
    )
    .unwrap();
    fs::write(
        &pbm,
        imx_codec_pnm::encode_pbm(&Image::new(2, 1, PixelFormat::Bilevel, vec![0, 255]).unwrap())
            .unwrap(),
    )
    .unwrap();
    fs::write(
        &pgm,
        imx_codec_pnm::encode_pgm(&Image::new(2, 1, PixelFormat::Gray8, vec![0, 255]).unwrap())
            .unwrap(),
    )
    .unwrap();
    fs::write(
        &ppm,
        imx_codec_pnm::encode_ppm(
            &Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    fs::write(
        &png,
        imx_codec_png::encode(
            &Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    vec![
        (
            "FARBFELD",
            ff,
            "format=FARBFELD width=2 height=1 channels=RGBA depth=16",
        ),
        (
            "JPEG",
            jpeg,
            "format=JPEG width=2 height=1 channels=RGB depth=8",
        ),
        (
            "QOI",
            qoi,
            "format=QOI width=2 height=1 channels=RGBA depth=8",
        ),
        (
            "PBM",
            pbm,
            "format=PBM width=2 height=1 channels=GRAY depth=1",
        ),
        (
            "PGM",
            pgm,
            "format=PGM width=2 height=1 channels=GRAY depth=8",
        ),
        (
            "PPM",
            ppm,
            "format=PPM width=2 height=1 channels=RGB depth=8",
        ),
        (
            "PNG",
            png,
            "format=PNG width=2 height=1 channels=RGB depth=8",
        ),
    ]
}

#[test]
fn identifies_farbfeld_qoi_pbm_pgm_and_ppm() {
    let dir = temp_dir("identify");
    let ff = dir.join("input.ff");
    let jpeg = dir.join("input.jpg");
    let qoi = dir.join("input.qoi");
    let pbm = dir.join("input.pbm");
    let ppm = dir.join("input.ppm");
    let png = dir.join("input.png");
    let pgm = dir.join("input.pgm");
    let image = Image::new(
        1,
        1,
        PixelFormat::Rgba16Be,
        vec![0xff, 0xff, 0, 0, 0, 0, 0xff, 0xff],
    )
    .unwrap();
    fs::write(&ff, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();
    fs::write(
        &jpeg,
        imx_codec_jpeg::encode(&Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap())
            .unwrap(),
    )
    .unwrap();
    fs::write(
        &qoi,
        imx_codec_qoi::encode_image(&image, imx_codec_qoi::QOI_SRGB).unwrap(),
    )
    .unwrap();
    fs::write(&ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    fs::write(
        &png,
        imx_codec_png::encode(&image.to_rgba8().unwrap()).unwrap(),
    )
    .unwrap();
    fs::write(
        &pbm,
        imx_codec_pnm::encode_pbm(&Image::new(1, 1, PixelFormat::Bilevel, vec![0]).unwrap())
            .unwrap(),
    )
    .unwrap();
    fs::write(
        &pgm,
        imx_codec_pnm::encode_pgm(&Image::new(1, 1, PixelFormat::Gray8, vec![0x80]).unwrap())
            .unwrap(),
    )
    .unwrap();

    let output = Command::new(imx())
        .args(["identify", ff.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=FARBFELD width=1 height=1 channels=RGBA depth=16"
    );

    let output = Command::new(imx())
        .args(["identify", jpeg.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=JPEG width=1 height=1 channels=RGB depth=8"
    );

    let output = Command::new(imx())
        .args(["identify", qoi.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=QOI width=1 height=1 channels=RGBA depth=8"
    );

    let output = Command::new(imx())
        .args(["identify", ppm.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=PPM width=1 height=1 channels=RGB depth=16"
    );

    let output = Command::new(imx())
        .args(["identify", pbm.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=PBM width=1 height=1 channels=GRAY depth=1"
    );

    let output = Command::new(imx())
        .args(["identify", pgm.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=PGM width=1 height=1 channels=GRAY depth=8"
    );

    let output = Command::new(imx())
        .args(["identify", png.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=PNG width=1 height=1 channels=RGBA depth=8"
    );
}

#[test]
fn identifies_with_exact_format_prefixes_for_supported_formats() {
    let dir = temp_dir("prefixed_identify");
    for (prefix, path, expected_identify) in write_supported_fixtures(&dir) {
        let arg = prefixed(prefix, &path);
        let output = Command::new(imx())
            .args(["identify", arg.as_str()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{prefix} prefixed identify failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            expected_identify
        );
    }
}

#[test]
fn lowercase_colon_path_segments_are_not_format_prefixes() {
    let dir = temp_dir("colon_path");
    let input = dir.join("qoi:input.ppm");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args(["identify", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "colon path identify failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=PPM width=1 height=1 channels=RGB depth=8"
    );
}

#[test]
fn detects_png_by_magic_before_extension_fallback() {
    let dir = temp_dir("png_magic_detection");
    let input = dir.join("input.ppm");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![0, 128, 255]).unwrap();
    fs::write(&input, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args(["identify", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "PNG magic identify failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=PNG width=1 height=1 channels=RGB depth=8"
    );
}

#[test]
fn detects_jpeg_by_magic_before_extension_fallback() {
    let dir = temp_dir("jpeg_magic_detection");
    let input = dir.join("input.ppm");
    let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 128, 255]).unwrap();
    fs::write(&input, imx_codec_jpeg::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args(["identify", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "JPEG magic identify failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=JPEG width=2 height=1 channels=RGB depth=8"
    );
}

#[test]
fn jpeg_exif_orientation_affects_identify_and_transcode_dimensions() {
    let dir = temp_dir("jpeg_exif_orientation");
    let input = dir.join("input.jpg");
    let output_ppm = dir.join("oriented.ppm");
    let image = Image::new(3, 2, PixelFormat::Rgb8, vec![0x80; 3 * 2 * 3]).unwrap();
    let jpeg = imx_codec_jpeg::encode(&image).unwrap();
    fs::write(&input, jpeg_with_exif_orientation(&jpeg, 6)).unwrap();

    let input_arg = prefixed("JPEG", &input);
    let identify = Command::new(imx())
        .args(["identify", input_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        identify.status.success(),
        "JPEG EXIF identify failed with stderr={}",
        String::from_utf8_lossy(&identify.stderr)
    );
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=JPEG width=2 height=3 channels=RGB depth=8"
    );

    let output_arg = prefixed("PPM", &output_ppm);
    let transcode = Command::new(imx())
        .args([input_arg.as_str(), output_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        transcode.status.success(),
        "JPEG EXIF transcode failed with stderr={}",
        String::from_utf8_lossy(&transcode.stderr)
    );
    assert_eq!(
        imx_codec_pnm::identify_ppm(&fs::read(output_ppm).unwrap())
            .unwrap()
            .stable_line(),
        "format=PPM width=2 height=3 channels=RGB depth=8"
    );
}

#[test]
fn transcodes_farbfeld_to_qoi_and_back() {
    let dir = temp_dir("transcode");
    let input_ff = dir.join("input.ff");
    let output_qoi = dir.join("output.qoi");
    let roundtrip_ff = dir.join("roundtrip.ff");
    let image = Image::new(
        2,
        1,
        PixelFormat::Rgba16Be,
        vec![
            0x00, 0x00, 0x80, 0x80, 0xff, 0xff, 0xff, 0xff, 0x12, 0x12, 0x34, 0x34, 0x56, 0x56,
            0x78, 0x78,
        ],
    )
    .unwrap();
    fs::write(&input_ff, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([input_ff.to_str().unwrap(), output_qoi.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        imx_codec_qoi::decode(&fs::read(&output_qoi).unwrap())
            .unwrap()
            .channels,
        4
    );

    let output = Command::new(imx())
        .args([output_qoi.to_str().unwrap(), roundtrip_ff.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(input_ff).unwrap(), fs::read(roundtrip_ff).unwrap());
}

#[test]
fn transcodes_with_exact_format_prefixes_for_supported_formats() {
    let dir = temp_dir("prefixed_transcode");
    for (prefix, input, expected_identify) in write_supported_fixtures(&dir) {
        let output_path = dir.join(format!(
            "rewrite.{}",
            input.extension().unwrap().to_str().unwrap()
        ));
        let input_arg = prefixed(prefix, &input);
        let output_arg = prefixed(prefix, &output_path);

        let output = Command::new(imx())
            .args([input_arg.as_str(), output_arg.as_str()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{prefix} prefixed transcode failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        let identify_arg = prefixed(prefix, &output_path);
        let identify = Command::new(imx())
            .args(["identify", identify_arg.as_str()])
            .output()
            .unwrap();
        assert!(identify.status.success());
        assert_eq!(
            String::from_utf8(identify.stdout).unwrap().trim(),
            expected_identify
        );
    }
}

#[test]
fn transcodes_ppm_to_farbfeld_and_farbfeld_to_ppm() {
    let dir = temp_dir("ppm_transcode");
    let input_ppm = dir.join("input.ppm");
    let output_ff = dir.join("output.ff");
    let roundtrip_ppm = dir.join("roundtrip.ppm");
    let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 128, 255]).unwrap();
    fs::write(&input_ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([input_ppm.to_str().unwrap(), output_ff.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(imx())
        .args([output_ff.to_str().unwrap(), roundtrip_ppm.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let roundtrip = imx_codec_pnm::decode_ppm(&fs::read(roundtrip_ppm).unwrap()).unwrap();
    assert_eq!(roundtrip.pixel_format(), PixelFormat::Rgb16Be);
    assert_eq!(roundtrip.to_rgb8().unwrap().pixels(), image.pixels());
}

#[test]
fn transcodes_png_to_farbfeld_and_farbfeld_to_png() {
    let dir = temp_dir("png_transcode");
    let input_png = dir.join("input.png");
    let output_ff = dir.join("output.ff");
    let roundtrip_png = dir.join("roundtrip.png");
    let image = Image::new(
        2,
        1,
        PixelFormat::Rgba8,
        vec![255, 0, 0, 255, 0, 128, 255, 64],
    )
    .unwrap();
    fs::write(&input_png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([input_png.to_str().unwrap(), output_ff.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        imx_codec_farbfeld::decode(&fs::read(&output_ff).unwrap())
            .unwrap()
            .to_rgba8()
            .unwrap()
            .pixels(),
        image.pixels()
    );

    let output = Command::new(imx())
        .args([output_ff.to_str().unwrap(), roundtrip_png.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        imx_codec_png::decode(&fs::read(roundtrip_png).unwrap())
            .unwrap()
            .to_rgba8()
            .unwrap(),
        image
    );
}

#[test]
fn transcodes_jpeg_to_farbfeld_and_farbfeld_to_jpeg() {
    let dir = temp_dir("jpeg_transcode");
    let input_jpeg = dir.join("input.jpeg");
    let output_ff = dir.join("output.ff");
    let roundtrip_jpeg = dir.join("roundtrip.jpg");
    let image = Image::new(
        8,
        8,
        PixelFormat::Rgb8,
        (0..8)
            .flat_map(|y| {
                (0..8).flat_map(move |x| {
                    [
                        (x * 31 + y * 3) as u8,
                        (x * 5 + y * 29) as u8,
                        (x * 17 + y * 11) as u8,
                    ]
                })
            })
            .collect(),
    )
    .unwrap();
    fs::write(&input_jpeg, imx_codec_jpeg::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([
            prefixed("JPEG", &input_jpeg).as_str(),
            output_ff.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "JPEG->FARBFELD failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        imx_codec_farbfeld::decode(&fs::read(&output_ff).unwrap())
            .unwrap()
            .pixel_format(),
        PixelFormat::Rgba16Be
    );

    let output = Command::new(imx())
        .args([
            output_ff.to_str().unwrap(),
            prefixed("JPEG", &roundtrip_jpeg).as_str(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "FARBFELD->JPEG failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decoded = imx_codec_jpeg::decode(&fs::read(roundtrip_jpeg).unwrap()).unwrap();
    assert_eq!(decoded.width(), 8);
    assert_eq!(decoded.height(), 8);
    assert_eq!(decoded.pixel_format(), PixelFormat::Rgb8);
}

#[test]
fn rejects_jpeg_encode_from_non_opaque_alpha() {
    let dir = temp_dir("jpeg_alpha_reject");
    let input_png = dir.join("input.png");
    let output_jpeg = dir.join("output.jpg");
    let image = Image::new(
        1,
        2,
        PixelFormat::Rgba8,
        vec![255, 0, 0, 255, 0, 0, 255, 128],
    )
    .unwrap();
    fs::write(&input_png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([input_png.to_str().unwrap(), output_jpeg.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("alpha is not supported"));
}

#[test]
fn identifies_sixteen_bit_ppm_with_and_without_prefix() {
    let dir = temp_dir("ppm16_identify");
    let input = dir.join("input.ppm");
    fs::write(
        &input,
        b"P6\n2 1\n65535\n\x12\x34\x56\x78\x9a\xbc\x00\x00\x80\x00\xff\xff",
    )
    .unwrap();

    for arg in [input.to_str().unwrap().to_string(), prefixed("PPM", &input)] {
        let output = Command::new(imx())
            .args(["identify", arg.as_str()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "PPM16 identify failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "format=PPM width=2 height=1 channels=RGB depth=16"
        );
    }
}

#[test]
fn transcodes_sixteen_bit_ppm_to_farbfeld_and_pgm16() {
    let dir = temp_dir("ppm16_transcode");
    let input = dir.join("input.ppm");
    let output_ff = dir.join("output.ff");
    let output_pgm = dir.join("output.pgm");
    let ppm16 = Image::new(
        2,
        1,
        PixelFormat::Rgb16Be,
        vec![
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0x00, 0x00, 0x80, 0x00, 0xff, 0xff,
        ],
    )
    .unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&ppm16).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([input.to_str().unwrap(), output_ff.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "PPM16->FARBFELD failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        imx_codec_farbfeld::decode(&fs::read(&output_ff).unwrap())
            .unwrap()
            .pixels(),
        &[
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xff, 0xff, 0x00, 0x00, 0x80, 0x00, 0xff, 0xff,
            0xff, 0xff,
        ]
    );

    let output = Command::new(imx())
        .args([input.to_str().unwrap(), output_pgm.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "PPM16->PGM failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        imx_codec_pnm::decode_pgm(&fs::read(output_pgm).unwrap())
            .unwrap()
            .pixel_format(),
        PixelFormat::Gray16Be
    );
}

#[test]
fn rewrites_sixteen_bit_ppm_same_format_with_prefix() {
    let dir = temp_dir("ppm16_rewrite");
    let input = dir.join("input.ppm");
    let output_path = dir.join("rewrite.ppm");
    let expected = b"P6\n2 1\n65535\n\x12\x34\x56\x78\x9a\xbc\x00\x00\x80\x00\xff\xff";
    fs::write(&input, expected).unwrap();

    let input_arg = prefixed("PPM", &input);
    let output_arg = prefixed("PPM", &output_path);
    let output = Command::new(imx())
        .args([input_arg.as_str(), output_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "PPM16 same-format rewrite failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&output_path).unwrap(), expected);

    let identify_arg = prefixed("PPM", &output_path);
    let identify = Command::new(imx())
        .args(["identify", identify_arg.as_str()])
        .output()
        .unwrap();
    assert!(identify.status.success());
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=PPM width=2 height=1 channels=RGB depth=16"
    );
}

#[test]
fn transcodes_pbm_to_farbfeld_and_farbfeld_to_pbm() {
    let dir = temp_dir("pbm_transcode");
    let input_pbm = dir.join("input.pbm");
    let output_ff = dir.join("output.ff");
    let roundtrip_pbm = dir.join("roundtrip.pbm");
    let image = Image::new(2, 2, PixelFormat::Bilevel, vec![255, 0, 0, 255]).unwrap();
    fs::write(&input_pbm, imx_codec_pnm::encode_pbm(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([input_pbm.to_str().unwrap(), output_ff.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        imx_codec_farbfeld::decode(&fs::read(&output_ff).unwrap())
            .unwrap()
            .pixels(),
        &[
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0, 0, 0,
            0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
        ]
    );

    let output = Command::new(imx())
        .args([output_ff.to_str().unwrap(), roundtrip_pbm.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        imx_codec_pnm::decode_pbm(&fs::read(roundtrip_pbm).unwrap())
            .unwrap()
            .pixels(),
        image.pixels()
    );
}

#[test]
fn transcodes_pgm_to_farbfeld_and_farbfeld_to_pgm() {
    let dir = temp_dir("pgm_transcode");
    let input_pgm = dir.join("input.pgm");
    let output_ff = dir.join("output.ff");
    let roundtrip_pgm = dir.join("roundtrip.pgm");
    let image = Image::new(2, 1, PixelFormat::Gray8, vec![0, 255]).unwrap();
    fs::write(&input_pgm, imx_codec_pnm::encode_pgm(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([input_pgm.to_str().unwrap(), output_ff.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        imx_codec_farbfeld::decode(&fs::read(&output_ff).unwrap())
            .unwrap()
            .pixels(),
        &[0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
    );

    let output = Command::new(imx())
        .args([output_ff.to_str().unwrap(), roundtrip_pgm.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        imx_codec_pnm::decode_pgm(&fs::read(roundtrip_pgm).unwrap())
            .unwrap()
            .to_gray8()
            .unwrap()
            .pixels(),
        image.pixels()
    );
}

#[test]
fn rewrites_same_format_outputs_for_supported_formats() {
    let dir = temp_dir("same_format_rewrite");
    let image = Image::new(
        2,
        1,
        PixelFormat::Rgba16Be,
        vec![
            0x00, 0x00, 0x80, 0x80, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x80, 0x80,
            0xff, 0xff,
        ],
    )
    .unwrap();

    let ff = dir.join("input.ff");
    let jpeg = dir.join("input.jpg");
    let qoi = dir.join("input.qoi");
    let pbm = dir.join("input.pbm");
    let pgm = dir.join("input.pgm");
    let png = dir.join("input.png");
    let ppm = dir.join("input.ppm");

    fs::write(&ff, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();
    fs::write(
        &jpeg,
        imx_codec_jpeg::encode(
            &Image::new(8, 8, PixelFormat::Rgb8, vec![0x80; 8 * 8 * 3]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    fs::write(
        &qoi,
        imx_codec_qoi::encode_image(&image, imx_codec_qoi::QOI_SRGB).unwrap(),
    )
    .unwrap();
    fs::write(
        &pbm,
        imx_codec_pnm::encode_pbm(&Image::new(2, 1, PixelFormat::Bilevel, vec![0, 255]).unwrap())
            .unwrap(),
    )
    .unwrap();
    fs::write(
        &pgm,
        imx_codec_pnm::encode_pgm(&Image::new(2, 1, PixelFormat::Gray8, vec![0, 255]).unwrap())
            .unwrap(),
    )
    .unwrap();
    fs::write(
        &ppm,
        imx_codec_pnm::encode_ppm(
            &Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    fs::write(
        &png,
        imx_codec_png::encode(&image.to_rgba8().unwrap()).unwrap(),
    )
    .unwrap();

    for (name, input, output_name, expected_identify) in [
        (
            "farbfeld",
            ff.as_path(),
            "output.ff",
            "format=FARBFELD width=2 height=1 channels=RGBA depth=16",
        ),
        (
            "jpeg",
            jpeg.as_path(),
            "output.jpg",
            "format=JPEG width=8 height=8 channels=RGB depth=8",
        ),
        (
            "qoi",
            qoi.as_path(),
            "output.qoi",
            "format=QOI width=2 height=1 channels=RGBA depth=8",
        ),
        (
            "pbm",
            pbm.as_path(),
            "output.pbm",
            "format=PBM width=2 height=1 channels=GRAY depth=1",
        ),
        (
            "pgm",
            pgm.as_path(),
            "output.pgm",
            "format=PGM width=2 height=1 channels=GRAY depth=8",
        ),
        (
            "ppm",
            ppm.as_path(),
            "output.ppm",
            "format=PPM width=2 height=1 channels=RGB depth=8",
        ),
        (
            "png",
            png.as_path(),
            "output.png",
            "format=PNG width=2 height=1 channels=RGBA depth=8",
        ),
    ] {
        let output_path = dir.join(output_name);
        let output = Command::new(imx())
            .args([input.to_str().unwrap(), output_path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name} same-format rewrite failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        let identify = Command::new(imx())
            .args(["identify", output_path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(identify.status.success());
        assert_eq!(
            String::from_utf8(identify.stdout).unwrap().trim(),
            expected_identify
        );
    }
}

#[test]
fn help_and_version_are_available() {
    for flag in ["--help", "--version"] {
        let output = Command::new(imx()).arg(flag).output().unwrap();
        assert!(
            output.status.success(),
            "{flag} failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
        if flag == "--help" {
            let stdout = String::from_utf8(output.stdout).unwrap();
            assert!(stdout.contains(".jpg"));
            assert!(stdout.contains(".jpeg"));
            assert!(stdout.contains("JPEG:"));
            assert!(stdout.contains(".png"));
            assert!(stdout.contains("PNG:"));
        }
    }
}

#[test]
fn malformed_input_exits_nonzero_with_error_prefix() {
    let dir = temp_dir("malformed");
    let bad = dir.join("bad.qoi");
    fs::write(&bad, b"qoif\0\0\0\x01\0\0\0\x01\x02\0").unwrap();
    let output = Command::new(imx())
        .args(["identify", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("error: "));
}

#[test]
fn malformed_png_input_exits_nonzero_with_error_prefix() {
    let dir = temp_dir("malformed_png");
    let bad = dir.join("bad.png");
    fs::write(&bad, imx_codec_png::MAGIC).unwrap();
    let output = Command::new(imx())
        .args(["identify", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("error: "));
    assert!(String::from_utf8_lossy(&output.stderr).contains("PNG identify failed"));
}

#[test]
fn malformed_jpeg_input_exits_nonzero_with_error_prefix() {
    let dir = temp_dir("malformed_jpeg");
    let bad = dir.join("bad.jpg");
    fs::write(&bad, imx_codec_jpeg::MAGIC).unwrap();
    let output = Command::new(imx())
        .args(["identify", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("error: "));
    assert!(String::from_utf8_lossy(&output.stderr).contains("JPEG identify failed"));
}

#[test]
fn malformed_jpeg_exif_orientation_exits_nonzero_with_clear_error() {
    let dir = temp_dir("malformed_jpeg_exif");
    let input = dir.join("bad-exif.jpg");
    let image = Image::new(2, 2, PixelFormat::Rgb8, vec![0x80; 2 * 2 * 3]).unwrap();
    let jpeg = imx_codec_jpeg::encode(&image).unwrap();
    fs::write(
        &input,
        jpeg_with_exif_app1(&jpeg, b"Exif\0\0ZZ\0*\0\0\0\x08"),
    )
    .unwrap();

    let input_arg = prefixed("JPEG", &input);
    let output = Command::new(imx())
        .args(["identify", input_arg.as_str()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("error: "));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("JPEG EXIF Orientation metadata is malformed"));
}

#[test]
fn grayscale_alpha_png_identify_and_transcode_are_supported() {
    let dir = temp_dir("png_gray_alpha");
    let png = dir.join("gray-alpha.png");
    let ff = dir.join("gray-alpha.ff");
    png_fixture(
        &png,
        2,
        1,
        png::ColorType::GrayscaleAlpha,
        png::BitDepth::Eight,
        &[0x20, 0x80, 0xff, 0x40],
    );

    let identify = Command::new(imx())
        .args(["identify", &prefixed("PNG", &png)])
        .output()
        .unwrap();
    assert!(
        identify.status.success(),
        "identify failed with stderr={}",
        String::from_utf8_lossy(&identify.stderr)
    );
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=PNG width=2 height=1 channels=RGBA depth=8"
    );

    let transcode = Command::new(imx())
        .args([prefixed("PNG", &png), prefixed("FARBFELD", &ff)])
        .output()
        .unwrap();
    assert!(
        transcode.status.success(),
        "transcode failed with stderr={}",
        String::from_utf8_lossy(&transcode.stderr)
    );
    let identify_ff = Command::new(imx())
        .args(["identify", &prefixed("FARBFELD", &ff)])
        .output()
        .unwrap();
    assert!(identify_ff.status.success());
    assert_eq!(
        String::from_utf8(identify_ff.stdout).unwrap().trim(),
        "format=FARBFELD width=2 height=1 channels=RGBA depth=16"
    );
}

#[test]
fn failed_transcode_does_not_leave_output_file() {
    let dir = temp_dir("no_partial_output");
    let bad = dir.join("bad.ppm");
    let output_path = dir.join("out.ff");
    fs::write(&bad, b"P6\n2 1\n255\n\xff\x00\x00").unwrap();

    let output = Command::new(imx())
        .args([bad.to_str().unwrap(), output_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("error: "));
    assert!(!output_path.exists());
}

#[test]
fn same_input_and_output_path_is_rejected() {
    let dir = temp_dir("same_path");
    let input = dir.join("input.ppm");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([input.to_str().unwrap(), input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be different"));
}

#[test]
fn same_prefixed_png_input_and_output_path_is_rejected() {
    let dir = temp_dir("same_png_path");
    let input = dir.join("input.png");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&input, imx_codec_png::encode(&image).unwrap()).unwrap();
    let arg = prefixed("PNG", &input);

    let output = Command::new(imx())
        .args([arg.as_str(), arg.as_str()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be different"));
}

#[test]
fn same_prefixed_jpeg_input_and_output_path_is_rejected() {
    let dir = temp_dir("same_jpeg_path");
    let input = dir.join("input.jpeg");
    let image = Image::new(8, 8, PixelFormat::Rgb8, vec![0x80; 8 * 8 * 3]).unwrap();
    fs::write(&input, imx_codec_jpeg::encode(&image).unwrap()).unwrap();
    let arg = prefixed("JPEG", &input);

    let output = Command::new(imx())
        .args([arg.as_str(), arg.as_str()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be different"));
}

#[test]
fn malformed_format_prefixes_are_rejected() {
    let dir = temp_dir("malformed_prefixes");
    let ppm = dir.join("input.ppm");
    let jpeg = dir.join("input.jpg");
    let png = dir.join("input.png");
    let qoi = dir.join("input.qoi");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    fs::write(&jpeg, imx_codec_jpeg::encode(&image).unwrap()).unwrap();
    fs::write(&png, imx_codec_png::encode(&image).unwrap()).unwrap();
    fs::write(
        &qoi,
        imx_codec_qoi::encode_image(&image, imx_codec_qoi::QOI_SRGB).unwrap(),
    )
    .unwrap();

    let output_ppm = dir.join("out.ppm");
    let output_jpeg = dir.join("out.jpg");
    let output_png = dir.join("out.png");
    let extensionless_output = dir.join("out");
    let cases = vec![
        (
            vec!["identify".to_string(), prefixed("GIF", &ppm)],
            "unsupported format prefix: GIF",
        ),
        (
            vec!["identify".to_string(), prefixed("JPG", &jpeg)],
            "unsupported format prefix: JPG",
        ),
        (
            vec!["identify".to_string(), "JPEG:".to_string()],
            "missing path after format prefix JPEG:",
        ),
        (
            vec!["identify".to_string(), "PNG:".to_string()],
            "missing path after format prefix PNG:",
        ),
        (
            vec!["identify".to_string(), prefixed("JPEG", &png)],
            "format prefix JPEG does not match detected format PNG",
        ),
        (
            vec!["identify".to_string(), prefixed("PNG", &ppm)],
            "format prefix PNG does not match detected format PPM",
        ),
        (
            vec!["identify".to_string(), prefixed("PPM", &qoi)],
            "format prefix PPM does not match detected format QOI",
        ),
        (
            vec![prefixed("PNG", &png), prefixed("PNG", &output_jpeg)],
            "format prefix PNG does not match path format JPEG",
        ),
        (
            vec![prefixed("PNG", &png), prefixed("PPM", &output_png)],
            "format prefix PPM does not match path format PNG",
        ),
        (
            vec![prefixed("PPM", &ppm), prefixed("QOI", &output_ppm)],
            "format prefix QOI does not match path format PPM",
        ),
        (
            vec![
                prefixed("JPEG", &jpeg),
                prefixed("JPEG", &extensionless_output),
            ],
            "unsupported format:",
        ),
        (
            vec![
                prefixed("PNG", &png),
                prefixed("PNG", &extensionless_output),
            ],
            "unsupported format:",
        ),
        (
            vec![
                prefixed("PPM", &ppm),
                prefixed("QOI", &extensionless_output),
            ],
            "unsupported format:",
        ),
        (
            vec![prefixed("PPM", &ppm), prefixed("PPM", &ppm)],
            "input and output paths must be different",
        ),
    ];

    for (args, expected_error) in cases {
        let output = Command::new(imx()).args(&args).output().unwrap();
        assert!(
            !output.status.success(),
            "malformed prefix case unexpectedly succeeded: {args:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "expected stderr to contain {expected_error:?}, got {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn oversized_input_is_rejected_before_reading() {
    let dir = temp_dir("oversized");
    let huge = dir.join("huge.ff");
    let file = File::create(&huge).unwrap();
    file.set_len((imx_core::MAX_PIXEL_BYTES as u64) + 1024 * 1024 + 1)
        .unwrap();

    let output = Command::new(imx())
        .args(["identify", huge.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("input file too large"));
}
