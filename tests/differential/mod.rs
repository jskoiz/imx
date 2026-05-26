use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use imx_core::{Image, PixelFormat};

fn require_oracle() -> bool {
    std::env::var("IMX_REQUIRE_ORACLE").is_ok_and(|value| value == "1" || value == "true")
}

fn magick_command() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("IMAGEMAGICK_MAGICK") {
        return Some(PathBuf::from(path));
    }
    for candidate in ["../utilities/magick", "../magick", "magick"] {
        let output = Command::new(candidate).arg("-version").output();
        if output.as_ref().is_ok_and(|output| output.status.success()) {
            return Some(PathBuf::from(candidate));
        }
    }
    None
}

fn standalone_imx_command() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("IMX_STANDALONE_BIN") {
        return Some(PathBuf::from(path));
    }

    let exe = std::env::current_exe().ok()?;
    let target_dir = exe.parent()?.parent()?;
    let candidate = target_dir.join(if cfg!(windows) { "imx.exe" } else { "imx" });
    candidate.exists().then_some(candidate)
}

fn require_or_skip(path: Option<PathBuf>, what: &str) -> Option<PathBuf> {
    match (path, require_oracle()) {
        (Some(path), _) => Some(path),
        (None, true) => panic!("{what} is required for release differential tests"),
        (None, false) => {
            eprintln!("skip: {what} not found");
            None
        }
    }
}

fn run_magick(magick: &Path, args: &[String]) -> Output {
    Command::new(magick)
        .args(args)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "failed to launch ImageMagick oracle {}: {err}",
                magick.display()
            )
        })
}

fn assert_success_or_skip(output: &Output, context: &str) -> bool {
    if output.status.success() {
        return true;
    }
    if require_oracle() {
        panic!(
            "{context} failed in required oracle lane\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    eprintln!(
        "skip: {context} failed\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

fn temp_dir(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    dir.push(format!("imx_diff_{name}_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn rgba16be_fixture() -> Image {
    Image::new(
        2,
        2,
        PixelFormat::Rgba16Be,
        vec![
            0x00, 0x00, 0xff, 0xff, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00,
            0x80, 0x80, 0x12, 0x12, 0x34, 0x34, 0x56, 0x56, 0x78, 0x78, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0x00, 0x00,
        ],
    )
    .unwrap()
}

fn pbm_ascii_fixture() -> &'static [u8] {
    b"P1\n# pbm 1=black 0=white\n5 3\n01010\n10101\n00110\n"
}

fn pbm_binary_fixture() -> &'static [u8] {
    b"P4\n10 2\n\x55\x40\xcc\x80"
}

fn magick_to_gray8(magick: &Path, format_name: &str, input: &Path, output: &Path) -> Output {
    run_magick(
        magick,
        &[
            format!("{format_name}:{}", input.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("GRAY:{}", output.display()),
        ],
    )
}

#[test]
fn imagemagick_oracle_decodes_standalone_farbfeld_to_expected_raw_rgba() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let dir = temp_dir("farbfeld_decode");
    let input = dir.join("input.ff");
    let output = dir.join("output.rgba");
    let image = rgba16be_fixture();
    fs::write(&input, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();

    let result = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", input.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("RGBA:{}", output.display()),
        ],
    );
    if !assert_success_or_skip(&result, "ImageMagick FARBFELD decode") {
        return;
    }

    assert_eq!(fs::read(output).unwrap(), image.pixels());
}

#[test]
fn standalone_farbfeld_to_qoi_matches_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("ff_to_qoi");
    let input = dir.join("input.ff");
    let output_qoi = dir.join("output.qoi");
    let im_raw = dir.join("im.rgba");
    let rust_raw = dir.join("rust.rgba");
    let image = rgba16be_fixture();
    fs::write(&input, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();

    let standalone_result = run_magick(
        &standalone,
        &[
            input.display().to_string(),
            output_qoi.display().to_string(),
        ],
    );
    assert!(
        standalone_result.status.success(),
        "standalone FARBFELD->QOI failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&standalone_result.stdout),
        String::from_utf8_lossy(&standalone_result.stderr)
    );

    let im = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", input.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGBA:{}", im_raw.display()),
        ],
    );
    let rust = run_magick(
        &magick,
        &[
            format!("QOI:{}", output_qoi.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGBA:{}", rust_raw.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick FARBFELD decode")
        || !assert_success_or_skip(&rust, "ImageMagick QOI decode")
    {
        return;
    }
    assert_eq!(fs::read(im_raw).unwrap(), fs::read(rust_raw).unwrap());
}

#[test]
fn standalone_qoi_to_farbfeld_matches_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("qoi_to_ff");
    let input = dir.join("input.qoi");
    let output_ff = dir.join("output.ff");
    let im_raw = dir.join("im.rgba");
    let rust_raw = dir.join("rust.rgba");
    let pixels = [
        0, 255, 0, 255, 255, 0, 0, 128, 18, 52, 86, 120, 255, 255, 255, 0,
    ];
    fs::write(
        &input,
        imx_codec_qoi::encode(2, 2, 4, imx_codec_qoi::QOI_SRGB, &pixels).unwrap(),
    )
    .unwrap();

    let standalone_result = run_magick(
        &standalone,
        &[input.display().to_string(), output_ff.display().to_string()],
    );
    assert!(
        standalone_result.status.success(),
        "standalone QOI->FARBFELD failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&standalone_result.stdout),
        String::from_utf8_lossy(&standalone_result.stderr)
    );

    let im = run_magick(
        &magick,
        &[
            format!("QOI:{}", input.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("RGBA:{}", im_raw.display()),
        ],
    );
    let rust = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", output_ff.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("RGBA:{}", rust_raw.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick QOI decode")
        || !assert_success_or_skip(&rust, "ImageMagick FARBFELD decode")
    {
        return;
    }
    assert_eq!(fs::read(im_raw).unwrap(), fs::read(rust_raw).unwrap());
}

