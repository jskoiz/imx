use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use imx_core::{Image, PixelFormat};

#[path = "../src/progressive_jpeg_fixtures.rs"]
#[allow(dead_code)]
mod progressive_jpeg_fixtures;

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

fn assert_failure(output: std::process::Output, code: i32, expected_stderr: &str) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "expected exit code {code}, got {:?}; stdout={:?}; stderr={:?}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("error: ") || code == 2,
        "expected error prefix for exit {code}, got {stderr:?}"
    );
    assert!(
        stderr.contains(expected_stderr),
        "expected stderr to contain {expected_stderr:?}, got {stderr:?}"
    );
}

fn prefixed(prefix: &str, path: &Path) -> String {
    format!("{prefix}:{}", path.to_str().unwrap())
}

fn identify_json_from_stable_line(stable_line: &str) -> String {
    let mut format = "";
    let mut width = "";
    let mut height = "";
    let mut channels = "";
    let mut depth = "";
    for field in stable_line.split_whitespace() {
        if let Some(value) = field.strip_prefix("format=") {
            format = value;
        } else if let Some(value) = field.strip_prefix("width=") {
            width = value;
        } else if let Some(value) = field.strip_prefix("height=") {
            height = value;
        } else if let Some(value) = field.strip_prefix("channels=") {
            channels = value;
        } else if let Some(value) = field.strip_prefix("depth=") {
            depth = value;
        }
    }
    format!(
        "{{\"schema_version\":1,\"format\":\"{format}\",\"width\":{width},\"height\":{height},\"channels\":\"{channels}\",\"depth\":{depth}}}"
    )
}

fn report_json_from_stable_line(stable_line: &str) -> String {
    identify_json_from_stable_line(stable_line).replacen(
        "{\"schema_version\":1,",
        "{\"schema_version\":1,\"status\":\"supported\",\"diagnostic_code\":null,",
        1,
    )
}

