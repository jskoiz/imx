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

fn write_supported_fixtures(dir: &Path) -> Vec<(&'static str, PathBuf, &'static str)> {
    let ff = dir.join("input.ff");
    let qoi = dir.join("input.qoi");
    let pbm = dir.join("input.pbm");
    let pgm = dir.join("input.pgm");
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

    vec![
        (
            "FARBFELD",
            ff,
            "format=FARBFELD width=2 height=1 channels=RGBA depth=16",
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
    ]
}

#[test]
fn identifies_farbfeld_qoi_pbm_pgm_and_ppm() {
    let dir = temp_dir("identify");
    let ff = dir.join("input.ff");
    let qoi = dir.join("input.qoi");
    let pbm = dir.join("input.pbm");
    let ppm = dir.join("input.ppm");
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
        &qoi,
        imx_codec_qoi::encode_image(&image, imx_codec_qoi::QOI_SRGB).unwrap(),
    )
    .unwrap();
    fs::write(&ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
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
        "format=PPM width=1 height=1 channels=RGB depth=8"
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
    assert_eq!(
        fs::read(input_ppm).unwrap(),
        fs::read(roundtrip_ppm).unwrap()
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
    let qoi = dir.join("input.qoi");
    let pbm = dir.join("input.pbm");
    let pgm = dir.join("input.pgm");
    let ppm = dir.join("input.ppm");

    fs::write(&ff, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();
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

    for (name, input, output_name, expected_identify) in [
        (
            "farbfeld",
            ff.as_path(),
            "output.ff",
            "format=FARBFELD width=2 height=1 channels=RGBA depth=16",
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
fn malformed_format_prefixes_are_rejected() {
    let dir = temp_dir("malformed_prefixes");
    let ppm = dir.join("input.ppm");
    let qoi = dir.join("input.qoi");
    let image = Image::new(1, 1, PixelFormat::Rgb8, vec![255, 0, 0]).unwrap();
    fs::write(&ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();
    fs::write(
        &qoi,
        imx_codec_qoi::encode_image(&image, imx_codec_qoi::QOI_SRGB).unwrap(),
    )
    .unwrap();

    let output_ppm = dir.join("out.ppm");
    let extensionless_output = dir.join("out");
    let cases = vec![
        (
            vec!["identify".to_string(), prefixed("PNG", &ppm)],
            "unsupported format prefix: PNG",
        ),
        (
            vec!["identify".to_string(), "PPM:".to_string()],
            "missing path after format prefix PPM:",
        ),
        (
            vec!["identify".to_string(), prefixed("PPM", &qoi)],
            "format prefix PPM does not match detected format QOI",
        ),
        (
            vec![prefixed("PPM", &ppm), prefixed("QOI", &output_ppm)],
            "format prefix QOI does not match path format PPM",
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