#[test]
fn standalone_ppm_transcodes_match_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("ppm_transcodes");
    let input_ppm = dir.join("input.ppm");
    let output_ff = dir.join("output.ff");
    let output_qoi = dir.join("output.qoi");
    let im_rgb = dir.join("im.rgb");
    let ff_rgb = dir.join("ff.rgb");
    let qoi_rgb = dir.join("qoi.rgb");

    let image = Image::new(
        3,
        1,
        PixelFormat::Rgb8,
        vec![255, 0, 0, 0, 128, 255, 17, 34, 51],
    )
    .unwrap();
    fs::write(&input_ppm, imx_codec_pnm::encode_ppm(&image).unwrap()).unwrap();

    for output in [&output_ff, &output_qoi] {
        let result = run_magick(
            &standalone,
            &[
                input_ppm.display().to_string(),
                output.display().to_string(),
            ],
        );
        assert!(
            result.status.success(),
            "standalone PPM transcode failed for {}\nstdout:\n{}\nstderr:\n{}",
            output.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    let im = run_magick(
        &magick,
        &[
            format!("PPM:{}", input_ppm.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", im_rgb.display()),
        ],
    );
    let ff = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", output_ff.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", ff_rgb.display()),
        ],
    );
    let qoi = run_magick(
        &magick,
        &[
            format!("QOI:{}", output_qoi.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", qoi_rgb.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick PPM decode")
        || !assert_success_or_skip(&ff, "ImageMagick FARBFELD decode")
        || !assert_success_or_skip(&qoi, "ImageMagick QOI decode")
    {
        return;
    }

    let expected = fs::read(im_rgb).unwrap();
    assert_eq!(fs::read(ff_rgb).unwrap(), expected);
    assert_eq!(fs::read(qoi_rgb).unwrap(), expected);
}

#[test]
fn standalone_ascii_ppm_decode_matches_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("ppm_ascii");
    let input_ppm = dir.join("input.ppm");
    let output_ff = dir.join("output.ff");
    let im_rgb = dir.join("im.rgb");
    let rust_rgb = dir.join("rust.rgb");

    fs::write(
        &input_ppm,
        b"P3\n# comments and lower maxval\n2 1\n31\n0 15 31\n31 0 15\n",
    )
    .unwrap();

    let standalone_result = run_magick(
        &standalone,
        &[
            input_ppm.display().to_string(),
            output_ff.display().to_string(),
        ],
    );
    assert!(
        standalone_result.status.success(),
        "standalone P3 PPM transcode failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&standalone_result.stdout),
        String::from_utf8_lossy(&standalone_result.stderr)
    );

    let im = run_magick(
        &magick,
        &[
            format!("PPM:{}", input_ppm.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", im_rgb.display()),
        ],
    );
    let rust = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", output_ff.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", rust_rgb.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick P3 PPM decode")
        || !assert_success_or_skip(&rust, "ImageMagick FARBFELD decode")
    {
        return;
    }
    assert_eq!(fs::read(rust_rgb).unwrap(), fs::read(im_rgb).unwrap());
}

#[test]
fn standalone_high_depth_ppm_transcodes_match_imagemagick_16bit_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("ppm16_transcodes");
    let input_ppm = dir.join("input.ppm");
    let output_ff = dir.join("output.ff");
    let output_pgm = dir.join("output.pgm");
    let im_rgb = dir.join("im.rgb16");
    let ff_rgb = dir.join("ff.rgb16");
    let im_gray = dir.join("im.gray16");
    let pgm_gray = dir.join("pgm.gray16");

    fs::write(
        &input_ppm,
        b"P6\n3 1\n65535\n\x00\x00\x00\x00\x00\x00\x80\x00\x80\x00\x80\x00\xff\xff\xff\xff\xff\xff",
    )
    .unwrap();

    for output in [&output_ff, &output_pgm] {
        let result = run_magick(
            &standalone,
            &[
                input_ppm.display().to_string(),
                output.display().to_string(),
            ],
        );
        assert!(
            result.status.success(),
            "standalone high-depth PPM transcode failed for {}\nstdout:\n{}\nstderr:\n{}",
            output.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    let im = run_magick(
        &magick,
        &[
            format!("PPM:{}", input_ppm.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("RGB:{}", im_rgb.display()),
        ],
    );
    let ff = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", output_ff.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("RGB:{}", ff_rgb.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick PPM16 decode")
        || !assert_success_or_skip(&ff, "ImageMagick FARBFELD decode")
    {
        return;
    }
    assert_eq!(fs::read(ff_rgb).unwrap(), fs::read(im_rgb).unwrap());

    let im = run_magick(
        &magick,
        &[
            format!("PPM:{}", input_ppm.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("GRAY:{}", im_gray.display()),
        ],
    );
    let pgm = run_magick(
        &magick,
        &[
            format!("PGM:{}", output_pgm.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("GRAY:{}", pgm_gray.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick PPM16 grayscale decode")
        || !assert_success_or_skip(&pgm, "ImageMagick PGM16 decode")
    {
        return;
    }
    assert_eq!(fs::read(pgm_gray).unwrap(), fs::read(im_gray).unwrap());
}

#[test]
fn standalone_pgm16_to_ppm16_matches_imagemagick_16bit_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("pgm16_to_ppm16");
    let input_pgm = dir.join("input.pgm");
    let output_ppm = dir.join("output.ppm");
    let oracle_ppm = dir.join("oracle.ppm");
    let rust_rgb = dir.join("rust.rgb16");
    let oracle_rgb = dir.join("oracle.rgb16");

    fs::write(&input_pgm, b"P5\n3 1\n65535\n\x00\x00\x80\x00\xff\xff").unwrap();

    let standalone_result = run_magick(
        &standalone,
        &[
            input_pgm.display().to_string(),
            output_ppm.display().to_string(),
        ],
    );
    assert!(
        standalone_result.status.success(),
        "standalone PGM16->PPM failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&standalone_result.stdout),
        String::from_utf8_lossy(&standalone_result.stderr)
    );

    let oracle_result = run_magick(
        &magick,
        &[
            format!("PGM:{}", input_pgm.display()),
            format!("PPM:{}", oracle_ppm.display()),
        ],
    );
    if !assert_success_or_skip(&oracle_result, "ImageMagick PGM16->PPM encode") {
        return;
    }

    let rust = run_magick(
        &magick,
        &[
            format!("PPM:{}", output_ppm.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("RGB:{}", rust_rgb.display()),
        ],
    );
    let oracle = run_magick(
        &magick,
        &[
            format!("PPM:{}", oracle_ppm.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("RGB:{}", oracle_rgb.display()),
        ],
    );
    if !assert_success_or_skip(&rust, "ImageMagick PPM decode")
        || !assert_success_or_skip(&oracle, "ImageMagick oracle PPM decode")
    {
        return;
    }
    assert_eq!(fs::read(rust_rgb).unwrap(), fs::read(oracle_rgb).unwrap());
}

#[test]
fn standalone_ascii_and_binary_pbm_decode_match_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("pbm_decode");
    let ascii_pbm = dir.join("ascii.pbm");
    let binary_pbm = dir.join("binary.pbm");
    let ascii_ff = dir.join("ascii.ff");
    let binary_ff = dir.join("binary.ff");
    let im_ascii = dir.join("im-ascii.gray");
    let rust_ascii = dir.join("rust-ascii.gray");
    let im_binary = dir.join("im-binary.gray");
    let rust_binary = dir.join("rust-binary.gray");
    fs::write(&ascii_pbm, pbm_ascii_fixture()).unwrap();
    fs::write(&binary_pbm, pbm_binary_fixture()).unwrap();

    for (input, output) in [(&ascii_pbm, &ascii_ff), (&binary_pbm, &binary_ff)] {
        let result = run_magick(
            &standalone,
            &[input.display().to_string(), output.display().to_string()],
        );
        assert!(
            result.status.success(),
            "standalone PBM->FARBFELD failed for {}\nstdout:\n{}\nstderr:\n{}",
            input.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    let im = magick_to_gray8(&magick, "PBM", &ascii_pbm, &im_ascii);
    let rust = magick_to_gray8(&magick, "FARBFELD", &ascii_ff, &rust_ascii);
    if !assert_success_or_skip(&im, "ImageMagick P1 PBM decode")
        || !assert_success_or_skip(&rust, "ImageMagick FARBFELD decode")
    {
        return;
    }
    assert_eq!(fs::read(rust_ascii).unwrap(), fs::read(im_ascii).unwrap());

    let im = magick_to_gray8(&magick, "PBM", &binary_pbm, &im_binary);
    let rust = magick_to_gray8(&magick, "FARBFELD", &binary_ff, &rust_binary);
    if !assert_success_or_skip(&im, "ImageMagick P4 PBM decode")
        || !assert_success_or_skip(&rust, "ImageMagick FARBFELD decode")
    {
        return;
    }
    assert_eq!(fs::read(rust_binary).unwrap(), fs::read(im_binary).unwrap());
}

#[test]
fn standalone_pbm_transcodes_match_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("pbm_transcodes");
    let input_pbm = dir.join("input.pbm");
    let output_ff = dir.join("output.ff");
    let output_qoi = dir.join("output.qoi");
    let output_ppm = dir.join("output.ppm");
    let output_pgm = dir.join("output.pgm");
    let im_gray = dir.join("im.gray");
    let ff_gray = dir.join("ff.gray");
    let qoi_gray = dir.join("qoi.gray");
    let ppm_gray = dir.join("ppm.gray");
    let pgm_gray = dir.join("pgm.gray");
    fs::write(&input_pbm, pbm_binary_fixture()).unwrap();

    for output in [&output_ff, &output_qoi, &output_ppm, &output_pgm] {
        let result = run_magick(
            &standalone,
            &[
                input_pbm.display().to_string(),
                output.display().to_string(),
            ],
        );
        assert!(
            result.status.success(),
            "standalone PBM transcode failed for {}\nstdout:\n{}\nstderr:\n{}",
            output.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    let im = magick_to_gray8(&magick, "PBM", &input_pbm, &im_gray);
    let ff = magick_to_gray8(&magick, "FARBFELD", &output_ff, &ff_gray);
    let qoi = magick_to_gray8(&magick, "QOI", &output_qoi, &qoi_gray);
    let ppm = magick_to_gray8(&magick, "PPM", &output_ppm, &ppm_gray);
    let pgm = magick_to_gray8(&magick, "PGM", &output_pgm, &pgm_gray);
    if !assert_success_or_skip(&im, "ImageMagick PBM decode")
        || !assert_success_or_skip(&ff, "ImageMagick FARBFELD decode")
        || !assert_success_or_skip(&qoi, "ImageMagick QOI decode")
        || !assert_success_or_skip(&ppm, "ImageMagick PPM decode")
        || !assert_success_or_skip(&pgm, "ImageMagick PGM decode")
    {
        return;
    }

    let expected = fs::read(im_gray).unwrap();
    assert_eq!(fs::read(ff_gray).unwrap(), expected);
    assert_eq!(fs::read(qoi_gray).unwrap(), expected);
    assert_eq!(fs::read(ppm_gray).unwrap(), expected);
    assert_eq!(fs::read(pgm_gray).unwrap(), expected);
}

#[test]
fn standalone_farbfeld_to_pbm_thresholds_like_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("ff_to_pbm_threshold");
    let input_ff = dir.join("input.ff");
    let output_pbm = dir.join("output.pbm");
    let im_pbm = dir.join("im.pbm");
    let im_gray = dir.join("im.gray");
    let rust_gray = dir.join("rust.gray");
    let image = Image::new(
        4,
        1,
        PixelFormat::Rgba16Be,
        vec![
            0, 0, 0, 0, 0, 0, 0xff, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0xff, 0xff, 0x80,
            0x00, 0x80, 0x00, 0x80, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff,
        ],
    )
    .unwrap();
    fs::write(&input_ff, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();

    let standalone_result = run_magick(
        &standalone,
        &[
            input_ff.display().to_string(),
            output_pbm.display().to_string(),
        ],
    );
    assert!(
        standalone_result.status.success(),
        "standalone FARBFELD->PBM failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&standalone_result.stdout),
        String::from_utf8_lossy(&standalone_result.stderr)
    );
    assert_eq!(fs::read(&output_pbm).unwrap(), b"P4\n4 1\n\xc0");

    let im_encode = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", input_ff.display()),
            format!("PBM:{}", im_pbm.display()),
        ],
    );
    let im = magick_to_gray8(&magick, "PBM", &im_pbm, &im_gray);
    let rust = magick_to_gray8(&magick, "PBM", &output_pbm, &rust_gray);
    if !assert_success_or_skip(&im_encode, "ImageMagick FARBFELD->PBM encode")
        || !assert_success_or_skip(&im, "ImageMagick PBM decode")
        || !assert_success_or_skip(&rust, "ImageMagick PBM decode")
    {
        return;
    }
    assert_eq!(fs::read(rust_gray).unwrap(), fs::read(im_gray).unwrap());
}

#[test]
fn standalone_pgm_transcodes_match_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("pgm_transcodes");
    let input_pgm = dir.join("input.pgm");
    let output_ff = dir.join("output.ff");
    let output_qoi = dir.join("output.qoi");
    let output_ppm = dir.join("output.ppm");
    let im_gray = dir.join("im.gray");
    let ff_gray = dir.join("ff.gray");
    let qoi_gray = dir.join("qoi.gray");
    let ppm_gray = dir.join("ppm.gray");

    let image = Image::new(3, 1, PixelFormat::Gray8, vec![0, 128, 255]).unwrap();
    fs::write(&input_pgm, imx_codec_pnm::encode_pgm(&image).unwrap()).unwrap();

    for output in [&output_ff, &output_qoi, &output_ppm] {
        let result = run_magick(
            &standalone,
            &[
                input_pgm.display().to_string(),
                output.display().to_string(),
            ],
        );
        assert!(
            result.status.success(),
            "standalone PGM transcode failed for {}\nstdout:\n{}\nstderr:\n{}",
            output.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    let im = run_magick(
        &magick,
        &[
            format!("PGM:{}", input_pgm.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("GRAY:{}", im_gray.display()),
        ],
    );
    let ff = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", output_ff.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("GRAY:{}", ff_gray.display()),
        ],
    );
    let qoi = run_magick(
        &magick,
        &[
            format!("QOI:{}", output_qoi.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("GRAY:{}", qoi_gray.display()),
        ],
    );
    let ppm = run_magick(
        &magick,
        &[
            format!("PPM:{}", output_ppm.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("GRAY:{}", ppm_gray.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick PGM decode")
        || !assert_success_or_skip(&ff, "ImageMagick FARBFELD decode")
        || !assert_success_or_skip(&qoi, "ImageMagick QOI decode")
        || !assert_success_or_skip(&ppm, "ImageMagick PPM decode")
    {
        return;
    }

    let expected = fs::read(im_gray).unwrap();
    assert_eq!(fs::read(ff_gray).unwrap(), expected);
    assert_eq!(fs::read(qoi_gray).unwrap(), expected);
    assert_eq!(fs::read(ppm_gray).unwrap(), expected);
}

#[test]
fn standalone_ascii_and_16bit_pgm_decode_match_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("pgm_ascii_16bit");
    let ascii_pgm = dir.join("ascii.pgm");
    let binary16_pgm = dir.join("binary16.pgm");
    let ascii_ff = dir.join("ascii.ff");
    let binary16_ff = dir.join("binary16.ff");
    let im_ascii = dir.join("im-ascii.gray");
    let rust_ascii = dir.join("rust-ascii.gray");
    let im_binary16 = dir.join("im-binary16.gray");
    let rust_binary16 = dir.join("rust-binary16.gray");

    fs::write(
        &ascii_pgm,
        b"P2\n# comments and lower maxval\n3 1\n31\n0 15 31\n",
    )
    .unwrap();
    fs::write(&binary16_pgm, b"P5\n3 1\n65535\n\x00\x00\x80\x00\xff\xff").unwrap();

    for (input, output) in [(&ascii_pgm, &ascii_ff), (&binary16_pgm, &binary16_ff)] {
        let result = run_magick(
            &standalone,
            &[input.display().to_string(), output.display().to_string()],
        );
        assert!(
            result.status.success(),
            "standalone PGM->FARBFELD failed for {}\nstdout:\n{}\nstderr:\n{}",
            input.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    let im = run_magick(
        &magick,
        &[
            format!("PGM:{}", ascii_pgm.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("GRAY:{}", im_ascii.display()),
        ],
    );
    let rust = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", ascii_ff.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("GRAY:{}", rust_ascii.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick P2 PGM decode")
        || !assert_success_or_skip(&rust, "ImageMagick FARBFELD decode")
    {
        return;
    }
    assert_eq!(fs::read(rust_ascii).unwrap(), fs::read(im_ascii).unwrap());

    let im = run_magick(
        &magick,
        &[
            format!("PGM:{}", binary16_pgm.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("GRAY:{}", im_binary16.display()),
        ],
    );
    let rust = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", binary16_ff.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("GRAY:{}", rust_binary16.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick P5 16-bit PGM decode")
        || !assert_success_or_skip(&rust, "ImageMagick FARBFELD decode")
    {
        return;
    }
    assert_eq!(
        fs::read(rust_binary16).unwrap(),
        fs::read(im_binary16).unwrap()
    );
}

#[test]
fn standalone_farbfeld_to_pgm_quantizes_like_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("ff_to_pgm_quantization");
    let input_ff = dir.join("input.ff");
    let output_pgm = dir.join("output.pgm");
    let im_gray = dir.join("im.gray");
    let rust_gray = dir.join("rust.gray");
    let image = Image::new(
        4,
        1,
        PixelFormat::Rgba16Be,
        vec![
            0xff, 0xff, 0, 0, 0, 0, 0xff, 0xff, 0, 0, 0xff, 0xff, 0, 0, 0xff, 0xff, 0, 0, 0, 0,
            0xff, 0xff, 0xff, 0xff, 0x80, 0x00, 0x40, 0x00, 0x20, 0x00, 0xff, 0xff,
        ],
    )
    .unwrap();
    fs::write(&input_ff, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();

    let standalone_result = run_magick(
        &standalone,
        &[
            input_ff.display().to_string(),
            output_pgm.display().to_string(),
        ],
    );
    assert!(
        standalone_result.status.success(),
        "standalone FARBFELD->PGM failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&standalone_result.stdout),
        String::from_utf8_lossy(&standalone_result.stderr)
    );

    let im = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", input_ff.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("GRAY:{}", im_gray.display()),
        ],
    );
    let rust = run_magick(
        &magick,
        &[
            format!("PGM:{}", output_pgm.display()),
            "-depth".to_string(),
            "16".to_string(),
            "-endian".to_string(),
            "MSB".to_string(),
            format!("GRAY:{}", rust_gray.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick FARBFELD decode")
        || !assert_success_or_skip(&rust, "ImageMagick PGM decode")
    {
        return;
    }
    assert_eq!(fs::read(rust_gray).unwrap(), fs::read(im_gray).unwrap());
}

#[test]
fn standalone_farbfeld_to_ppm_quantizes_like_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("ff_to_ppm_quantization");
    let input_ff = dir.join("input.ff");
    let output_ppm = dir.join("output.ppm");
    let im_rgb = dir.join("im.rgb");
    let rust_rgb = dir.join("rust.rgb");
    let image = Image::new(
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
    fs::write(&input_ff, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();

    let standalone_result = run_magick(
        &standalone,
        &[
            input_ff.display().to_string(),
            output_ppm.display().to_string(),
        ],
    );
    assert!(
        standalone_result.status.success(),
        "standalone FARBFELD->PPM failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&standalone_result.stdout),
        String::from_utf8_lossy(&standalone_result.stderr)
    );

    let im = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", input_ff.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", im_rgb.display()),
        ],
    );
    let rust = run_magick(
        &magick,
        &[
            format!("PPM:{}", output_ppm.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", rust_rgb.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick FARBFELD decode")
        || !assert_success_or_skip(&rust, "ImageMagick PPM decode")
    {
        return;
    }
    assert_eq!(fs::read(rust_rgb).unwrap(), fs::read(im_rgb).unwrap());
}

#[test]
fn standalone_png_transcodes_match_imagemagick_decoded_pixels() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("png_transcodes");
    let input_png = dir.join("input.png");
    let output_ff = dir.join("output.ff");
    let output_png = dir.join("output.png");
    let oracle_ff = dir.join("oracle.ff");
    let png_rgb = dir.join("png.rgb");
    let ff_rgb = dir.join("ff.rgb");
    let out_png_rgb = dir.join("out-png.rgb");
    let oracle_ff_rgb = dir.join("oracle-ff.rgb");

    let image = Image::new(
        2,
        2,
        PixelFormat::Rgb8,
        vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
    )
    .unwrap();
    fs::write(&input_png, imx_codec_png::encode(&image).unwrap()).unwrap();

    for output in [&output_ff, &output_png] {
        let result = run_magick(
            &standalone,
            &[
                input_png.display().to_string(),
                output.display().to_string(),
            ],
        );
        assert!(
            result.status.success(),
            "standalone PNG transcode failed for {}\nstdout:\n{}\nstderr:\n{}",
            output.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    let oracle_result = run_magick(
        &magick,
        &[
            format!("PNG:{}", input_png.display()),
            format!("FARBFELD:{}", oracle_ff.display()),
        ],
    );
    if !assert_success_or_skip(&oracle_result, "ImageMagick PNG->FARBFELD encode") {
        return;
    }

    let im = run_magick(
        &magick,
        &[
            format!("PNG:{}", input_png.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", png_rgb.display()),
        ],
    );
    let ff = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", output_ff.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", ff_rgb.display()),
        ],
    );
    let out_png = run_magick(
        &magick,
        &[
            format!("PNG:{}", output_png.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", out_png_rgb.display()),
        ],
    );
    let oracle_ff_decode = run_magick(
        &magick,
        &[
            format!("FARBFELD:{}", oracle_ff.display()),
            "-depth".to_string(),
            "8".to_string(),
            format!("RGB:{}", oracle_ff_rgb.display()),
        ],
    );
    if !assert_success_or_skip(&im, "ImageMagick PNG decode")
        || !assert_success_or_skip(&ff, "ImageMagick FARBFELD decode")
        || !assert_success_or_skip(&out_png, "ImageMagick PNG output decode")
        || !assert_success_or_skip(&oracle_ff_decode, "ImageMagick oracle FARBFELD decode")
    {
        return;
    }

    let expected = fs::read(png_rgb).unwrap();
    assert_eq!(fs::read(ff_rgb).unwrap(), expected);
    assert_eq!(fs::read(out_png_rgb).unwrap(), expected);
    assert_eq!(fs::read(oracle_ff_rgb).unwrap(), expected);
}

#[test]
fn supported_identify_fields_match_imagemagick_oracle_when_available() {
    let Some(magick) = require_or_skip(magick_command(), "ImageMagick oracle") else {
        return;
    };
    let Some(standalone) = require_or_skip(standalone_imx_command(), "standalone imx binary")
    else {
        return;
    };
    let dir = temp_dir("identify");
    let input = dir.join("input.ff");
    let image = rgba16be_fixture();
    fs::write(&input, imx_codec_farbfeld::encode(&image).unwrap()).unwrap();

    let result = run_magick(
        &magick,
        &[
            "identify".to_string(),
            "-format".to_string(),
            "%m %w %h %[depth]".to_string(),
            input.display().to_string(),
        ],
    );
    if !assert_success_or_skip(&result, "ImageMagick identify") {
        return;
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("FARBFELD") || stdout.contains("FF"));
    assert!(stdout.contains("2 2 16"));

    let standalone_result = run_magick(
        &standalone,
        &["identify".to_string(), input.display().to_string()],
    );
    assert!(
        standalone_result.status.success(),
        "standalone identify failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&standalone_result.stdout),
        String::from_utf8_lossy(&standalone_result.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&standalone_result.stdout).trim(),
        "format=FARBFELD width=2 height=2 channels=RGBA depth=16"
    );

    let ppm = dir.join("input.ppm");
    fs::write(
        &ppm,
        imx_codec_pnm::encode_ppm(&image.to_rgb8().unwrap()).unwrap(),
    )
    .unwrap();
    let result = run_magick(
        &magick,
        &[
            "identify".to_string(),
            "-format".to_string(),
            "%m %w %h %[depth]".to_string(),
            ppm.display().to_string(),
        ],
    );
    if !assert_success_or_skip(&result, "ImageMagick PPM identify") {
        return;
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("PPM"));
    assert!(stdout.contains("2 2 8"));

    let standalone_result = run_magick(
        &standalone,
        &["identify".to_string(), ppm.display().to_string()],
    );
    assert!(standalone_result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&standalone_result.stdout).trim(),
        "format=PPM width=2 height=2 channels=RGB depth=8"
    );

    let pbm = dir.join("input.pbm");
    fs::write(&pbm, pbm_binary_fixture()).unwrap();
    let result = run_magick(
        &magick,
        &[
            "identify".to_string(),
            "-format".to_string(),
            "%m %w %h %[colorspace] %[depth]".to_string(),
            pbm.display().to_string(),
        ],
    );
    if !assert_success_or_skip(&result, "ImageMagick PBM identify") {
        return;
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("PBM"));
    assert!(stdout.contains("10 2"));
    assert!(stdout.contains("Gray"));

    let standalone_result = run_magick(
        &standalone,
        &["identify".to_string(), pbm.display().to_string()],
    );
    assert!(standalone_result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&standalone_result.stdout).trim(),
        "format=PBM width=10 height=2 channels=GRAY depth=1"
    );

    let pgm = dir.join("input.pgm");
    fs::write(
        &pgm,
        imx_codec_pnm::encode_pgm(
            &Image::new(2, 2, PixelFormat::Gray8, vec![0, 85, 170, 255]).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let result = run_magick(
        &magick,
        &[
            "identify".to_string(),
            "-format".to_string(),
            "%m %w %h %[colorspace]".to_string(),
            pgm.display().to_string(),
        ],
    );
    if !assert_success_or_skip(&result, "ImageMagick PGM identify") {
        return;
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("PGM"));
    assert!(stdout.contains("2 2"));
    assert!(stdout.contains("Gray"));

    let standalone_result = run_magick(
        &standalone,
        &["identify".to_string(), pgm.display().to_string()],
    );
    assert!(standalone_result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&standalone_result.stdout).trim(),
        "format=PGM width=2 height=2 channels=GRAY depth=8"
    );

    let png = dir.join("input.png");
    fs::write(
        &png,
        imx_codec_png::encode(
            &Image::new(
                2,
                2,
                PixelFormat::Rgb8,
                vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let result = run_magick(
        &magick,
        &[
            "identify".to_string(),
            "-format".to_string(),
            "%m %w %h %[colorspace] %[depth]".to_string(),
            png.display().to_string(),
        ],
    );
    if !assert_success_or_skip(&result, "ImageMagick PNG identify") {
        return;
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("PNG"));
    assert!(stdout.contains("2 2"));

    let standalone_result = run_magick(
        &standalone,
        &["identify".to_string(), png.display().to_string()],
    );
    assert!(standalone_result.status.success());
    assert_eq!(
        String::from_utf8_lossy(&standalone_result.stdout).trim(),
        "format=PNG width=2 height=2 channels=RGB depth=8"
    );
}