fn report_unsupported_json(code: &str, message: &str) -> String {
    format!(
        "{{\"schema_version\":1,\"status\":\"unsupported\",\"diagnostic_code\":\"{code}\",\"message\":\"{message}\"}}"
    )
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

fn write_supported_fixtures(dir: &Path) -> Vec<(&'static str, PathBuf, &'static str)> {
    let ff = dir.join("input.ff");
    let bmp = dir.join("input.bmp");
    let jpeg = dir.join("input.jpg");
    let qoi = dir.join("input.qoi");
    let pbm = dir.join("input.pbm");
    let pgm = dir.join("input.pgm");
    let png = dir.join("input.png");
    let ppm = dir.join("input.ppm");
    let tiff = dir.join("input.tiff");
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
    fs::write(&bmp, imx_codec_bmp::encode(&rgba16).unwrap()).unwrap();
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
    fs::write(
        &tiff,
        imx_codec_tiff::encode(
            &Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    vec![
        (
            "BMP",
            bmp,
            "format=BMP width=2 height=1 channels=RGBA depth=8",
        ),
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
        (
            "TIFF",
            tiff,
            "format=TIFF width=2 height=1 channels=RGB depth=8",
        ),
    ]
}

#[test]
fn identifies_farbfeld_qoi_pbm_pgm_and_ppm() {
    let dir = temp_dir("identify");
    let ff = dir.join("input.ff");
    let bmp = dir.join("input.bmp");
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
    fs::write(&bmp, imx_codec_bmp::encode(&image).unwrap()).unwrap();
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
        .args(["identify", bmp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=BMP width=1 height=1 channels=RGBA depth=8"
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
fn identifies_json_for_supported_formats_with_and_without_prefixes() {
    let dir = temp_dir("identify_json");
    for (prefix, path, expected_identify) in write_supported_fixtures(&dir) {
        for arg in [path.to_string_lossy().into_owned(), prefixed(prefix, &path)] {
            let output = Command::new(imx())
                .args(["identify", "--json", arg.as_str()])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{prefix} JSON identify failed for {arg} with stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                identify_json_from_stable_line(expected_identify)
            );
        }
    }
}

#[test]
fn report_json_summarizes_supported_inputs() {
    let dir = temp_dir("report_json_supported");
    for (prefix, path, expected_identify) in write_supported_fixtures(&dir) {
        for arg in [path.to_string_lossy().into_owned(), prefixed(prefix, &path)] {
            let output = Command::new(imx())
                .args(["report", "--json", arg.as_str()])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{prefix} JSON report failed for {arg} with stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                report_json_from_stable_line(expected_identify)
            );
        }
    }
}

#[test]
fn farbfeld_extension_alias_identifies_and_transcodes() {
    let dir = temp_dir("farbfeld_extension_alias");
    let input = dir.join("input.farbfeld");
    let output = dir.join("rewrite.farbfeld");
    let image = Image::new(
        1,
        1,
        PixelFormat::Rgba16Be,
        vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xff, 0xff],
    )
    .unwrap();
    fs::write(&input, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();

    let input_arg = prefixed("FARBFELD", &input);
    let output_arg = prefixed("FARBFELD", &output);

    let identify = Command::new(imx())
        .args(["identify", input_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        identify.status.success(),
        "farbfeld alias identify failed with stderr={}",
        String::from_utf8_lossy(&identify.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&identify.stdout).trim(),
        "format=FARBFELD width=1 height=1 channels=RGBA depth=16"
    );
    let identify_json = Command::new(imx())
        .args(["identify", "--json", input_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        identify_json.status.success(),
        "farbfeld alias JSON identify failed with stderr={}",
        String::from_utf8_lossy(&identify_json.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&identify_json.stdout).trim(),
        "{\"schema_version\":1,\"format\":\"FARBFELD\",\"width\":1,\"height\":1,\"channels\":\"RGBA\",\"depth\":16}"
    );
    let report_json = Command::new(imx())
        .args(["report", "--json", input_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        report_json.status.success(),
        "farbfeld alias JSON report failed with stderr={}",
        String::from_utf8_lossy(&report_json.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&report_json.stdout).trim(),
        "{\"schema_version\":1,\"status\":\"supported\",\"diagnostic_code\":null,\"format\":\"FARBFELD\",\"width\":1,\"height\":1,\"channels\":\"RGBA\",\"depth\":16}"
    );

    let transcode = Command::new(imx())
        .args([input_arg.as_str(), output_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        transcode.status.success(),
        "farbfeld alias transcode failed with stderr={}",
        String::from_utf8_lossy(&transcode.stderr)
    );
    assert_eq!(fs::read(input).unwrap(), fs::read(output).unwrap());
}

#[test]
fn jpeg_extension_alias_reports_json() {
    let dir = temp_dir("jpeg_extension_alias_json");
    let input = dir.join("input.jpeg");
    let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap();
    fs::write(&input, imx_codec_jpeg::encode(&image).unwrap()).unwrap();
    let input_arg = prefixed("JPEG", &input);

    let identify = Command::new(imx())
        .args(["identify", "--json", input_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        identify.status.success(),
        "jpeg alias JSON identify failed with stderr={}",
        String::from_utf8_lossy(&identify.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&identify.stdout).trim(),
        "{\"schema_version\":1,\"format\":\"JPEG\",\"width\":2,\"height\":1,\"channels\":\"RGB\",\"depth\":8}"
    );

    let report = Command::new(imx())
        .args(["report", "--json", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "jpeg alias JSON report failed with stderr={}",
        String::from_utf8_lossy(&report.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&report.stdout).trim(),
        "{\"schema_version\":1,\"status\":\"supported\",\"diagnostic_code\":null,\"format\":\"JPEG\",\"width\":2,\"height\":1,\"channels\":\"RGB\",\"depth\":8}"
    );
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
fn detects_magic_before_extension_fallback_for_all_formats() {
    let dir = temp_dir("magic_detection");
    for (prefix, original, expected_identify) in write_supported_fixtures(&dir) {
        let misleading_extension = if prefix == "PPM" { "qoi" } else { "ppm" };
        let misleading = dir.join(format!(
            "{}-misleading.{}",
            prefix.to_ascii_lowercase(),
            misleading_extension
        ));
        fs::write(&misleading, fs::read(&original).unwrap()).unwrap();

        for arg in [
            misleading.to_str().unwrap().to_string(),
            prefixed(prefix, &misleading),
        ] {
            let output = Command::new(imx())
                .args(["identify", arg.as_str()])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{prefix} magic identify failed for {arg} with stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                expected_identify
            );
        }
    }
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
    let identify_json = Command::new(imx())
        .args(["identify", "--json", input_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        identify_json.status.success(),
        "JPEG EXIF JSON identify failed with stderr={}",
        String::from_utf8_lossy(&identify_json.stderr)
    );
    assert_eq!(
        String::from_utf8(identify_json.stdout).unwrap().trim(),
        "{\"schema_version\":1,\"format\":\"JPEG\",\"width\":2,\"height\":3,\"channels\":\"RGB\",\"depth\":8}"
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
fn camera_style_little_endian_exif_orientation_affects_identify_and_transcode_dimensions() {
    let dir = temp_dir("jpeg_camera_exif_orientation");
    let input = dir.join("input.jpg");
    let output_ppm = dir.join("oriented.ppm");
    let image = Image::new(3, 2, PixelFormat::Rgb8, vec![0x80; 3 * 2 * 3]).unwrap();
    let jpeg = imx_codec_jpeg::encode(&image).unwrap();
    fs::write(&input, jpeg_with_camera_exif_orientation_le(&jpeg, 6)).unwrap();

    let input_arg = prefixed("JPEG", &input);
    let identify = Command::new(imx())
        .args(["identify", input_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        identify.status.success(),
        "camera-style JPEG EXIF identify failed with stderr={}",
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
        "camera-style JPEG EXIF transcode failed with stderr={}",
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
fn top_down_bmp_identify_and_transcode_preserve_logical_rows() {
    let dir = temp_dir("top_down_bmp");
    let input = dir.join("input.bmp");
    let output = dir.join("output.ff");
    let image = Image::new(
        3,
        2,
        PixelFormat::Rgb8,
        vec![
            255, 0, 0, 0, 255, 0, 0, 0, 255, 12, 34, 56, 78, 90, 123, 222, 111, 3,
        ],
    )
    .unwrap();
    let bmp = top_down_bmp(imx_codec_bmp::encode(&image).unwrap(), 3, 2, 3);
    fs::write(&input, bmp).unwrap();

    let input_arg = prefixed("BMP", &input);
    let identify = Command::new(imx())
        .args(["identify", input_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        identify.status.success(),
        "top-down BMP identify failed with stderr={}",
        String::from_utf8_lossy(&identify.stderr)
    );
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=BMP width=3 height=2 channels=RGB depth=8"
    );

    let output_arg = prefixed("FARBFELD", &output);
    let transcode = Command::new(imx())
        .args([input_arg.as_str(), output_arg.as_str()])
        .output()
        .unwrap();
    assert!(
        transcode.status.success(),
        "top-down BMP transcode failed with stderr={}",
        String::from_utf8_lossy(&transcode.stderr)
    );
    assert_eq!(
        imx_codec_farbfeld::decode(&fs::read(output).unwrap()).unwrap(),
        image.to_rgba16be().unwrap()
    );
}

#[test]
fn progressive_jpeg_identify_and_transcode_keep_prefix_and_orientation_behavior() {
    let dir = temp_dir("progressive_jpeg");
    let rgb = dir.join("progressive-rgb.jpg");
    let gray = dir.join("progressive-gray.jpg");
    let oriented = dir.join("progressive-o6.jpg");
    let rgb_ppm = dir.join("progressive-rgb.ppm");
    let gray_pgm = dir.join("progressive-gray.pgm");
    let oriented_ppm = dir.join("progressive-o6.ppm");
    let rgb_jpeg = progressive_jpeg_fixtures::progressive_rgb_jpeg();
    let gray_jpeg = progressive_jpeg_fixtures::progressive_gray_jpeg();
    assert!(progressive_jpeg_fixtures::is_progressive_jpeg(&rgb_jpeg));
    assert!(progressive_jpeg_fixtures::is_progressive_jpeg(&gray_jpeg));
    fs::write(&rgb, &rgb_jpeg).unwrap();
    fs::write(&gray, &gray_jpeg).unwrap();
    fs::write(&oriented, jpeg_with_exif_orientation(&rgb_jpeg, 6)).unwrap();

    let rgb_identify = Command::new(imx())
        .args(["identify", prefixed("JPEG", &rgb).as_str()])
        .output()
        .unwrap();
    assert!(
        rgb_identify.status.success(),
        "progressive RGB identify failed with stderr={}",
        String::from_utf8_lossy(&rgb_identify.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&rgb_identify.stdout).trim(),
        "format=JPEG width=4 height=3 channels=RGB depth=8"
    );

    let gray_identify = Command::new(imx())
        .args(["identify", prefixed("JPEG", &gray).as_str()])
        .output()
        .unwrap();
    assert!(
        gray_identify.status.success(),
        "progressive gray identify failed with stderr={}",
        String::from_utf8_lossy(&gray_identify.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&gray_identify.stdout).trim(),
        "format=JPEG width=4 height=2 channels=GRAY depth=8"
    );

    let oriented_identify = Command::new(imx())
        .args(["identify", prefixed("JPEG", &oriented).as_str()])
        .output()
        .unwrap();
    assert!(
        oriented_identify.status.success(),
        "progressive orientation identify failed with stderr={}",
        String::from_utf8_lossy(&oriented_identify.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&oriented_identify.stdout).trim(),
        "format=JPEG width=3 height=4 channels=RGB depth=8"
    );

    for (input, output) in [
        (prefixed("JPEG", &rgb), prefixed("PPM", &rgb_ppm)),
        (prefixed("JPEG", &gray), prefixed("PGM", &gray_pgm)),
        (prefixed("JPEG", &oriented), prefixed("PPM", &oriented_ppm)),
    ] {
        let result = Command::new(imx())
            .args([input.as_str(), output.as_str()])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "progressive JPEG transcode failed with stderr={}",
            String::from_utf8_lossy(&result.stderr)
        );
    }

    let oriented_ppm_identify = Command::new(imx())
        .args(["identify", prefixed("PPM", &oriented_ppm).as_str()])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&oriented_ppm_identify.stdout).trim(),
        "format=PPM width=3 height=4 channels=RGB depth=8"
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
fn transcodes_png_to_tiff_and_back() {
    let dir = temp_dir("png_tiff_transcode");
    let input_png = dir.join("input.png");
    let output_tiff = dir.join("output.tiff");
    let roundtrip_png = dir.join("roundtrip.png");
    let image = Image::new(
        3,
        2,
        PixelFormat::Rgb8,
        vec![
            255, 0, 0, 0, 255, 0, 0, 0, 255, 10, 20, 30, 40, 50, 60, 70, 80, 90,
        ],
    )
    .unwrap();
    fs::write(&input_png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([input_png.to_str().unwrap(), output_tiff.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let identify = Command::new(imx())
        .args(["identify", output_tiff.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(identify.status.success());
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=TIFF width=3 height=2 channels=RGB depth=8"
    );

    let output = Command::new(imx())
        .args([
            output_tiff.to_str().unwrap(),
            roundtrip_png.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decoded = imx_codec_png::decode(&fs::read(&roundtrip_png).unwrap()).unwrap();
    assert_eq!(decoded.pixels(), image.pixels());
}

#[test]
fn transcodes_with_tiff_prefix_and_stdin_extension() {
    let dir = temp_dir("tiff_prefix_stream");
    let input_png = dir.join("input.png");
    let output_tiff = dir.join("output.tif");
    let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap();
    fs::write(&input_png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([
            prefixed("PNG", &input_png).as_str(),
            prefixed("TIFF", &output_tiff).as_str(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decoded = imx_codec_tiff::decode(&fs::read(&output_tiff).unwrap()).unwrap();
    assert_eq!(decoded.pixel_format(), PixelFormat::Rgb8);
    assert_eq!(decoded.pixels(), image.pixels());
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
fn resizes_with_exact_dimensions_for_supported_formats() {
    let dir = temp_dir("resize_supported");
    for (prefix, input, expected_identify) in write_supported_fixtures(&dir) {
        let output_path = dir.join(format!(
            "resized.{}",
            input.extension().unwrap().to_str().unwrap()
        ));
        let input_arg = prefixed(prefix, &input);
        let output_arg = prefixed(prefix, &output_path);

        let output = Command::new(imx())
            .args(["resize", "3x2", input_arg.as_str(), output_arg.as_str()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{prefix} prefixed resize failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        let expected_identify = expected_identify
            .replace("width=2", "width=3")
            .replace("height=1", "height=2");
        let identify = Command::new(imx())
            .args(["identify", prefixed(prefix, &output_path).as_str()])
            .output()
            .unwrap();
        assert!(
            identify.status.success(),
            "{prefix} resized identify failed with stderr={}",
            String::from_utf8_lossy(&identify.stderr)
        );
        assert_eq!(
            String::from_utf8(identify.stdout).unwrap().trim(),
            expected_identify
        );
    }
}

#[test]
fn resize_uses_center_sampled_nearest_neighbor_pixels() {
    let dir = temp_dir("resize_nearest_pixels");
    let input = dir.join("input.ppm");
    let output_path = dir.join("output.ppm");
    let image = Image::new(
        3,
        1,
        PixelFormat::Rgb8,
        vec![255, 0, 0, 0, 255, 0, 0, 0, 255],
    )
    .unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([
            "resize",
            "2x1",
            prefixed("PPM", &input).as_str(),
            prefixed("PPM", &output_path).as_str(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "resize failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let resized = imx_codec_pnm::decode_ppm(&fs::read(output_path).unwrap()).unwrap();
    assert_eq!(resized.width(), 2);
    assert_eq!(resized.height(), 1);
    assert_eq!(resized.pixel_format(), PixelFormat::Rgb8);
    assert_eq!(resized.pixels(), &[255, 0, 0, 0, 0, 255]);
}

#[test]
fn resize_geometry_shorthands_produce_expected_dimensions() {
    let dir = temp_dir("resize_geometry_shorthands");
    let input = dir.join("input.ppm");
    let mut pixels = Vec::with_capacity(100 * 40 * 3);
    for y in 0..40u32 {
        for x in 0..100u32 {
            pixels.push((x * 255 / 99) as u8);
            pixels.push((y * 255 / 39) as u8);
            pixels.push(128);
        }
    }
    let image = Image::new(100, 40, PixelFormat::Rgb8, pixels).unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    for (geometry, expected_width, expected_height) in [
        ("50%", 50, 20),
        ("200x", 200, 80),
        ("x10", 25, 10),
        ("100x40", 100, 40),
    ] {
        let output_path = dir.join(format!("out_{expected_width}x{expected_height}.ppm"));
        let output = Command::new(imx())
            .args([
                "resize",
                geometry,
                prefixed("PPM", &input).as_str(),
                prefixed("PPM", &output_path).as_str(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "resize {geometry} failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let resized = imx_codec_pnm::decode_ppm(&fs::read(&output_path).unwrap()).unwrap();
        assert_eq!(resized.width(), expected_width, "geometry {geometry}");
        assert_eq!(resized.height(), expected_height, "geometry {geometry}");
    }
}

#[test]
fn malformed_resize_arguments_are_rejected() {
    let dir = temp_dir("resize_malformed_args");
    let input = dir.join("input.ppm");
    let output_path = dir.join("output.ppm");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    let expected_error = "invalid resize geometry";
    for dimensions in [
        "",
        "2",
        "2X2",
        "0x2",
        "2x0",
        "0%",
        "50%%",
        "%50",
        "abc",
        "x0",
        "0x",
        "1.5x2",
        "50.0%",
        "10x10x10",
        "4294967296x2",
        "2x4294967296",
        "4294967296%",
    ] {
        let output = Command::new(imx())
            .args([
                "resize",
                dimensions,
                input.to_str().unwrap(),
                output_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(2),
            "malformed resize geometry should exit 2: {dimensions:?}; stderr={:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "expected stderr to contain {expected_error:?}, got {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output_path.exists());
    }
}

#[test]
fn resize_rejects_same_input_and_output_path() {
    let dir = temp_dir("resize_same_path");
    let input = dir.join("input.ppm");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    let arg = prefixed("PPM", &input);

    let output = Command::new(imx())
        .args(["resize", "2x2", arg.as_str(), arg.as_str()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be different"));
}

#[test]
fn resize_prefix_errors_match_identify_and_transcode_contract() {
    let dir = temp_dir("resize_prefix_errors");
    let ppm = dir.join("input.ppm");
    let png = dir.join("input.png");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    fs::write(&png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output_ppm = dir.join("out.ppm");
    let output_png = dir.join("out.png");
    let extensionless_output = dir.join("out");
    let cases = vec![
        (
            vec![
                "resize".to_string(),
                "2x2".to_string(),
                prefixed("XYZ", &ppm),
                prefixed("PPM", &output_ppm),
            ],
            "unsupported format prefix: XYZ",
        ),
        (
            vec![
                "resize".to_string(),
                "2x2".to_string(),
                "PNG:".to_string(),
                prefixed("PNG", &output_png),
            ],
            "missing path after format prefix PNG:",
        ),
        (
            vec![
                "resize".to_string(),
                "2x2".to_string(),
                prefixed("PNG", &ppm),
                prefixed("PPM", &output_ppm),
            ],
            "format prefix PNG does not match detected format PPM",
        ),
        (
            vec![
                "resize".to_string(),
                "2x2".to_string(),
                prefixed("PNG", &png),
                prefixed("PPM", &output_png),
            ],
            "format prefix PPM does not match path format PNG",
        ),
        (
            vec![
                "resize".to_string(),
                "2x2".to_string(),
                prefixed("PNG", &png),
                prefixed("PNG", &extensionless_output),
            ],
            "unsupported format:",
        ),
        (
            vec![
                "resize".to_string(),
                "2x2".to_string(),
                prefixed("PPM", &ppm),
                prefixed("PPM", &ppm),
            ],
            "input and output paths must be different",
        ),
    ];

    for (args, expected_error) in cases {
        let output = Command::new(imx()).args(&args).output().unwrap();
        assert!(
            !output.status.success(),
            "malformed resize prefix case unexpectedly succeeded: {args:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "expected stderr to contain {expected_error:?}, got {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn resize_fit_preserves_aspect_for_supported_formats() {
    let dir = temp_dir("resize_fit_supported");
    for (prefix, input, expected_identify) in write_supported_fixtures(&dir) {
        let output_path = dir.join(format!(
            "fit.{}",
            input.extension().unwrap().to_str().unwrap()
        ));
        let input_arg = prefixed(prefix, &input);
        let output_arg = prefixed(prefix, &output_path);

        let output = Command::new(imx())
            .args(["resize-fit", "5x5", input_arg.as_str(), output_arg.as_str()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{prefix} prefixed resize-fit failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        let expected_identify = expected_identify
            .replace("width=2", "width=5")
            .replace("height=1", "height=3");
        let identify = Command::new(imx())
            .args(["identify", prefixed(prefix, &output_path).as_str()])
            .output()
            .unwrap();
        assert!(
            identify.status.success(),
            "{prefix} resize-fit identify failed with stderr={}",
            String::from_utf8_lossy(&identify.stderr)
        );
        assert_eq!(
            String::from_utf8(identify.stdout).unwrap().trim(),
            expected_identify
        );
    }
}

#[test]
fn resize_fit_uses_fitted_dimensions_then_center_sampled_pixels() {
    let dir = temp_dir("resize_fit_pixels");
    let input = dir.join("input.ppm");
    let output_path = dir.join("output.ppm");
    let image = Image::new(
        3,
        1,
        PixelFormat::Rgb8,
        vec![255, 0, 0, 0, 255, 0, 0, 0, 255],
    )
    .unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([
            "resize-fit",
            "2x2",
            prefixed("PPM", &input).as_str(),
            prefixed("PPM", &output_path).as_str(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "resize-fit failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let resized = imx_codec_pnm::decode_ppm(&fs::read(output_path).unwrap()).unwrap();
    assert_eq!(resized.width(), 2);
    assert_eq!(resized.height(), 1);
    assert_eq!(resized.pixel_format(), PixelFormat::Rgb8);
    assert_eq!(resized.pixels(), &[255, 0, 0, 0, 0, 255]);
}

#[test]
fn malformed_resize_fit_arguments_are_rejected() {
    let dir = temp_dir("resize_fit_malformed_args");
    let input = dir.join("input.ppm");
    let output_path = dir.join("output.ppm");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    for (dimensions, expected_error) in [
        ("", "invalid resize dimensions"),
        ("2", "invalid resize dimensions"),
        ("x2", "invalid resize dimensions"),
        ("2x", "invalid resize dimensions"),
        ("2X2", "invalid resize dimensions"),
        ("0x2", "resize dimensions must be non-zero"),
        ("2x0", "resize dimensions must be non-zero"),
        ("4294967296x2", "invalid resize width"),
        ("2x4294967296", "invalid resize height"),
    ] {
        let output = Command::new(imx())
            .args([
                "resize-fit",
                dimensions,
                input.to_str().unwrap(),
                output_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "malformed resize-fit dimensions unexpectedly succeeded: {dimensions:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "expected stderr to contain {expected_error:?}, got {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output_path.exists());
    }
}

#[test]
fn resize_fit_rejects_same_input_and_output_path() {
    let dir = temp_dir("resize_fit_same_path");
    let input = dir.join("input.ppm");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    let arg = prefixed("PPM", &input);

    let output = Command::new(imx())
        .args(["resize-fit", "2x2", arg.as_str(), arg.as_str()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be different"));
}

#[test]
fn resize_fit_prefix_errors_match_identify_and_transcode_contract() {
    let dir = temp_dir("resize_fit_prefix_errors");
    let ppm = dir.join("input.ppm");
    let png = dir.join("input.png");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    fs::write(&png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output_ppm = dir.join("out.ppm");
    let output_png = dir.join("out.png");
    let extensionless_output = dir.join("out");
    let cases = vec![
        (
            vec![
                "resize-fit".to_string(),
                "2x2".to_string(),
                prefixed("XYZ", &ppm),
                prefixed("PPM", &output_ppm),
            ],
            "unsupported format prefix: XYZ",
        ),
        (
            vec![
                "resize-fit".to_string(),
                "2x2".to_string(),
                "PNG:".to_string(),
                prefixed("PNG", &output_png),
            ],
            "missing path after format prefix PNG:",
        ),
        (
            vec![
                "resize-fit".to_string(),
                "2x2".to_string(),
                prefixed("PNG", &ppm),
                prefixed("PPM", &output_ppm),
            ],
            "format prefix PNG does not match detected format PPM",
        ),
        (
            vec![
                "resize-fit".to_string(),
                "2x2".to_string(),
                prefixed("PNG", &png),
                prefixed("PPM", &output_png),
            ],
            "format prefix PPM does not match path format PNG",
        ),
        (
            vec![
                "resize-fit".to_string(),
                "2x2".to_string(),
                prefixed("PNG", &png),
                prefixed("PNG", &extensionless_output),
            ],
            "unsupported format:",
        ),
        (
            vec![
                "resize-fit".to_string(),
                "2x2".to_string(),
                prefixed("PPM", &ppm),
                prefixed("PPM", &ppm),
            ],
            "input and output paths must be different",
        ),
    ];

    for (args, expected_error) in cases {
        let output = Command::new(imx()).args(&args).output().unwrap();
        assert!(
            !output.status.success(),
            "malformed resize-fit prefix case unexpectedly succeeded: {args:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "expected stderr to contain {expected_error:?}, got {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn batch_convert_writes_multiple_prefixed_inputs_to_requested_format() {
    let dir = temp_dir("batch_convert_many");
    let source_dir = dir.join("source");
    let output_dir = dir.join("out");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    let mut args = vec![
        "batch-convert".to_string(),
        "--to".to_string(),
        "PPM".to_string(),
        "--output-dir".to_string(),
        output_dir.to_string_lossy().into_owned(),
    ];
    let fixtures = write_supported_fixtures(&source_dir);
    let mut stems = Vec::new();
    for (prefix, input, _) in fixtures {
        let stem = prefix.to_ascii_lowercase();
        let renamed = source_dir.join(format!(
            "{stem}.{}",
            input.extension().unwrap().to_str().unwrap()
        ));
        fs::rename(&input, &renamed).unwrap();
        args.push(prefixed(prefix, &renamed));
        stems.push(stem);
    }

    let output = Command::new(imx()).args(&args).output().unwrap();
    assert!(
        output.status.success(),
        "batch-convert failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    for stem in stems {
        let output_path = output_dir.join(format!("{stem}.ppm"));
        assert!(output_path.exists(), "missing {}", output_path.display());
        let identify = Command::new(imx())
            .args(["identify", output_path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            identify.status.success(),
            "identify failed for {} with stderr={}",
            output_path.display(),
            String::from_utf8_lossy(&identify.stderr)
        );
        let stdout = String::from_utf8(identify.stdout).unwrap();
        assert!(
            stdout.starts_with("format=PPM width=2 height=1 channels=RGB depth="),
            "unexpected identify output for {}: {stdout}",
            output_path.display()
        );
    }
}

#[test]
fn batch_convert_supports_each_output_format_from_ppm() {
    let dir = temp_dir("batch_convert_targets");
    let input = dir.join("input.ppm");
    let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    for (format, extension, expected_identify) in [
        (
            "BMP",
            "bmp",
            "format=BMP width=2 height=1 channels=RGB depth=8",
        ),
        (
            "FARBFELD",
            "ff",
            "format=FARBFELD width=2 height=1 channels=RGBA depth=16",
        ),
        (
            "JPEG",
            "jpg",
            "format=JPEG width=2 height=1 channels=RGB depth=8",
        ),
        (
            "QOI",
            "qoi",
            "format=QOI width=2 height=1 channels=RGBA depth=8",
        ),
        (
            "PBM",
            "pbm",
            "format=PBM width=2 height=1 channels=GRAY depth=1",
        ),
        (
            "PGM",
            "pgm",
            "format=PGM width=2 height=1 channels=GRAY depth=8",
        ),
        (
            "PNG",
            "png",
            "format=PNG width=2 height=1 channels=RGB depth=8",
        ),
        (
            "PPM",
            "ppm",
            "format=PPM width=2 height=1 channels=RGB depth=8",
        ),
        (
            "WEBP",
            "webp",
            "format=WEBP width=2 height=1 channels=RGB depth=8",
        ),
    ] {
        let output_dir = dir.join(format!("out-{}", format.to_ascii_lowercase()));
        fs::create_dir_all(&output_dir).unwrap();
        let output = Command::new(imx())
            .args([
                "batch-convert",
                "--to",
                format,
                "--output-dir",
                output_dir.to_str().unwrap(),
                prefixed("PPM", &input).as_str(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "batch-convert to {format} failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        let output_path = output_dir.join(format!("input.{extension}"));
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
fn batch_convert_composes_resize_modes() {
    let dir = temp_dir("batch_convert_resize");
    let input = dir.join("input.ppm");
    let exact_dir = dir.join("exact");
    let fit_dir = dir.join("fit");
    fs::create_dir_all(&exact_dir).unwrap();
    fs::create_dir_all(&fit_dir).unwrap();
    let image = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 0, 255]).unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    let exact = Command::new(imx())
        .args([
            "batch-convert",
            "--to",
            "PPM",
            "--output-dir",
            exact_dir.to_str().unwrap(),
            "--resize",
            "1x1",
            prefixed("PPM", &input).as_str(),
        ])
        .output()
        .unwrap();
    assert!(
        exact.status.success(),
        "batch exact resize failed with stderr={}",
        String::from_utf8_lossy(&exact.stderr)
    );
    let exact_identify = Command::new(imx())
        .args(["identify", exact_dir.join("input.ppm").to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(exact_identify.stdout).unwrap().trim(),
        "format=PPM width=1 height=1 channels=RGB depth=8"
    );

    let fit = Command::new(imx())
        .args([
            "batch-convert",
            "--to",
            "PPM",
            "--output-dir",
            fit_dir.to_str().unwrap(),
            "--resize-fit",
            "5x5",
            prefixed("PPM", &input).as_str(),
        ])
        .output()
        .unwrap();
    assert!(
        fit.status.success(),
        "batch resize-fit failed with stderr={}",
        String::from_utf8_lossy(&fit.stderr)
    );
    let fit_identify = Command::new(imx())
        .args(["identify", fit_dir.join("input.ppm").to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(fit_identify.stdout).unwrap().trim(),
        "format=PPM width=5 height=3 channels=RGB depth=8"
    );
}

#[test]
fn malformed_batch_convert_arguments_are_rejected_without_outputs() {
    let dir = temp_dir("batch_convert_malformed");
    let input = dir.join("input.ppm");
    let output_dir = dir.join("out");
    fs::create_dir_all(&output_dir).unwrap();
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&input, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    let input = input.to_string_lossy().into_owned();
    let output_dir = output_dir.to_string_lossy().into_owned();
    let missing = dir.join("missing.ppm").to_string_lossy().into_owned();
    let file_output_dir = dir.join("not-a-dir");
    fs::write(&file_output_dir, b"not a dir").unwrap();
    let file_output_dir = file_output_dir.to_string_lossy().into_owned();

    let cases = vec![
        (
            vec!["batch-convert", "--output-dir", &output_dir, &input],
            "batch-convert requires --to <FORMAT>",
        ),
        (
            vec!["batch-convert", "--to", "PPM", &input],
            "batch-convert requires --output-dir <dir>",
        ),
        (
            vec!["batch-convert", "--to", "PPM", "--output-dir", &output_dir],
            "batch-convert requires at least one input",
        ),
        (
            vec!["batch-convert", "--to"],
            "batch-convert --to requires a format",
        ),
        (
            vec!["batch-convert", "--output-dir"],
            "batch-convert --output-dir requires a directory",
        ),
        (
            vec![
                "batch-convert",
                "--to",
                "PPM",
                "--to",
                "PNG",
                "--output-dir",
                &output_dir,
                &input,
            ],
            "batch-convert --to may only be supplied once",
        ),
        (
            vec![
                "batch-convert",
                "--to",
                "PPM",
                "--output-dir",
                &output_dir,
                "--output-dir",
                &output_dir,
                &input,
            ],
            "batch-convert --output-dir may only be supplied once",
        ),
        (
            vec![
                "batch-convert",
                "--to",
                "JPG",
                "--output-dir",
                &output_dir,
                &input,
            ],
            "unsupported output format: JPG",
        ),
        (
            vec![
                "batch-convert",
                "--to",
                "PPM",
                "--output-dir",
                &output_dir,
                "--resize",
                "1x1",
                "--resize-fit",
                "1x1",
                &input,
            ],
            "batch-convert accepts only one of --resize or --resize-fit",
        ),
        (
            vec![
                "batch-convert",
                "--to",
                "PPM",
                "--output-dir",
                &output_dir,
                "--resize",
                "x1",
                &input,
            ],
            "invalid resize dimensions",
        ),
        (
            vec![
                "batch-convert",
                "--to",
                "PPM",
                "--output-dir",
                &output_dir,
                "--unknown",
                &input,
            ],
            "unsupported batch-convert option: --unknown",
        ),
        (
            vec![
                "batch-convert",
                "--to",
                "PPM",
                "--output-dir",
                &output_dir,
                &missing,
            ],
            "missing input:",
        ),
        (
            vec![
                "batch-convert",
                "--to",
                "PPM",
                "--output-dir",
                &file_output_dir,
                &input,
            ],
            "output directory is not a directory",
        ),
        (
            vec![
                "batch-convert",
                "--to",
                "PPM",
                "--output-dir",
                &output_dir,
                "-",
            ],
            "stdin/stdout is not supported",
        ),
        (
            vec![
                "batch-convert",
                "--to",
                "PPM",
                "--output-dir",
                &output_dir,
                "*.ppm",
            ],
            "missing input:",
        ),
    ];

    for (args, expected_error) in cases {
        let output = Command::new(imx()).args(args).output().unwrap();
        assert!(
            !output.status.success(),
            "malformed batch-convert unexpectedly succeeded"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "expected stderr to contain {expected_error:?}, got {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read_dir(&output_dir).unwrap().count(), 0);
    }
}

#[test]
fn batch_convert_rejects_collisions_existing_outputs_and_same_paths() {
    let dir = temp_dir("batch_convert_collisions");
    let a_dir = dir.join("a");
    let b_dir = dir.join("b");
    let output_dir = dir.join("out");
    fs::create_dir_all(&a_dir).unwrap();
    fs::create_dir_all(&b_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    let a = a_dir.join("same.ppm");
    let b = b_dir.join("same.ppm");
    fs::write(&a, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    fs::write(&b, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([
            "batch-convert",
            "--to",
            "PPM",
            "--output-dir",
            output_dir.to_str().unwrap(),
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("batch output collision"));
    assert_eq!(fs::read_dir(&output_dir).unwrap().count(), 0);

    let existing = output_dir.join("same.ppm");
    fs::write(&existing, b"existing").unwrap();
    let output = Command::new(imx())
        .args([
            "batch-convert",
            "--to",
            "PPM",
            "--output-dir",
            output_dir.to_str().unwrap(),
            a.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("output path already exists"));
    assert_eq!(fs::read(&existing).unwrap(), b"existing");

    let output = Command::new(imx())
        .args([
            "batch-convert",
            "--to",
            "PPM",
            "--output-dir",
            a_dir.to_str().unwrap(),
            a.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be different"));
}

#[test]
fn batch_convert_prefix_errors_match_existing_contract() {
    let dir = temp_dir("batch_convert_prefix_errors");
    let output_dir = dir.join("out");
    fs::create_dir_all(&output_dir).unwrap();
    let ppm = dir.join("input.ppm");
    let png = dir.join("input.png");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    fs::write(&png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let cases = vec![
        (prefixed("XYZ", &ppm), "unsupported format prefix: XYZ"),
        ("PNG:".to_string(), "missing path after format prefix PNG:"),
        (
            prefixed("PNG", &ppm),
            "format prefix PNG does not match detected format PPM",
        ),
        (
            prefixed("PPM", &png),
            "format prefix PPM does not match detected format PNG",
        ),
    ];

    for (input_arg, expected_error) in cases {
        let output = Command::new(imx())
            .args([
                "batch-convert",
                "--to",
                "PPM",
                "--output-dir",
                output_dir.to_str().unwrap(),
                input_arg.as_str(),
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "expected stderr to contain {expected_error:?}, got {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn batch_convert_encode_failure_leaves_no_target_file() {
    let dir = temp_dir("batch_convert_encode_failure");
    let output_dir = dir.join("out");
    fs::create_dir_all(&output_dir).unwrap();
    let input = dir.join("transparent.png");
    let image = Image::new(1, 1, PixelFormat::Rgba8, vec![255, 0, 0, 0]).unwrap();
    fs::write(&input, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([
            "batch-convert",
            "--to",
            "JPEG",
            "--output-dir",
            output_dir.to_str().unwrap(),
            prefixed("PNG", &input).as_str(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("alpha"),
        "expected JPEG alpha error, got {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output_dir.join("transparent.jpg").exists());
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to encode JPEG output"));
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
        let output = Command::new(imx())
            .args(["identify", "--json", arg.as_str()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "PPM16 JSON identify failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "{\"schema_version\":1,\"format\":\"PPM\",\"width\":2,\"height\":1,\"channels\":\"RGB\",\"depth\":16}"
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
    let bmp = dir.join("input.bmp");
    let jpeg = dir.join("input.jpg");
    let qoi = dir.join("input.qoi");
    let pbm = dir.join("input.pbm");
    let pgm = dir.join("input.pgm");
    let png = dir.join("input.png");
    let ppm = dir.join("input.ppm");

    fs::write(&ff, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();
    fs::write(&bmp, imx_codec_bmp::encode(&image).unwrap()).unwrap();
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
            "bmp",
            bmp.as_path(),
            "output.bmp",
            "format=BMP width=2 height=1 channels=RGBA depth=8",
        ),
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
            assert!(stdout.contains("imx identify --json"));
            assert!(stdout.contains("imx report --json"));
            assert!(stdout.contains("supported identify JSON"));
            assert!(stdout.contains("stable diagnostic_code"));
            assert!(stdout.contains("imx resize <width>x<height>"));
            assert!(stdout.contains("imx resize-fit <width>x<height>"));
            assert!(stdout.contains("imx batch-convert --to <FORMAT> --output-dir <dir>"));
            assert!(stdout.contains("imx compare [--metric <ae|mae|psnr>]"));
            assert!(stdout.contains("supported compare:"));
            assert!(stdout.contains("imx self-test"));
            assert!(stdout.contains("imx completions <bash|zsh|fish>"));
            assert!(stdout.contains("man/imx.1"));
            assert!(stdout.contains("offline install confidence check"));
            assert!(stdout.contains("nearest-neighbor exact dimensions (<width>x<height>)"));
            assert!(stdout.contains("<width>x or x<height>"));
            assert!(stdout.contains("uniform percent (<percent>%)"));
            assert!(stdout.contains("no overwrite or collision renaming"));
            assert!(stdout.contains(".bmp"));
            assert!(stdout.contains(".farbfeld"));
            assert!(stdout.contains("BMP:"));
            assert!(stdout.contains(".jpg"));
            assert!(stdout.contains(".jpeg"));
            assert!(stdout.contains("JPEG:"));
            assert!(stdout.contains(".png"));
            assert!(stdout.contains("PNG:"));
            assert!(stdout.contains(".tif"));
            assert!(stdout.contains(".tiff"));
            assert!(stdout.contains("TIFF:"));
        }
    }
}

fn write_png(path: &Path, image: &Image) {
    fs::write(path, imx_codec_png::encode(image).unwrap()).unwrap();
}

fn rgb_image(width: u32, height: u32, fill: [u8; 3]) -> Image {
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for _ in 0..(width * height) {
        pixels.extend_from_slice(&fill);
    }
    Image::new(width, height, PixelFormat::Rgb8, pixels).unwrap()
}

#[test]
fn compare_identical_images_prints_identical_and_exits_zero() {
    let dir = temp_dir("compare_identical");
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    let image = rgb_image(4, 4, [10, 20, 30]);
    write_png(&a, &image);
    write_png(&b, &image);

    let output = Command::new(imx())
        .args(["compare", a.to_str().unwrap(), b.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "identical"
    );
}

#[test]
fn compare_one_pixel_difference_reports_stats_and_exits_one() {
    let dir = temp_dir("compare_one_pixel");
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    let base = rgb_image(2, 2, [0, 0, 0]);
    write_png(&a, &base);
    // Flip a single channel of one pixel.
    let mut pixels = base.pixels().to_vec();
    pixels[0] = 200;
    let changed = Image::new(2, 2, PixelFormat::Rgb8, pixels).unwrap();
    write_png(&b, &changed);

    let output = Command::new(imx())
        .args(["compare", a.to_str().unwrap(), b.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("differ: 1/4 pixels"), "got {stdout:?}");
    assert!(stdout.contains("ae=200"), "got {stdout:?}");
    assert!(stdout.contains("mae="), "got {stdout:?}");
}

#[test]
fn compare_dimension_mismatch_reports_differ_without_stats() {
    let dir = temp_dir("compare_dim_mismatch");
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    write_png(&a, &rgb_image(4, 4, [1, 2, 3]));
    write_png(&b, &rgb_image(2, 2, [1, 2, 3]));

    let output = Command::new(imx())
        .args(["compare", a.to_str().unwrap(), b.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "differ: dimensions 4x4 vs 2x2");
    assert!(!stdout.contains("pixels"), "got {stdout:?}");
}

#[test]
fn compare_metric_mae_on_identical_prints_zero() {
    let dir = temp_dir("compare_metric_mae");
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    let image = rgb_image(3, 3, [7, 7, 7]);
    write_png(&a, &image);
    write_png(&b, &image);

    let output = Command::new(imx())
        .args([
            "compare",
            "--metric",
            "mae",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "0.000000");
}

#[test]
fn compare_metric_psnr_on_identical_prints_inf() {
    let dir = temp_dir("compare_metric_psnr");
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    let image = rgb_image(3, 3, [9, 9, 9]);
    write_png(&a, &image);
    write_png(&b, &image);

    let output = Command::new(imx())
        .args([
            "compare",
            "--metric",
            "psnr",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "inf");
}

#[test]
fn compare_is_deterministic_across_runs() {
    let dir = temp_dir("compare_determinism");
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    write_png(&a, &rgb_image(4, 4, [0, 0, 0]));
    let mut pixels = rgb_image(4, 4, [0, 0, 0]).pixels().to_vec();
    pixels[5] = 17;
    write_png(&b, &Image::new(4, 4, PixelFormat::Rgb8, pixels).unwrap());

    let run = || {
        Command::new(imx())
            .args(["compare", a.to_str().unwrap(), b.to_str().unwrap()])
            .output()
            .unwrap()
    };
    let first = run();
    let second = run();
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.status.code(), second.status.code());
    assert_eq!(first.status.code(), Some(1));
}

#[test]
fn compare_missing_operand_is_usage_error() {
    let dir = temp_dir("compare_missing_operand");
    let a = dir.join("a.png");
    write_png(&a, &rgb_image(2, 2, [0, 0, 0]));
    let output = Command::new(imx())
        .args(["compare", a.to_str().unwrap()])
        .output()
        .unwrap();
    assert_failure(output, 2, "usage:");
}

#[test]
fn compare_unknown_metric_is_usage_error() {
    let dir = temp_dir("compare_unknown_metric");
    let a = dir.join("a.png");
    let b = dir.join("b.png");
    write_png(&a, &rgb_image(2, 2, [0, 0, 0]));
    write_png(&b, &rgb_image(2, 2, [0, 0, 0]));
    let output = Command::new(imx())
        .args([
            "compare",
            "--metric",
            "bogus",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_failure(output, 2, "invalid --metric value");
}

#[test]
fn compare_both_stdin_is_usage_error() {
    let output = Command::new(imx())
        .args(["compare", "PNG:-", "PNG:-"])
        .output()
        .unwrap();
    assert_failure(output, 2, "at most one compare operand");
}

#[test]
fn completions_bash_emits_script() {
    let output = Command::new(imx())
        .args(["completions", "bash"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("complete -F _imx imx"));
    assert!(stdout.contains("identify"));
    assert!(stdout.contains("resize"));
}

#[test]
fn completions_zsh_emits_compdef_marker() {
    let output = Command::new(imx())
        .args(["completions", "zsh"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("#compdef imx"));
}

#[test]
fn completions_fish_emits_complete_directives() {
    let output = Command::new(imx())
        .args(["completions", "fish"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("complete -c imx"));
}

#[test]
fn completions_unknown_shell_is_usage_error() {
    let output = Command::new(imx())
        .args(["completions", "powershell"])
        .output()
        .unwrap();
    assert_failure(output, 2, "unsupported shell: powershell");
}

#[test]
fn completions_missing_shell_is_usage_error() {
    let output = Command::new(imx()).arg("completions").output().unwrap();
    assert_failure(output, 2, "usage:");
}

#[test]
fn completions_output_is_deterministic() {
    for shell in ["bash", "zsh", "fish"] {
        let first = Command::new(imx())
            .args(["completions", shell])
            .output()
            .unwrap();
        let second = Command::new(imx())
            .args(["completions", shell])
            .output()
            .unwrap();
        assert!(first.status.success());
        assert!(second.status.success());
        assert_eq!(first.stdout, second.stdout);
    }
}

#[test]
fn self_test_command_exercises_installed_surface() {
    let output = Command::new(imx()).arg("self-test").output().unwrap();
    assert!(
        output.status.success(),
        "self-test failed with stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "self-test: identify ok",
        "self-test: transcode ok",
        "self-test: resize ok",
        "self-test: resize-fit ok",
        "self-test: batch-convert ok",
        "self-test: passed",
    ] {
        assert!(
            stdout.contains(expected),
            "missing {expected:?} in {stdout:?}"
        );
    }
}

#[test]
fn self_test_rejects_extra_arguments_as_usage() {
    let output = Command::new(imx())
        .args(["self-test", "extra"])
        .output()
        .unwrap();
    assert_failure(output, 2, "usage:");
}

#[test]
fn unsupported_imagemagick_command_shapes_are_rejected() {
    let dir = temp_dir("unsupported_command_shapes");
    let input = dir.join("input.ppm");
    let output_path = dir.join("output.qoi");
    fs::write(
        &input,
        imx_codec_pnm::encode_ppm(&Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap())
            .unwrap(),
    )
    .unwrap();

    for args in [
        vec!["convert".to_string(), input.to_string_lossy().into_owned()],
        vec![
            "convert".to_string(),
            input.to_string_lossy().into_owned(),
            output_path.to_string_lossy().into_owned(),
        ],
        vec![
            input.to_string_lossy().into_owned(),
            "-resize".to_string(),
            output_path.to_string_lossy().into_owned(),
        ],
        vec!["identify".to_string(), "-".to_string()],
        vec![input.to_string_lossy().into_owned(), "-".to_string()],
    ] {
        let output = Command::new(imx()).args(&args).output().unwrap();
        assert!(
            !output.status.success(),
            "unsupported command shape unexpectedly succeeded: {args:?}"
        );
    }
}

#[test]
fn json_command_shapes_are_exact() {
    let dir = temp_dir("json_command_shapes");
    let input = dir.join("input.ppm");
    fs::write(
        &input,
        imx_codec_pnm::encode_ppm(&Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap())
            .unwrap(),
    )
    .unwrap();

    for args in [
        vec!["identify".to_string(), "--json".to_string()],
        vec![
            "identify".to_string(),
            input.to_string_lossy().into_owned(),
            "--json".to_string(),
        ],
        vec![
            "identify".to_string(),
            "--format".to_string(),
            "json".to_string(),
            input.to_string_lossy().into_owned(),
        ],
        vec!["report".to_string(), "--json".to_string()],
        vec!["report".to_string(), input.to_string_lossy().into_owned()],
        vec![
            "report".to_string(),
            "--format".to_string(),
            "json".to_string(),
            input.to_string_lossy().into_owned(),
        ],
    ] {
        let output = Command::new(imx()).args(&args).output().unwrap();
        assert_failure(output, 2, "usage:");
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to identify QOI input"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("QOI channels must be 3 or 4, got 2"));
}

#[test]
fn identify_json_errors_are_machine_readable() {
    let dir = temp_dir("identify_json_error");
    let bad = dir.join("bad.qoi");
    fs::write(&bad, b"qoif\0\0\0\x01\0\0\0\x01\x02\0").unwrap();

    let output = Command::new(imx())
        .args(["identify", "--json", prefixed("QOI", &bad).as_str()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim(),
        report_unsupported_json(
            "qoi.invalid_channels",
            "failed to identify QOI input: QOI channels must be 3 or 4, got 2",
        )
    );
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to identify PNG input"));
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to identify JPEG input"));
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to decode PPM input"));
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
            vec!["identify".to_string(), prefixed("XYZ", &ppm)],
            "unsupported format prefix: XYZ",
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
fn report_json_returns_stable_diagnostic_codes() {
    let dir = temp_dir("report_json_diagnostics");
    let ppm = dir.join("input.ppm");
    let qoi = dir.join("bad.qoi");
    let bad_max_ppm = dir.join("bad-max.ppm");
    let unknown = dir.join("unknown.dat");
    let bad_bmp = dir.join("bad-compression.bmp");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    fs::write(&qoi, b"qoif\0\0\0\x01\0\0\0\x01\x02\0").unwrap();
    fs::write(&bad_max_ppm, b"P3\n1 1\n65536\n0 0 0\n").unwrap();
    fs::write(&unknown, b"not an image\n").unwrap();
    let mut bad_bmp_bytes = imx_codec_bmp::encode(&image).unwrap();
    bad_bmp_bytes[30..34].copy_from_slice(&1_u32.to_le_bytes());
    fs::write(&bad_bmp, bad_bmp_bytes).unwrap();
    let missing = dir.join("missing.ppm");

    let cases = [
        (
            vec![
                "report".to_string(),
                "--json".to_string(),
                prefixed("XYZ", &ppm),
            ],
            report_unsupported_json(
                "input.unsupported_format_prefix",
                "unsupported format prefix: XYZ",
            ),
        ),
        (
            vec![
                "report".to_string(),
                "--json".to_string(),
                "PPM:".to_string(),
            ],
            report_unsupported_json(
                "input.missing_prefix_path",
                "missing path after format prefix PPM:",
            ),
        ),
        (
            vec![
                "report".to_string(),
                "--json".to_string(),
                prefixed("PNG", &ppm),
            ],
            report_unsupported_json(
                "input.format_prefix_mismatch",
                "format prefix PNG does not match detected format PPM",
            ),
        ),
        (
            vec![
                "report".to_string(),
                "--json".to_string(),
                missing.to_string_lossy().into_owned(),
            ],
            report_unsupported_json(
                "input.missing",
                &format!("missing input: {}", missing.to_string_lossy()),
            ),
        ),
        (
            vec![
                "report".to_string(),
                "--json".to_string(),
                unknown.to_string_lossy().into_owned(),
            ],
            report_unsupported_json(
                "input.unsupported_format",
                &format!("unsupported format: {}", unknown.to_string_lossy()),
            ),
        ),
        (
            vec![
                "report".to_string(),
                "--json".to_string(),
                prefixed("QOI", &qoi),
            ],
            report_unsupported_json(
                "qoi.invalid_channels",
                "failed to identify QOI input: QOI channels must be 3 or 4, got 2",
            ),
        ),
        (
            vec![
                "report".to_string(),
                "--json".to_string(),
                prefixed("PPM", &bad_max_ppm),
            ],
            report_unsupported_json(
                "pnm.invalid_max_value",
                "failed to identify PPM input: PPM max value must be 1..=65535, got 65536",
            ),
        ),
        (
            vec![
                "report".to_string(),
                "--json".to_string(),
                prefixed("BMP", &bad_bmp),
            ],
            report_unsupported_json(
                "bmp.unsupported_feature",
                "failed to identify BMP input: unsupported format: BMP compression is not supported",
            ),
        ),
    ];

    for (args, expected_json) in cases {
        let output = Command::new(imx()).args(&args).output().unwrap();
        assert!(
            output.status.success(),
            "report failed for {args:?} with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            expected_json
        );
    }
}

#[test]
fn diagnostic_failures_have_stable_exit_codes_and_context() {
    let dir = temp_dir("diagnostic_exit_codes");
    let output_dir = dir.join("out");
    fs::create_dir_all(&output_dir).unwrap();
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    let ppm = dir.join("input.ppm");
    let png = dir.join("input.png");
    let bmp = dir.join("compressed.bmp");
    fs::write(&ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    fs::write(&png, imx_codec_png::encode(&image).unwrap()).unwrap();
    let mut compressed_bmp = imx_codec_bmp::encode(&image).unwrap();
    compressed_bmp[30..34].copy_from_slice(&1_u32.to_le_bytes());
    fs::write(&bmp, compressed_bmp).unwrap();
    let missing = dir.join("missing.ppm");
    let resized = dir.join("resized.ppm");
    let missing_output_dir = dir.join("missing-output-dir");

    let cases = vec![
        (
            vec!["identify".to_string(), prefixed("XYZ", &ppm)],
            1,
            "unsupported format prefix: XYZ",
        ),
        (
            vec!["identify".to_string(), prefixed("PNG", &ppm)],
            1,
            "format prefix PNG does not match detected format PPM",
        ),
        (
            vec![
                "identify".to_string(),
                missing.to_string_lossy().into_owned(),
            ],
            1,
            "missing input:",
        ),
        (
            vec!["identify".to_string(), prefixed("BMP", &bmp)],
            1,
            "failed to identify BMP input",
        ),
        (
            vec![
                "resize".to_string(),
                "0x2".to_string(),
                prefixed("PPM", &ppm),
                prefixed("PPM", &resized),
            ],
            2,
            "invalid resize geometry",
        ),
        (
            vec![prefixed("PPM", &ppm), prefixed("PPM", &ppm)],
            1,
            "input and output paths must be different",
        ),
        (
            vec![
                "batch-convert".to_string(),
                "--to".to_string(),
                "PPM".to_string(),
                "--output-dir".to_string(),
                missing_output_dir.to_string_lossy().into_owned(),
                prefixed("PPM", &ppm),
            ],
            1,
            "missing output directory:",
        ),
        (
            vec!["self-test".to_string(), "extra".to_string()],
            2,
            "usage:",
        ),
        (
            vec!["convert".to_string(), ppm.to_string_lossy().into_owned()],
            2,
            "usage:",
        ),
    ];

    for (args, expected_code, expected_error) in cases {
        let output = Command::new(imx()).args(&args).output().unwrap();
        assert_failure(output, expected_code, expected_error);
    }

    let output = Command::new(imx())
        .args(["identify", prefixed("BMP", &bmp).as_str()])
        .output()
        .unwrap();
    assert_failure(output, 1, "BMP compression is not supported");

    assert_eq!(fs::read_dir(&output_dir).unwrap().count(), 0);
}

#[test]
fn all_supported_prefixes_reject_mismatched_inputs_and_outputs() {
    let dir = temp_dir("prefix_mismatch_matrix");
    let fixtures = write_supported_fixtures(&dir);
    let output_extensions = [
        ("BMP", "bmp"),
        ("FARBFELD", "ff"),
        ("JPEG", "jpg"),
        ("QOI", "qoi"),
        ("PBM", "pbm"),
        ("PGM", "pgm"),
        ("PNG", "png"),
        ("PPM", "ppm"),
    ];

    for (prefix, _, _) in &fixtures {
        for (actual_prefix, input, _) in &fixtures {
            if prefix == actual_prefix {
                continue;
            }
            let output = Command::new(imx())
                .args(["identify", prefixed(prefix, input).as_str()])
                .output()
                .unwrap();
            assert!(
                !output.status.success(),
                "{prefix} unexpectedly accepted {actual_prefix} input"
            );
            let expected =
                format!("format prefix {prefix} does not match detected format {actual_prefix}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(&expected),
                "expected stderr to contain {expected:?}, got {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    let input = prefixed(fixtures[0].0, &fixtures[0].1);
    for (prefix, _) in &output_extensions {
        for (actual_prefix, extension) in &output_extensions {
            if prefix == actual_prefix {
                continue;
            }
            let output_path = dir.join(format!(
                "{}-as-{}.{}",
                prefix.to_ascii_lowercase(),
                actual_prefix.to_ascii_lowercase(),
                extension
            ));
            let output_arg = prefixed(prefix, &output_path);
            let output = Command::new(imx())
                .args([input.as_str(), output_arg.as_str()])
                .output()
                .unwrap();
            assert!(
                !output.status.success(),
                "{prefix} unexpectedly accepted {actual_prefix} output path"
            );
            let expected =
                format!("format prefix {prefix} does not match path format {actual_prefix}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(&expected),
                "expected stderr to contain {expected:?}, got {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[test]
fn output_prefixes_do_not_select_extensionless_outputs() {
    let dir = temp_dir("extensionless_prefix_outputs");
    for (prefix, input, _) in write_supported_fixtures(&dir) {
        let output_path = dir.join(format!("extensionless-{}", prefix.to_ascii_lowercase()));
        let output = Command::new(imx())
            .args([prefixed(prefix, &input), prefixed(prefix, &output_path)])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "{prefix} unexpectedly selected an extensionless output"
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported format:"));
    }
}

#[test]
fn lowercase_mixed_case_and_alias_prefixes_do_not_expand_prefix_surface() {
    let dir = temp_dir("prefix_aliases");
    let input = dir.join("input.jpg");
    let image = Image::new(8, 8, PixelFormat::Rgb8, vec![0x80; 8 * 8 * 3]).unwrap();
    fs::write(&input, imx_codec_jpeg::encode(&image).unwrap()).unwrap();

    for alias in ["BM", "JPG", "FF", "TGA"] {
        let output = Command::new(imx())
            .args(["identify", prefixed(alias, &input).as_str()])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains(&format!("unsupported format prefix: {alias}")));
    }

    for alias in ["bmp", "Bmp", "jpeg", "Jpeg", "jpg", "ff"] {
        let arg = format!("{alias}:{}", input.to_string_lossy());
        let output = Command::new(imx())
            .args(["identify", arg.as_str()])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("missing input:") && stderr.contains(&arg),
            "{alias}: should remain an ordinary path segment, got stderr={}",
            stderr
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

fn run_with_stdin(args: &[&str], stdin_bytes: &[u8]) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new(imx())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin_bytes).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn streams_png_stdin_to_ppm_stdout() {
    let dir = temp_dir("stream_roundtrip");
    let png_path = dir.join("input.png");
    png_fixture(
        &png_path,
        2,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[255, 0, 0, 0, 0, 255],
    );
    let png_bytes = fs::read(&png_path).unwrap();

    let output = run_with_stdin(&["PNG:-", "PPM:-"], &png_bytes);
    assert!(
        output.status.success(),
        "stream transcode failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "expected empty stderr, got {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    let out_ppm = dir.join("out.ppm");
    fs::write(&out_ppm, &output.stdout).unwrap();
    let identify = Command::new(imx())
        .args(["identify", out_ppm.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(identify.status.success());
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=PPM width=2 height=1 channels=RGB depth=8"
    );
}

#[test]
fn identifies_from_stdin() {
    let dir = temp_dir("stream_identify");
    let png_path = dir.join("input.png");
    png_fixture(
        &png_path,
        2,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[255, 0, 0, 0, 0, 255],
    );
    let png_bytes = fs::read(&png_path).unwrap();

    let output = run_with_stdin(&["identify", "PNG:-"], &png_bytes);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "format=PNG width=2 height=1 channels=RGB depth=8"
    );

    let json = run_with_stdin(&["identify", "--json", "PNG:-"], &png_bytes);
    assert!(json.status.success());
    assert_eq!(
        String::from_utf8(json.stdout).unwrap().trim(),
        "{\"schema_version\":1,\"format\":\"PNG\",\"width\":2,\"height\":1,\"channels\":\"RGB\",\"depth\":8}"
    );

    let report = run_with_stdin(&["report", "--json", "PNG:-"], &png_bytes);
    assert!(report.status.success());
    assert_eq!(
        String::from_utf8(report.stdout).unwrap().trim(),
        "{\"schema_version\":1,\"status\":\"supported\",\"diagnostic_code\":null,\"format\":\"PNG\",\"width\":2,\"height\":1,\"channels\":\"RGB\",\"depth\":8}"
    );
}

#[test]
fn resize_streams_stdin_to_stdout() {
    let dir = temp_dir("stream_resize");
    let png_path = dir.join("input.png");
    png_fixture(
        &png_path,
        2,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[255, 0, 0, 0, 0, 255],
    );
    let png_bytes = fs::read(&png_path).unwrap();

    let output = run_with_stdin(&["resize", "3x2", "PNG:-", "PPM:-"], &png_bytes);
    assert!(
        output.status.success(),
        "stream resize failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out_ppm = dir.join("out.ppm");
    fs::write(&out_ppm, &output.stdout).unwrap();
    let identify = Command::new(imx())
        .args(["identify", out_ppm.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=PPM width=3 height=2 channels=RGB depth=8"
    );
}

#[test]
fn stdout_output_without_prefix_is_rejected() {
    let dir = temp_dir("stream_no_prefix");
    let png_path = dir.join("input.png");
    png_fixture(
        &png_path,
        2,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[255, 0, 0, 0, 0, 255],
    );
    let png_bytes = fs::read(&png_path).unwrap();

    let output = run_with_stdin(&["PNG:-", "-"], &png_bytes);
    assert_failure(output, 1, "stdout output (-) requires a format prefix");
}

#[test]
fn quality_flag_changes_jpeg_output_size() {
    let dir = temp_dir("quality_jpeg");
    let png_path = dir.join("input.png");
    let pixels: Vec<u8> = (0..16)
        .flat_map(|y: u32| {
            (0..16).flat_map(move |x: u32| {
                [
                    (x.wrapping_mul(13).wrapping_add(y.wrapping_mul(7)) & 0xff) as u8,
                    (x.wrapping_mul(3).wrapping_add(y.wrapping_mul(19)) & 0xff) as u8,
                    (x.wrapping_mul(23).wrapping_add(y.wrapping_mul(5)) & 0xff) as u8,
                ]
            })
        })
        .collect();
    png_fixture(
        &png_path,
        16,
        16,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &pixels,
    );

    let low = dir.join("q20.jpg");
    let high = dir.join("q95.jpg");
    let default = dir.join("default.jpg");

    for (quality, out) in [("20", &low), ("95", &high)] {
        let output = Command::new(imx())
            .args([
                "--quality",
                quality,
                png_path.to_str().unwrap(),
                &prefixed("JPEG", out),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "--quality {quality} failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let identify = Command::new(imx())
            .args(["identify", out.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(identify.status.success());
        assert_eq!(
            String::from_utf8(identify.stdout).unwrap().trim(),
            "format=JPEG width=16 height=16 channels=RGB depth=8"
        );
    }

    let default_output = Command::new(imx())
        .args([png_path.to_str().unwrap(), &prefixed("JPEG", &default)])
        .output()
        .unwrap();
    assert!(default_output.status.success());

    let low_len = fs::read(&low).unwrap().len();
    let high_len = fs::read(&high).unwrap().len();
    let default_len = fs::read(&default).unwrap().len();
    assert!(
        low_len < high_len,
        "expected q20 ({low_len}) smaller than q95 ({high_len})"
    );
    assert_ne!(default_len, low_len);
    assert_ne!(default_len, high_len);
}

#[test]
fn quality_flag_rejected_for_non_jpeg_output() {
    let dir = temp_dir("quality_non_jpeg");
    let png_path = dir.join("input.png");
    png_fixture(
        &png_path,
        2,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[255, 0, 0, 0, 0, 255],
    );
    let out = dir.join("out.png");
    let output = Command::new(imx())
        .args([
            "--quality",
            "50",
            png_path.to_str().unwrap(),
            &prefixed("PNG", &out),
        ])
        .output()
        .unwrap();
    assert_failure(output, 1, "--quality only applies to JPEG output");
}

#[test]
fn quality_flag_rejects_out_of_range_value() {
    let dir = temp_dir("quality_range");
    let png_path = dir.join("input.png");
    png_fixture(
        &png_path,
        2,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[255, 0, 0, 0, 0, 255],
    );
    let out = dir.join("out.jpg");
    for value in ["0", "101", "abc"] {
        let output = Command::new(imx())
            .args([
                "--quality",
                value,
                png_path.to_str().unwrap(),
                &prefixed("JPEG", &out),
            ])
            .output()
            .unwrap();
        assert_failure(output, 1, "invalid --quality value");
    }
}

#[test]
fn batch_convert_quality_changes_jpeg_output_size() {
    let dir = temp_dir("batch_quality_jpeg");
    let png_path = dir.join("input.png");
    let pixels: Vec<u8> = (0..16)
        .flat_map(|y: u32| {
            (0..16).flat_map(move |x: u32| {
                [
                    (x.wrapping_mul(13).wrapping_add(y.wrapping_mul(7)) & 0xff) as u8,
                    (x.wrapping_mul(3).wrapping_add(y.wrapping_mul(19)) & 0xff) as u8,
                    (x.wrapping_mul(23).wrapping_add(y.wrapping_mul(5)) & 0xff) as u8,
                ]
            })
        })
        .collect();
    png_fixture(
        &png_path,
        16,
        16,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &pixels,
    );

    let low_dir = dir.join("low");
    let high_dir = dir.join("high");
    fs::create_dir_all(&low_dir).unwrap();
    fs::create_dir_all(&high_dir).unwrap();

    for (quality, out_dir) in [("40", &low_dir), ("95", &high_dir)] {
        let output = Command::new(imx())
            .args([
                "batch-convert",
                "--to",
                "JPEG",
                "--output-dir",
                out_dir.to_str().unwrap(),
                "--quality",
                quality,
                png_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "batch --quality {quality} failed with stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let low_len = fs::read(low_dir.join("input.jpg")).unwrap().len();
    let high_len = fs::read(high_dir.join("input.jpg")).unwrap().len();
    assert!(
        low_len < high_len,
        "expected q40 ({low_len}) smaller than q95 ({high_len})"
    );
}

#[test]
fn batch_convert_quality_rejected_for_non_jpeg_output() {
    let dir = temp_dir("batch_quality_non_jpeg");
    let png = dir.join("input.png");
    let out_dir = dir.join("out");
    fs::create_dir_all(&out_dir).unwrap();
    png_fixture(
        &png,
        2,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[255, 0, 0, 0, 0, 255],
    );
    let output = Command::new(imx())
        .args([
            "batch-convert",
            "--to",
            "PNG",
            "--output-dir",
            out_dir.to_str().unwrap(),
            "--quality",
            "50",
            png.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_failure(output, 1, "--quality only applies to JPEG output");
}

#[test]
fn batch_convert_quality_rejects_out_of_range_value() {
    let dir = temp_dir("batch_quality_range");
    let png = dir.join("input.png");
    let out_dir = dir.join("out");
    fs::create_dir_all(&out_dir).unwrap();
    png_fixture(
        &png,
        2,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &[255, 0, 0, 0, 0, 255],
    );
    for value in ["0", "101", "abc"] {
        let output = Command::new(imx())
            .args([
                "batch-convert",
                "--to",
                "JPEG",
                "--output-dir",
                out_dir.to_str().unwrap(),
                "--quality",
                value,
                png.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_failure(output, 1, "invalid --quality value");
    }
}

fn write_webp_fixture(
    path: &Path,
    width: u32,
    height: u32,
    color: image_webp::ColorType,
    pixels: &[u8],
) {
    let mut out = Vec::new();
    image_webp::WebPEncoder::new(std::io::Cursor::new(&mut out))
        .encode(pixels, width, height, color)
        .unwrap();
    fs::write(path, out).unwrap();
}

fn write_gif_fixture(path: &Path, width: u16, height: u16, rgba: &[u8]) {
    let mut out = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut out, width, height, &[]).unwrap();
        let mut pixels = rgba.to_vec();
        let frame = gif::Frame::from_rgba_speed(width, height, &mut pixels, 10);
        encoder.write_frame(&frame).unwrap();
    }
    fs::write(path, out).unwrap();
}

#[test]
fn webp_identify_and_transcode_to_png_are_supported() {
    let dir = temp_dir("webp_input");
    let webp = dir.join("input.webp");
    let png = dir.join("output.png");
    write_webp_fixture(
        &webp,
        2,
        1,
        image_webp::ColorType::Rgb8,
        &[255, 0, 0, 0, 255, 0],
    );

    let identify = Command::new(imx())
        .args(["identify", &prefixed("WEBP", &webp)])
        .output()
        .unwrap();
    assert!(
        identify.status.success(),
        "identify failed with stderr={}",
        String::from_utf8_lossy(&identify.stderr)
    );
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=WEBP width=2 height=1 channels=RGB depth=8"
    );

    let unprefixed = Command::new(imx())
        .args(["identify", webp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(unprefixed.status.success());
    assert_eq!(
        String::from_utf8(unprefixed.stdout).unwrap().trim(),
        "format=WEBP width=2 height=1 channels=RGB depth=8"
    );

    let transcode = Command::new(imx())
        .args([webp.to_str().unwrap(), png.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        transcode.status.success(),
        "transcode failed with stderr={}",
        String::from_utf8_lossy(&transcode.stderr)
    );
    let identify_png = Command::new(imx())
        .args(["identify", png.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(identify_png.status.success());
    assert_eq!(
        String::from_utf8(identify_png.stdout).unwrap().trim(),
        "format=PNG width=2 height=1 channels=RGB depth=8"
    );
}

#[test]
fn gif_identify_and_transcode_to_png_are_supported() {
    let dir = temp_dir("gif_input");
    let gif = dir.join("input.gif");
    let png = dir.join("output.png");
    write_gif_fixture(&gif, 2, 1, &[255, 0, 0, 255, 0, 255, 0, 255]);

    let identify = Command::new(imx())
        .args(["identify", &prefixed("GIF", &gif)])
        .output()
        .unwrap();
    assert!(
        identify.status.success(),
        "identify failed with stderr={}",
        String::from_utf8_lossy(&identify.stderr)
    );
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=GIF width=2 height=1 channels=RGBA depth=8"
    );

    let transcode = Command::new(imx())
        .args([gif.to_str().unwrap(), png.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        transcode.status.success(),
        "transcode failed with stderr={}",
        String::from_utf8_lossy(&transcode.stderr)
    );
    let identify_png = Command::new(imx())
        .args(["identify", png.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(identify_png.status.success());
    assert_eq!(
        String::from_utf8(identify_png.stdout).unwrap().trim(),
        "format=PNG width=2 height=1 channels=RGBA depth=8"
    );
}

#[test]
fn png_transcodes_to_webp_and_round_trips_to_ppm() {
    let dir = temp_dir("webp_output");
    let png = dir.join("input.png");
    let webp = dir.join("output.webp");
    let ppm = dir.join("round.ppm");
    let image = Image::new(
        2,
        2,
        PixelFormat::Rgb8,
        vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 9, 8, 7],
    )
    .unwrap();
    fs::write(&png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let transcode = Command::new(imx())
        .args([png.to_str().unwrap(), webp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        transcode.status.success(),
        "PNG->WEBP failed with stderr={}",
        String::from_utf8_lossy(&transcode.stderr)
    );

    let identify = Command::new(imx())
        .args(["identify", webp.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(identify.status.success());
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=WEBP width=2 height=2 channels=RGB depth=8"
    );

    let round = Command::new(imx())
        .args([webp.to_str().unwrap(), ppm.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        round.status.success(),
        "WEBP->PPM failed with stderr={}",
        String::from_utf8_lossy(&round.stderr)
    );
    let decoded = imx_codec_pnm::decode_ppm(&fs::read(&ppm).unwrap()).unwrap();
    assert_eq!(decoded, image);
}

#[test]
fn webp_output_is_deterministic() {
    let dir = temp_dir("webp_output_deterministic");
    let png = dir.join("input.png");
    let first = dir.join("first.webp");
    let second = dir.join("second.webp");
    let image = Image::new(2, 1, PixelFormat::Rgb8, vec![10, 20, 30, 40, 50, 60]).unwrap();
    fs::write(&png, imx_codec_png::encode(&image).unwrap()).unwrap();

    for out in [&first, &second] {
        let status = Command::new(imx())
            .args([png.to_str().unwrap(), out.to_str().unwrap()])
            .status()
            .unwrap();
        assert!(status.success());
    }
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
}

#[test]
fn batch_convert_supports_webp_output() {
    let dir = temp_dir("webp_batch_output");
    let png = dir.join("input.png");
    let out_dir = dir.join("out");
    fs::create_dir_all(&out_dir).unwrap();
    let image = Image::new(2, 1, PixelFormat::Rgb8, vec![1, 2, 3, 4, 5, 6]).unwrap();
    fs::write(&png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([
            "batch-convert",
            "--to",
            "WEBP",
            "--output-dir",
            out_dir.to_str().unwrap(),
            png.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "batch WEBP failed with stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let identify = Command::new(imx())
        .args(["identify", out_dir.join("input.webp").to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(identify.stdout).unwrap().trim(),
        "format=WEBP width=2 height=1 channels=RGB depth=8"
    );
}

#[test]
fn gif_is_rejected_as_output_target() {
    let dir = temp_dir("input_only_output");
    let png = dir.join("input.png");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output_path = dir.join("out.gif");
    let output = Command::new(imx())
        .args([png.to_str().unwrap(), output_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success(), "GIF output should be rejected");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("input-only format") && stderr.contains("GIF"),
        "GIF: got stderr={stderr}"
    );
    assert!(!output_path.exists());
}

#[test]
fn gif_is_rejected_as_batch_output_format() {
    let dir = temp_dir("input_only_batch");
    let png = dir.join("input.png");
    let out_dir = dir.join("out");
    fs::create_dir_all(&out_dir).unwrap();
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&png, imx_codec_png::encode(&image).unwrap()).unwrap();

    let output = Command::new(imx())
        .args([
            "batch-convert",
            "--to",
            "GIF",
            "--output-dir",
            out_dir.to_str().unwrap(),
            png.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("input-only format"));
}
