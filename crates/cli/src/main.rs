use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::time::{SystemTime, UNIX_EPOCH};

use imx_core::{Format, Identify, ImageError, MAX_PIXEL_BYTES};

const MAX_INPUT_BYTES: u64 = MAX_PIXEL_BYTES as u64 + 1024 * 1024;

fn usage() -> ! {
    eprintln!(
        "usage:\n  imx --help\n  imx --version\n  imx identify [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>\n  imx identify --json [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>\n  imx report --json [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>\n  imx resize <width>x<height> [FORMAT:]<input> [FORMAT:]<output>\n  imx resize-fit <width>x<height> [FORMAT:]<input> [FORMAT:]<output>\n  imx crop <width>x<height>+<x>+<y> [FORMAT:]<input> [FORMAT:]<output>\n  imx rotate <90|180|270> [FORMAT:]<input> [FORMAT:]<output>\n  imx flip [FORMAT:]<input> [FORMAT:]<output>\n  imx flop [FORMAT:]<input> [FORMAT:]<output>\n  imx batch-convert --to <FORMAT> --output-dir <dir> [--resize <width>x<height>|--resize-fit <width>x<height>] [FORMAT:]<input>...\n  imx self-test\n  imx [FORMAT:]<input> [FORMAT:]<output>\n\nsupported formats: bmp (.bmp), farbfeld (.ff, .farbfeld), jpeg (.jpg, .jpeg), qoi (.qoi), pbm (.pbm), pgm (.pgm), png (.png), ppm (.ppm)\nsupported prefixes: BMP:, FARBFELD:, JPEG:, QOI:, PBM:, PGM:, PNG:, PPM:"
    );
    process::exit(2);
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("error: {message}");
    process::exit(1);
}

fn fail_usage(message: impl std::fmt::Display) -> ! {
    eprintln!("error: {message}");
    process::exit(2);
}

fn fail_image_operation(
    format: Format,
    operation: &str,
    path_role: &str,
    path: &CliPath<'_>,
    err: ImageError,
) -> ! {
    fail(format!(
        "failed to {operation} {} {path_role} {}: {err}",
        format.name(),
        path.original
    ));
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    match args.as_slice() {
        [_, flag] if flag == "--help" || flag == "-h" || flag == "help" => {
            println!(
                "IMX Developer Preview\n\nusage:\n  imx identify [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>\n  imx identify --json [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>\n  imx report --json [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>\n  imx resize <width>x<height> [FORMAT:]<input> [FORMAT:]<output>\n  imx resize-fit <width>x<height> [FORMAT:]<input> [FORMAT:]<output>\n  imx crop <width>x<height>+<x>+<y> [FORMAT:]<input> [FORMAT:]<output>\n  imx rotate <90|180|270> [FORMAT:]<input> [FORMAT:]<output>\n  imx flip [FORMAT:]<input> [FORMAT:]<output>\n  imx flop [FORMAT:]<input> [FORMAT:]<output>\n  imx batch-convert --to <FORMAT> --output-dir <dir> [--resize <width>x<height>|--resize-fit <width>x<height>] [FORMAT:]<input>...\n  imx self-test\n  imx [FORMAT:]<input> [FORMAT:]<output>\n\nsupported transcodes: BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM, including deterministic same-format rewrites except lossy JPEG re-encoding\nsupported identify JSON: deterministic schema_version/format/width/height/channels/depth over existing identify metadata\nsupported report JSON: single-input supported/unsupported status with stable diagnostic_code values\nsupported resize: nearest-neighbor exact dimensions and aspect-preserving fit for existing supported formats\nsupported geometry: bounds-checked crop (<width>x<height>+<x>+<y>), clockwise rotate (90/180/270), vertical flip, and horizontal flop, all format-preserving\nsupported batch conversion: explicit output format, existing output directory, shell-expanded input paths, no overwrite or collision renaming\nsupported self-test: offline install confidence check for identify/transcode/resize/resize-fit/batch-convert across supported formats\nsupported prefixes: BMP:, FARBFELD:, JPEG:, QOI:, PBM:, PGM:, PNG:, PPM:\nunsupported: stdin/stdout, recursive directory walking, arbitrary-angle rotation, delegates, color management, and formats beyond BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM"
            );
            process::exit(0);
        }
        [_, flag] if flag == "--version" || flag == "-V" || flag == "version" => {
            println!("imx {}", env!("CARGO_PKG_VERSION"));
            process::exit(0);
        }
        [_, command, flag] if command == "identify" && flag == "--json" => usage(),
        [_, command, flag, input] if command == "identify" && flag == "--json" => {
            identify_json(input)
        }
        [_, command, input] if command == "identify" => identify(input),
        [_, command, flag, input] if command == "report" && flag == "--json" => report_json(input),
        [_, command, ..] if command == "report" => usage(),
        [_, command, dimensions, input, output] if command == "resize" => {
            resize(dimensions, input, output)
        }
        [_, command, dimensions, input, output] if command == "resize-fit" => {
            resize_fit(dimensions, input, output)
        }
        [_, command, geometry, input, output] if command == "crop" => crop(geometry, input, output),
        [_, command, ..] if command == "crop" => usage(),
        [_, command, angle, input, output] if command == "rotate" => rotate(angle, input, output),
        [_, command, ..] if command == "rotate" => usage(),
        [_, command, input, output] if command == "flip" => flip(input, output),
        [_, command, ..] if command == "flip" => usage(),
        [_, command, input, output] if command == "flop" => flop(input, output),
        [_, command, ..] if command == "flop" => usage(),
        [_, command, rest @ ..] if command == "batch-convert" => batch_convert(rest),
        [_, command] if command == "self-test" => self_test(),
        [_, command, ..] if command == "self-test" => usage(),
        [_, command, ..] if is_unsupported_command_shape(command) => usage(),
        [_, input, output] => transcode(input, output),
        _ => usage(),
    }
}

fn is_unsupported_command_shape(command: &str) -> bool {
    matches!(command, "convert" | "magick" | "mogrify")
}

#[derive(Clone, Debug)]
struct SelfTestFixture {
    format: Format,
    path: PathBuf,
    expected_identify: &'static str,
}

fn self_test() -> ! {
    if let Err(err) = run_self_test() {
        fail(format!("self-test failed: {err}"));
    }
    println!("self-test: passed");
    process::exit(0);
}

fn run_self_test() -> Result<(), String> {
    let work_dir = self_test_work_dir()?;
    let keep_work_dir = env::var("IMX_SELF_TEST_KEEP")
        .map(|value| value == "1")
        .unwrap_or(false);
    let result = run_self_test_in(&work_dir);
    if !keep_work_dir {
        let _ = fs::remove_dir_all(&work_dir);
    }
    result
}

fn self_test_work_dir() -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("failed to read system clock: {err}"))?
        .as_nanos();
    let path = env::temp_dir().join(format!("imx-self-test-{}-{nanos}", process::id()));
    fs::create_dir_all(&path).map_err(|err| {
        format!(
            "failed to create self-test directory {}: {err}",
            path.display()
        )
    })?;
    Ok(path)
}

fn run_self_test_in(work_dir: &Path) -> Result<(), String> {
    let binary = env::current_exe().map_err(|err| format!("failed to locate imx binary: {err}"))?;
    let fixture_dir = work_dir.join("fixtures");
    fs::create_dir_all(&fixture_dir).map_err(|err| {
        format!(
            "failed to create fixture directory {}: {err}",
            fixture_dir.display()
        )
    })?;
    let fixtures = write_self_test_fixtures(&fixture_dir)?;

    for fixture in &fixtures {
        let stdout = run_self_test_command(
            &binary,
            &format!("identify unprefixed {}", fixture.format.name()),
            &["identify".to_string(), path_string(&fixture.path)?],
        )?;
        require_exact_stdout(&stdout, fixture.expected_identify)?;
        let input = prefixed_path(fixture.format, &fixture.path)?;
        let stdout = run_self_test_command(
            &binary,
            &format!("identify {}", fixture.format.name()),
            &["identify".to_string(), input],
        )?;
        require_exact_stdout(&stdout, fixture.expected_identify)?;
        let input = prefixed_path(fixture.format, &fixture.path)?;
        let stdout = run_self_test_command(
            &binary,
            &format!("identify --json {}", fixture.format.name()),
            &["identify".to_string(), "--json".to_string(), input.clone()],
        )?;
        require_exact_stdout(&stdout, &expected_identify_json(fixture.expected_identify)?)?;
        let stdout = run_self_test_command(
            &binary,
            &format!("report --json {}", fixture.format.name()),
            &["report".to_string(), "--json".to_string(), input],
        )?;
        require_exact_stdout(&stdout, &expected_report_json(fixture.expected_identify)?)?;
    }
    println!("self-test: identify ok");

    let decode_dir = work_dir.join("decode-transcodes");
    fs::create_dir_all(&decode_dir).map_err(|err| {
        format!(
            "failed to create transcode directory {}: {err}",
            decode_dir.display()
        )
    })?;
    for fixture in &fixtures {
        let output = decode_dir.join(format!(
            "{}-to-ppm.ppm",
            fixture.format.name().to_ascii_lowercase()
        ));
        let input = prefixed_path(fixture.format, &fixture.path)?;
        let output_arg = prefixed_path(Format::Ppm, &output)?;
        run_self_test_command(
            &binary,
            &format!("transcode {} to PPM", fixture.format.name()),
            &[input, output_arg],
        )?;
        let stdout = run_self_test_command(
            &binary,
            &format!("identify {} decoded PPM", fixture.format.name()),
            &["identify".to_string(), prefixed_path(Format::Ppm, &output)?],
        )?;
        require_stdout_contains(&stdout, "format=PPM width=2 height=1")?;
    }
    let ppm = fixtures
        .iter()
        .find(|fixture| fixture.format == Format::Ppm)
        .ok_or_else(|| "missing PPM self-test fixture".to_string())?;
    let encode_dir = work_dir.join("encode-transcodes");
    fs::create_dir_all(&encode_dir).map_err(|err| {
        format!(
            "failed to create encode directory {}: {err}",
            encode_dir.display()
        )
    })?;
    for fixture in &fixtures {
        let output = encode_dir.join(format!(
            "ppm-to-{}.{}",
            fixture.format.name().to_ascii_lowercase(),
            format_extension(fixture.format)
        ));
        run_self_test_command(
            &binary,
            &format!("transcode PPM to {}", fixture.format.name()),
            &[
                prefixed_path(Format::Ppm, &ppm.path)?,
                prefixed_path(fixture.format, &output)?,
            ],
        )?;
        let stdout = run_self_test_command(
            &binary,
            &format!("identify encoded {}", fixture.format.name()),
            &[
                "identify".to_string(),
                prefixed_path(fixture.format, &output)?,
            ],
        )?;
        require_exact_stdout(&stdout, fixture.expected_identify)?;
    }
    println!("self-test: transcode ok");

    let resize_dir = work_dir.join("resize");
    fs::create_dir_all(&resize_dir).map_err(|err| {
        format!(
            "failed to create resize directory {}: {err}",
            resize_dir.display()
        )
    })?;
    for fixture in &fixtures {
        let output = resize_dir.join(format!(
            "{}.{}",
            fixture.format.name().to_ascii_lowercase(),
            format_extension(fixture.format)
        ));
        run_self_test_command(
            &binary,
            &format!("resize {}", fixture.format.name()),
            &[
                "resize".to_string(),
                "3x2".to_string(),
                prefixed_path(fixture.format, &fixture.path)?,
                prefixed_path(fixture.format, &output)?,
            ],
        )?;
        let stdout = run_self_test_command(
            &binary,
            &format!("identify resized {}", fixture.format.name()),
            &[
                "identify".to_string(),
                prefixed_path(fixture.format, &output)?,
            ],
        )?;
        require_exact_stdout(&stdout, &resized_expected(fixture.expected_identify, 3, 2))?;
    }
    println!("self-test: resize ok");

    let fit_dir = work_dir.join("resize-fit");
    fs::create_dir_all(&fit_dir).map_err(|err| {
        format!(
            "failed to create resize-fit directory {}: {err}",
            fit_dir.display()
        )
    })?;
    for fixture in &fixtures {
        let output = fit_dir.join(format!(
            "{}.{}",
            fixture.format.name().to_ascii_lowercase(),
            format_extension(fixture.format)
        ));
        run_self_test_command(
            &binary,
            &format!("resize-fit {}", fixture.format.name()),
            &[
                "resize-fit".to_string(),
                "5x5".to_string(),
                prefixed_path(fixture.format, &fixture.path)?,
                prefixed_path(fixture.format, &output)?,
            ],
        )?;
        let stdout = run_self_test_command(
            &binary,
            &format!("identify resize-fit {}", fixture.format.name()),
            &[
                "identify".to_string(),
                prefixed_path(fixture.format, &output)?,
            ],
        )?;
        require_exact_stdout(&stdout, &resized_expected(fixture.expected_identify, 5, 3))?;
    }
    println!("self-test: resize-fit ok");

    let pgm = fixtures
        .iter()
        .find(|fixture| fixture.format == Format::Pgm)
        .ok_or_else(|| "missing PGM self-test fixture".to_string())?;
    let batch_inputs = work_dir.join("batch-inputs");
    fs::create_dir_all(&batch_inputs).map_err(|err| {
        format!(
            "failed to create batch input directory {}: {err}",
            batch_inputs.display()
        )
    })?;
    let batch_ppm = batch_inputs.join("batch-rgb.ppm");
    let batch_pgm = batch_inputs.join("batch-gray.pgm");
    fs::copy(&ppm.path, &batch_ppm)
        .map_err(|err| format!("failed to prepare batch PPM fixture: {err}"))?;
    fs::copy(&pgm.path, &batch_pgm)
        .map_err(|err| format!("failed to prepare batch PGM fixture: {err}"))?;
    for fixture in &fixtures {
        let output_dir = work_dir.join(format!(
            "batch-{}",
            fixture.format.name().to_ascii_lowercase()
        ));
        fs::create_dir_all(&output_dir).map_err(|err| {
            format!(
                "failed to create batch output directory {}: {err}",
                output_dir.display()
            )
        })?;
        run_self_test_command(
            &binary,
            &format!("batch-convert to {}", fixture.format.name()),
            &[
                "batch-convert".to_string(),
                "--to".to_string(),
                fixture.format.name().to_string(),
                "--output-dir".to_string(),
                path_string(&output_dir)?,
                "--resize-fit".to_string(),
                "5x5".to_string(),
                prefixed_path(Format::Ppm, &batch_ppm)?,
                prefixed_path(Format::Pgm, &batch_pgm)?,
            ],
        )?;
        for stem in ["batch-rgb", "batch-gray"] {
            let output = output_dir.join(format!("{stem}.{}", format_extension(fixture.format)));
            let stdout = run_self_test_command(
                &binary,
                &format!("identify batch {} {stem}", fixture.format.name()),
                &[
                    "identify".to_string(),
                    prefixed_path(fixture.format, &output)?,
                ],
            )?;
            require_stdout_contains(
                &stdout,
                &format!("format={} width=5 height=3", fixture.format.name()),
            )?;
        }
    }
    println!("self-test: batch-convert ok");

    Ok(())
}

fn write_self_test_fixtures(output_dir: &Path) -> Result<Vec<SelfTestFixture>, String> {
    let rgb = imx_core::Image::new(
        2,
        1,
        imx_core::PixelFormat::Rgb8,
        vec![255, 0, 0, 0, 0, 255],
    )
    .map_err(|err| format!("failed to build RGB fixture: {err}"))?;
    let bilevel = imx_core::Image::new(2, 1, imx_core::PixelFormat::Bilevel, vec![0, 255])
        .map_err(|err| format!("failed to build PBM fixture: {err}"))?;
    let gray = imx_core::Image::new(2, 1, imx_core::PixelFormat::Gray8, vec![0, 255])
        .map_err(|err| format!("failed to build PGM fixture: {err}"))?;

    let fixtures = [
        (
            Format::Bmp,
            "input.bmp",
            imx_codec_bmp::encode(&rgb),
            "format=BMP width=2 height=1 channels=RGB depth=8",
        ),
        (
            Format::Farbfeld,
            "input.ff",
            imx_codec_farbfeld::encode(&rgb),
            "format=FARBFELD width=2 height=1 channels=RGBA depth=16",
        ),
        (
            Format::Jpeg,
            "input.jpg",
            imx_codec_jpeg::encode(&rgb),
            "format=JPEG width=2 height=1 channels=RGB depth=8",
        ),
        (
            Format::Qoi,
            "input.qoi",
            imx_codec_qoi::encode_image(&rgb, imx_codec_qoi::QOI_SRGB),
            "format=QOI width=2 height=1 channels=RGBA depth=8",
        ),
        (
            Format::Pbm,
            "input.pbm",
            imx_codec_pnm::encode_pbm(&bilevel),
            "format=PBM width=2 height=1 channels=GRAY depth=1",
        ),
        (
            Format::Pgm,
            "input.pgm",
            imx_codec_pnm::encode_pgm(&gray),
            "format=PGM width=2 height=1 channels=GRAY depth=8",
        ),
        (
            Format::Png,
            "input.png",
            imx_codec_png::encode(&rgb),
            "format=PNG width=2 height=1 channels=RGB depth=8",
        ),
        (
            Format::Ppm,
            "input.ppm",
            imx_codec_pnm::encode_ppm(&rgb),
            "format=PPM width=2 height=1 channels=RGB depth=8",
        ),
    ];

    let mut written = Vec::new();
    for (format, name, bytes, expected_identify) in fixtures {
        let path = output_dir.join(name);
        let bytes = bytes.map_err(|err| {
            format!(
                "failed to encode {} self-test fixture {name}: {err}",
                format.name()
            )
        })?;
        fs::write(&path, bytes).map_err(|err| {
            format!(
                "failed to write {} self-test fixture {}: {err}",
                format.name(),
                path.display()
            )
        })?;
        written.push(SelfTestFixture {
            format,
            path,
            expected_identify,
        });
    }
    Ok(written)
}

fn run_self_test_command(binary: &Path, label: &str, args: &[String]) -> Result<String, String> {
    let output = Command::new(binary)
        .args(args)
        .output()
        .map_err(|err| format!("{label} could not start: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "{label} exited with status {}; stderr: {}",
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_string()),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn require_exact_stdout(actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected stdout {expected:?}, got {actual:?}"))
}

fn require_stdout_contains(actual: &str, expected: &str) -> Result<(), String> {
    if actual.contains(expected) {
        return Ok(());
    }
    Err(format!(
        "expected stdout to contain {expected:?}, got {actual:?}"
    ))
}

fn resized_expected(expected_identify: &str, width: u32, height: u32) -> String {
    expected_identify
        .replace("width=2", &format!("width={width}"))
        .replace("height=1", &format!("height={height}"))
}

fn expected_identify_json(expected_identify: &str) -> Result<String, String> {
    let (format, width, height, channels, depth) = parse_expected_identify(expected_identify)?;
    Ok(format!(
        "{{\"schema_version\":1,\"format\":\"{format}\",\"width\":{width},\"height\":{height},\"channels\":\"{channels}\",\"depth\":{depth}}}"
    ))
}

fn expected_report_json(expected_identify: &str) -> Result<String, String> {
    let (format, width, height, channels, depth) = parse_expected_identify(expected_identify)?;
    Ok(format!(
        "{{\"schema_version\":1,\"status\":\"supported\",\"diagnostic_code\":null,\"format\":\"{format}\",\"width\":{width},\"height\":{height},\"channels\":\"{channels}\",\"depth\":{depth}}}"
    ))
}

fn parse_expected_identify(
    expected_identify: &str,
) -> Result<(&str, &str, &str, &str, &str), String> {
    let mut format = None;
    let mut width = None;
    let mut height = None;
    let mut channels = None;
    let mut depth = None;
    for field in expected_identify.split_whitespace() {
        if let Some(value) = field.strip_prefix("format=") {
            format = Some(value);
        } else if let Some(value) = field.strip_prefix("width=") {
            width = Some(value);
        } else if let Some(value) = field.strip_prefix("height=") {
            height = Some(value);
        } else if let Some(value) = field.strip_prefix("channels=") {
            channels = Some(value);
        } else if let Some(value) = field.strip_prefix("depth=") {
            depth = Some(value);
        }
    }
    match (format, width, height, channels, depth) {
        (Some(format), Some(width), Some(height), Some(channels), Some(depth)) => {
            Ok((format, width, height, channels, depth))
        }
        _ => Err(format!(
            "failed to parse expected identify line: {expected_identify}"
        )),
    }
}

fn prefixed_path(format: Format, path: &Path) -> Result<String, String> {
    Ok(format!("{}:{}", format.name(), path_string(path)?))
}

fn path_string(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))
}

fn identify(input_path: &str) -> ! {
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let input = read(input_path.path);
    let format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let info = identify_bytes(format, &input)
        .unwrap_or_else(|err| fail_image_operation(format, "identify", "input", &input_path, err));
    println!("{}", info.stable_line());
    process::exit(0);
}

fn identify_json(input_path: &str) -> ! {
    match try_identify(input_path) {
        Ok(info) => {
            println!("{}", identify_json_object(info));
            process::exit(0);
        }
        Err(err) => {
            eprintln!("{}", diagnostic_json_object(&err));
            process::exit(1);
        }
    }
}

fn report_json(input_path: &str) -> ! {
    match try_identify(input_path) {
        Ok(info) => println!("{}", report_supported_json_object(info)),
        Err(err) => println!("{}", report_unsupported_json_object(&err)),
    }
    process::exit(0);
}

fn identify_bytes(format: Format, input: &[u8]) -> Result<Identify, ImageError> {
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

fn try_identify(input_path: &str) -> Result<Identify, Diagnostic> {
    let input_path = parse_cli_path_diagnostic(input_path)?;
    let input = read_diagnostic(input_path.path)?;
    let format = detect_input_format_diagnostic(&input_path, &input)?;
    identify_bytes(format, &input).map_err(|err| {
        Diagnostic::new(
            image_diagnostic_code(format, "identify", &err),
            format!("failed to identify {} input: {err}", format.name()),
        )
    })
}

fn transcode(input_path: &str, output_path: &str) -> ! {
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let output_path = parse_cli_path(output_path).unwrap_or_else(|err| fail(err));
    reject_same_path(input_path.path, output_path.path);
    let input = read(input_path.path);
    let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));

    let image = decode_image(input_format, &input).unwrap_or_else(|err| {
        fail_image_operation(input_format, "decode", "input", &input_path, err)
    });
    let output = encode_image(output_format, &image).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_atomic(output_path.path, &output);
    process::exit(0);
}

fn resize(dimensions: &str, input_path: &str, output_path: &str) -> ! {
    let dimensions = parse_resize_dimensions(dimensions).unwrap_or_else(|err| fail(err));
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let output_path = parse_cli_path(output_path).unwrap_or_else(|err| fail(err));
    reject_same_path(input_path.path, output_path.path);
    let input = read(input_path.path);
    let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));

    let image = decode_image(input_format, &input).unwrap_or_else(|err| {
        fail_image_operation(input_format, "decode", "input", &input_path, err)
    });
    let image = image
        .resize_nearest(dimensions.width, dimensions.height)
        .unwrap_or_else(|err| {
            fail_image_operation(input_format, "resize", "input", &input_path, err)
        });
    let output = encode_image(output_format, &image).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_atomic(output_path.path, &output);
    process::exit(0);
}

fn resize_fit(dimensions: &str, input_path: &str, output_path: &str) -> ! {
    let dimensions = parse_resize_dimensions(dimensions).unwrap_or_else(|err| fail(err));
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let output_path = parse_cli_path(output_path).unwrap_or_else(|err| fail(err));
    reject_same_path(input_path.path, output_path.path);
    let input = read(input_path.path);
    let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));

    let image = decode_image(input_format, &input).unwrap_or_else(|err| {
        fail_image_operation(input_format, "decode", "input", &input_path, err)
    });
    let image = image
        .resize_nearest_fit(dimensions.width, dimensions.height)
        .unwrap_or_else(|err| {
            fail_image_operation(input_format, "resize-fit", "input", &input_path, err)
        });
    let output = encode_image(output_format, &image).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_atomic(output_path.path, &output);
    process::exit(0);
}

fn crop(geometry: &str, input_path: &str, output_path: &str) -> ! {
    let geometry = parse_crop_geometry(geometry).unwrap_or_else(|err| fail_usage(err));
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let output_path = parse_cli_path(output_path).unwrap_or_else(|err| fail(err));
    reject_same_path(input_path.path, output_path.path);
    let input = read(input_path.path);
    let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));

    let image = decode_image(input_format, &input).unwrap_or_else(|err| {
        fail_image_operation(input_format, "decode", "input", &input_path, err)
    });
    let image = image
        .crop(geometry.x, geometry.y, geometry.width, geometry.height)
        .unwrap_or_else(|err| {
            fail_image_operation(input_format, "crop", "input", &input_path, err)
        });
    let output = encode_image(output_format, &image).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_atomic(output_path.path, &output);
    process::exit(0);
}

fn rotate(angle: &str, input_path: &str, output_path: &str) -> ! {
    let angle = parse_rotate_angle(angle).unwrap_or_else(|err| fail_usage(err));
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let output_path = parse_cli_path(output_path).unwrap_or_else(|err| fail(err));
    reject_same_path(input_path.path, output_path.path);
    let input = read(input_path.path);
    let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));

    let image = decode_image(input_format, &input).unwrap_or_else(|err| {
        fail_image_operation(input_format, "decode", "input", &input_path, err)
    });
    let image = match angle {
        RotateAngle::Ninety => image.rotate_90(),
        RotateAngle::OneEighty => image.rotate_180(),
        RotateAngle::TwoSeventy => image.rotate_270(),
    }
    .unwrap_or_else(|err| fail_image_operation(input_format, "rotate", "input", &input_path, err));
    let output = encode_image(output_format, &image).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_atomic(output_path.path, &output);
    process::exit(0);
}

fn flip(input_path: &str, output_path: &str) -> ! {
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let output_path = parse_cli_path(output_path).unwrap_or_else(|err| fail(err));
    reject_same_path(input_path.path, output_path.path);
    let input = read(input_path.path);
    let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));

    let image = decode_image(input_format, &input).unwrap_or_else(|err| {
        fail_image_operation(input_format, "decode", "input", &input_path, err)
    });
    let image = image.flip_vertical().unwrap_or_else(|err| {
        fail_image_operation(input_format, "flip", "input", &input_path, err)
    });
    let output = encode_image(output_format, &image).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_atomic(output_path.path, &output);
    process::exit(0);
}

fn flop(input_path: &str, output_path: &str) -> ! {
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let output_path = parse_cli_path(output_path).unwrap_or_else(|err| fail(err));
    reject_same_path(input_path.path, output_path.path);
    let input = read(input_path.path);
    let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));

    let image = decode_image(input_format, &input).unwrap_or_else(|err| {
        fail_image_operation(input_format, "decode", "input", &input_path, err)
    });
    let image = image.flop_horizontal().unwrap_or_else(|err| {
        fail_image_operation(input_format, "flop", "input", &input_path, err)
    });
    let output = encode_image(output_format, &image).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_atomic(output_path.path, &output);
    process::exit(0);
}

fn batch_convert(args: &[String]) -> ! {
    let options = parse_batch_options(args).unwrap_or_else(|err| fail(err));
    let output_dir = Path::new(options.output_dir);
    let output_dir = validate_output_dir(output_dir, options.output_dir);

    let mut planned_outputs = Vec::new();
    for input in &options.inputs {
        let input_path = parse_cli_path(input).unwrap_or_else(|err| fail(err));
        validate_batch_input(&input_path);
        let output_path = batch_output_path(output_dir, &input_path, options.output_format)
            .unwrap_or_else(|err| fail(err));
        validate_batch_output(&input_path, &output_path);
        let output_key = output_path.to_string_lossy().to_ascii_lowercase();
        if planned_outputs.iter().any(|planned: &BatchPlan| {
            planned.output_path == output_path
                || planned.output_path.to_string_lossy().to_ascii_lowercase() == output_key
        }) {
            fail(format!("batch output collision: {}", output_path.display()));
        }
        planned_outputs.push(BatchPlan {
            input_path,
            output_path,
        });
    }

    let mut encoded_outputs = Vec::with_capacity(planned_outputs.len());
    for plan in &planned_outputs {
        let input = read(plan.input_path.path);
        let input_format =
            detect_input_format(&plan.input_path, &input).unwrap_or_else(|err| fail(err));
        let output_path_string = plan.output_path.to_string_lossy().into_owned();
        let output_path = CliPath {
            original: &output_path_string,
            path: &output_path_string,
            prefix: None,
        };
        let image = decode_image(input_format, &input).unwrap_or_else(|err| {
            fail_image_operation(input_format, "decode", "input", &plan.input_path, err)
        });
        let image = match options.transform {
            Some(BatchTransform::Resize(dimensions)) => image
                .resize_nearest(dimensions.width, dimensions.height)
                .unwrap_or_else(|err| {
                    fail_image_operation(input_format, "resize", "input", &plan.input_path, err)
                }),
            Some(BatchTransform::ResizeFit(dimensions)) => image
                .resize_nearest_fit(dimensions.width, dimensions.height)
                .unwrap_or_else(|err| {
                    fail_image_operation(input_format, "resize-fit", "input", &plan.input_path, err)
                }),
            None => image,
        };
        let output = encode_image(options.output_format, &image).unwrap_or_else(|err| {
            fail_image_operation(options.output_format, "encode", "output", &output_path, err)
        });
        encoded_outputs.push((output_path_string, output));
    }

    for (output_path, output) in encoded_outputs {
        write_atomic_new(&output_path, &output);
    }

    process::exit(0);
}

fn decode_image(format: Format, input: &[u8]) -> Result<imx_core::Image, ImageError> {
    match format {
        Format::Bmp => imx_codec_bmp::decode(input),
        Format::Farbfeld => imx_codec_farbfeld::decode(input),
        Format::Jpeg => imx_codec_jpeg::decode(input),
        Format::Pbm => imx_codec_pnm::decode_pbm(input),
        Format::Pgm => imx_codec_pnm::decode_pgm(input),
        Format::Png => imx_codec_png::decode(input),
        Format::Ppm => imx_codec_pnm::decode_ppm(input),
        Format::Qoi => imx_codec_qoi::decode(input).and_then(|decoded| decoded.into_core_image()),
    }
}

fn encode_image(format: Format, image: &imx_core::Image) -> Result<Vec<u8>, ImageError> {
    match format {
        Format::Bmp => imx_codec_bmp::encode(image),
        Format::Farbfeld => imx_codec_farbfeld::encode(image),
        Format::Jpeg => imx_codec_jpeg::encode(image),
        Format::Pbm => imx_codec_pnm::encode_pbm(image),
        Format::Pgm => imx_codec_pnm::encode_pgm(image),
        Format::Png => imx_codec_png::encode(image),
        Format::Ppm => imx_codec_pnm::encode_ppm(image),
        Format::Qoi => imx_codec_qoi::encode_image(image, imx_codec_qoi::QOI_SRGB),
    }
}

fn read(path: &str) -> Vec<u8> {
    let mut file = fs::File::open(path).unwrap_or_else(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            fail(format!("missing input: {path}"));
        }
        fail(format!("failed to read {path}: {err}"));
    });
    if let Ok(metadata) = file.metadata() {
        if metadata.len() > MAX_INPUT_BYTES {
            fail(format!(
                "input file too large: {} bytes exceeds {} byte limit for {path}",
                metadata.len(),
                MAX_INPUT_BYTES
            ));
        }
    }
    let mut input = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut input)
        .unwrap_or_else(|err| fail(format!("failed to read {path}: {err}")));
    if input.len() as u64 > MAX_INPUT_BYTES {
        fail(format!(
            "input file too large: {} bytes exceeds {} byte limit for {path}",
            input.len(),
            MAX_INPUT_BYTES
        ));
    }
    input
}

#[derive(Debug)]
struct Diagnostic {
    code: &'static str,
    message: String,
}

impl Diagnostic {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

fn read_diagnostic(path: &str) -> Result<Vec<u8>, Diagnostic> {
    let mut file = fs::File::open(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            Diagnostic::new("input.missing", format!("missing input: {path}"))
        } else {
            Diagnostic::new("input.read_failed", format!("failed to read {path}: {err}"))
        }
    })?;
    if let Ok(metadata) = file.metadata() {
        if metadata.len() > MAX_INPUT_BYTES {
            return Err(Diagnostic::new(
                "input.too_large",
                format!(
                    "input file too large: {} bytes exceeds {} byte limit for {path}",
                    metadata.len(),
                    MAX_INPUT_BYTES
                ),
            ));
        }
    }
    let mut input = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut input)
        .map_err(|err| {
            Diagnostic::new("input.read_failed", format!("failed to read {path}: {err}"))
        })?;
    if input.len() as u64 > MAX_INPUT_BYTES {
        return Err(Diagnostic::new(
            "input.too_large",
            format!(
                "input file too large: {} bytes exceeds {} byte limit for {path}",
                input.len(),
                MAX_INPUT_BYTES
            ),
        ));
    }
    Ok(input)
}

fn identify_json_object(info: Identify) -> String {
    format!(
        "{{\"schema_version\":1,\"format\":\"{}\",\"width\":{},\"height\":{},\"channels\":\"{}\",\"depth\":{}}}",
        info.format.name(),
        info.width,
        info.height,
        info.pixel_format.channels(),
        info.pixel_format.depth()
    )
}

fn report_supported_json_object(info: Identify) -> String {
    format!(
        "{{\"schema_version\":1,\"status\":\"supported\",\"diagnostic_code\":null,\"format\":\"{}\",\"width\":{},\"height\":{},\"channels\":\"{}\",\"depth\":{}}}",
        info.format.name(),
        info.width,
        info.height,
        info.pixel_format.channels(),
        info.pixel_format.depth()
    )
}

fn report_unsupported_json_object(diagnostic: &Diagnostic) -> String {
    format!(
        "{{\"schema_version\":1,\"status\":\"unsupported\",\"diagnostic_code\":\"{}\",\"message\":{}}}",
        diagnostic.code,
        json_string(&diagnostic.message)
    )
}

fn diagnostic_json_object(diagnostic: &Diagnostic) -> String {
    format!(
        "{{\"schema_version\":1,\"status\":\"unsupported\",\"diagnostic_code\":\"{}\",\"message\":{}}}",
        diagnostic.code,
        json_string(&diagnostic.message)
    )
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn reject_same_path(input_path: &str, output_path: &str) {
    if let (Ok(input), Ok(output)) = (fs::canonicalize(input_path), fs::canonicalize(output_path)) {
        if input == output {
            fail(format!(
                "input and output paths must be different: {input_path} and {output_path}"
            ));
        }
    }
}

fn write_atomic(output_path: &str, bytes: &[u8]) {
    let output = Path::new(output_path);
    let directory = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let Some(file_name) = output.file_name().and_then(|name| name.to_str()) else {
        fail(format!("invalid output path: {output_path}"));
    };
    let process_id = process::id();
    for attempt in 0..100 {
        let temp_path = directory.join(format!(".{file_name}.imx-{process_id}-{attempt}.tmp"));
        let mut temp = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(temp) => temp,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => fail(format!("failed to write {output_path}: {err}")),
        };
        if let Err(err) = temp.write_all(bytes) {
            let _ = fs::remove_file(&temp_path);
            fail(format!("failed to write {output_path}: {err}"));
        }
        if let Err(err) = temp.flush() {
            let _ = fs::remove_file(&temp_path);
            fail(format!("failed to write {output_path}: {err}"));
        }
        drop(temp);
        if let Err(err) = fs::rename(&temp_path, output) {
            let _ = fs::remove_file(&temp_path);
            fail(format!("failed to write {output_path}: {err}"));
        }
        return;
    }
    fail(format!(
        "failed to write {output_path}: could not create temporary file"
    ));
}

fn write_atomic_new(output_path: &str, bytes: &[u8]) {
    let output = Path::new(output_path);
    let directory = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let Some(file_name) = output.file_name().and_then(|name| name.to_str()) else {
        fail(format!("invalid output path: {output_path}"));
    };
    let process_id = process::id();
    for attempt in 0..100 {
        let temp_path = directory.join(format!(".{file_name}.imx-{process_id}-{attempt}.tmp"));
        let mut temp = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(temp) => temp,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => fail(format!("failed to write {output_path}: {err}")),
        };
        if let Err(err) = temp.write_all(bytes) {
            let _ = fs::remove_file(&temp_path);
            fail(format!("failed to write {output_path}: {err}"));
        }
        if let Err(err) = temp.flush() {
            let _ = fs::remove_file(&temp_path);
            fail(format!("failed to write {output_path}: {err}"));
        }
        drop(temp);
        if let Err(err) = fs::hard_link(&temp_path, output) {
            let _ = fs::remove_file(&temp_path);
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                fail(format!("output path already exists: {output_path}"));
            }
            fail(format!("failed to write {output_path}: {err}"));
        }
        let _ = fs::remove_file(&temp_path);
        return;
    }
    fail(format!(
        "failed to write {output_path}: could not create temporary file"
    ));
}

#[derive(Debug, Clone, Copy)]
struct CliPath<'a> {
    original: &'a str,
    path: &'a str,
    prefix: Option<Format>,
}

#[derive(Debug, Clone, Copy)]
enum BatchTransform {
    Resize(ResizeDimensions),
    ResizeFit(ResizeDimensions),
}

#[derive(Debug)]
struct BatchOptions<'a> {
    output_format: Format,
    output_dir: &'a str,
    transform: Option<BatchTransform>,
    inputs: Vec<&'a str>,
}

#[derive(Debug)]
struct BatchPlan<'a> {
    input_path: CliPath<'a>,
    output_path: PathBuf,
}

fn parse_cli_path(value: &str) -> Result<CliPath<'_>, String> {
    if let Some((prefix, path)) = value.split_once(':') {
        if !prefix.is_empty() && prefix.bytes().all(|byte| byte.is_ascii_uppercase()) {
            let Some(format) = parse_format_prefix(prefix) else {
                return Err(format!("unsupported format prefix: {prefix}"));
            };
            if path.is_empty() {
                return Err(format!("missing path after format prefix {prefix}:"));
            }
            return Ok(CliPath {
                original: value,
                path,
                prefix: Some(format),
            });
        }
    }

    Ok(CliPath {
        original: value,
        path: value,
        prefix: None,
    })
}

fn parse_cli_path_diagnostic(value: &str) -> Result<CliPath<'_>, Diagnostic> {
    if let Some((prefix, path)) = value.split_once(':') {
        if !prefix.is_empty() && prefix.bytes().all(|byte| byte.is_ascii_uppercase()) {
            let Some(format) = parse_format_prefix(prefix) else {
                return Err(Diagnostic::new(
                    "input.unsupported_format_prefix",
                    format!("unsupported format prefix: {prefix}"),
                ));
            };
            if path.is_empty() {
                return Err(Diagnostic::new(
                    "input.missing_prefix_path",
                    format!("missing path after format prefix {prefix}:"),
                ));
            }
            return Ok(CliPath {
                original: value,
                path,
                prefix: Some(format),
            });
        }
    }

    Ok(CliPath {
        original: value,
        path: value,
        prefix: None,
    })
}

fn parse_format_prefix(prefix: &str) -> Option<Format> {
    match prefix {
        "BMP" => Some(Format::Bmp),
        "FARBFELD" => Some(Format::Farbfeld),
        "JPEG" => Some(Format::Jpeg),
        "PBM" => Some(Format::Pbm),
        "PGM" => Some(Format::Pgm),
        "PNG" => Some(Format::Png),
        "PPM" => Some(Format::Ppm),
        "QOI" => Some(Format::Qoi),
        _ => None,
    }
}

fn parse_batch_options(args: &[String]) -> Result<BatchOptions<'_>, String> {
    let mut output_format = None;
    let mut output_dir = None;
    let mut transform = None;
    let mut index = 0;

    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--to" => {
                if output_format.is_some() {
                    return Err("batch-convert --to may only be supplied once".to_string());
                }
                let Some(value) = args.get(index + 1) else {
                    return Err("batch-convert --to requires a format".to_string());
                };
                if value.starts_with("--") {
                    return Err("batch-convert --to requires a format".to_string());
                }
                output_format = Some(parse_batch_output_format(value)?);
                index += 2;
            }
            "--output-dir" => {
                if output_dir.is_some() {
                    return Err("batch-convert --output-dir may only be supplied once".to_string());
                }
                let Some(value) = args.get(index + 1) else {
                    return Err("batch-convert --output-dir requires a directory".to_string());
                };
                if value.starts_with("--") {
                    return Err("batch-convert --output-dir requires a directory".to_string());
                }
                output_dir = Some(value.as_str());
                index += 2;
            }
            "--resize" | "--resize-fit" => {
                if transform.is_some() {
                    return Err(
                        "batch-convert accepts only one of --resize or --resize-fit".to_string()
                    );
                }
                let Some(value) = args.get(index + 1) else {
                    return Err(format!("batch-convert {arg} requires dimensions"));
                };
                if value.starts_with("--") {
                    return Err(format!("batch-convert {arg} requires dimensions"));
                }
                let dimensions = parse_resize_dimensions(value)?;
                transform = Some(if arg == "--resize" {
                    BatchTransform::Resize(dimensions)
                } else {
                    BatchTransform::ResizeFit(dimensions)
                });
                index += 2;
            }
            option if option.starts_with("--") => {
                return Err(format!("unsupported batch-convert option: {option}"));
            }
            _ => break,
        }
    }

    let output_format =
        output_format.ok_or_else(|| "batch-convert requires --to <FORMAT>".to_string())?;
    let output_dir =
        output_dir.ok_or_else(|| "batch-convert requires --output-dir <dir>".to_string())?;
    let inputs = args[index..].iter().map(String::as_str).collect::<Vec<_>>();
    if inputs.is_empty() {
        return Err("batch-convert requires at least one input".to_string());
    }

    Ok(BatchOptions {
        output_format,
        output_dir,
        transform,
        inputs,
    })
}

fn parse_batch_output_format(value: &str) -> Result<Format, String> {
    parse_format_prefix(value).ok_or_else(|| format!("unsupported output format: {value}"))
}

fn validate_output_dir<'a>(path: &'a Path, original: &str) -> &'a Path {
    let metadata = fs::metadata(path).unwrap_or_else(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            fail(format!("missing output directory: {original}"));
        }
        fail(format!(
            "failed to inspect output directory {original}: {err}"
        ));
    });
    if !metadata.is_dir() {
        fail(format!("output directory is not a directory: {original}"));
    }
    path
}

fn validate_batch_input(input_path: &CliPath<'_>) {
    if input_path.path == "-" {
        fail("stdin/stdout is not supported");
    }
    let metadata = match fs::metadata(input_path.path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            fail(format!("missing input: {}", input_path.original));
        }
        Err(err) => fail(format!(
            "failed to inspect input {}: {err}",
            input_path.original
        )),
    };
    if !metadata.is_file() {
        fail(format!("input is not a file: {}", input_path.original));
    }
    if metadata.len() > MAX_INPUT_BYTES {
        fail(format!(
            "input file too large: {} bytes exceeds {} byte limit for {}",
            metadata.len(),
            MAX_INPUT_BYTES,
            input_path.original
        ));
    }
}

fn batch_output_path(
    output_dir: &Path,
    input_path: &CliPath<'_>,
    output_format: Format,
) -> Result<PathBuf, String> {
    let Some(stem) = Path::new(input_path.path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
    else {
        return Err(format!(
            "input path does not have a usable file name: {}",
            input_path.original
        ));
    };
    Ok(output_dir.join(format!("{stem}.{}", format_extension(output_format))))
}

fn validate_batch_output(input_path: &CliPath<'_>, output_path: &Path) {
    if let (Ok(input), Ok(output)) = (
        fs::canonicalize(input_path.path),
        fs::canonicalize(output_path),
    ) {
        if input == output {
            fail("input and output paths must be different");
        }
    }

    match fs::metadata(output_path) {
        Ok(_) => fail(format!(
            "output path already exists: {}",
            output_path.display()
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => fail(format!(
            "failed to inspect output {}: {err}",
            output_path.display()
        )),
    }
}

fn format_extension(format: Format) -> &'static str {
    match format {
        Format::Bmp => "bmp",
        Format::Farbfeld => "ff",
        Format::Jpeg => "jpg",
        Format::Pbm => "pbm",
        Format::Pgm => "pgm",
        Format::Png => "png",
        Format::Ppm => "ppm",
        Format::Qoi => "qoi",
    }
}

#[derive(Debug, Clone, Copy)]
struct ResizeDimensions {
    width: u32,
    height: u32,
}

fn parse_resize_dimensions(value: &str) -> Result<ResizeDimensions, String> {
    let Some((width, height)) = value.split_once('x') else {
        return Err(format!(
            "invalid resize dimensions: {value}; expected <width>x<height>"
        ));
    };
    if width.is_empty()
        || height.is_empty()
        || !width.bytes().all(|byte| byte.is_ascii_digit())
        || !height.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!(
            "invalid resize dimensions: {value}; expected <width>x<height>"
        ));
    }
    let width = width
        .parse::<u32>()
        .map_err(|_| format!("invalid resize width: {width}"))?;
    let height = height
        .parse::<u32>()
        .map_err(|_| format!("invalid resize height: {height}"))?;
    if width == 0 || height == 0 {
        return Err("resize dimensions must be non-zero".to_string());
    }
    Ok(ResizeDimensions { width, height })
}

#[derive(Debug, Clone, Copy)]
struct CropGeometry {
    width: u32,
    height: u32,
    x: u32,
    y: u32,
}

fn parse_crop_geometry(value: &str) -> Result<CropGeometry, String> {
    let invalid = || format!("invalid crop geometry: {value}; expected <width>x<height>+<x>+<y>");
    let (size, offsets) = value.split_once('+').ok_or_else(invalid)?;
    let (x, y) = offsets.split_once('+').ok_or_else(invalid)?;
    let (width, height) = size.split_once('x').ok_or_else(invalid)?;
    let width = parse_geometry_u32(width).ok_or_else(invalid)?;
    let height = parse_geometry_u32(height).ok_or_else(invalid)?;
    let x = parse_geometry_u32(x).ok_or_else(invalid)?;
    let y = parse_geometry_u32(y).ok_or_else(invalid)?;
    if width == 0 || height == 0 {
        return Err("crop dimensions must be non-zero".to_string());
    }
    Ok(CropGeometry {
        width,
        height,
        x,
        y,
    })
}

fn parse_geometry_u32(value: &str) -> Option<u32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse::<u32>().ok()
}

#[derive(Debug, Clone, Copy)]
enum RotateAngle {
    Ninety,
    OneEighty,
    TwoSeventy,
}

fn parse_rotate_angle(value: &str) -> Result<RotateAngle, String> {
    match value {
        "90" => Ok(RotateAngle::Ninety),
        "180" => Ok(RotateAngle::OneEighty),
        "270" => Ok(RotateAngle::TwoSeventy),
        _ => Err(format!(
            "invalid rotation angle: {value}; expected 90, 180, or 270"
        )),
    }
}

fn detect_input_format(path: &CliPath<'_>, bytes: &[u8]) -> Result<Format, String> {
    let detected = detect_unprefixed_input_format(path.path, bytes)?;
    enforce_prefix(path, detected, "detected format")
}

fn detect_input_format_diagnostic(path: &CliPath<'_>, bytes: &[u8]) -> Result<Format, Diagnostic> {
    let detected = detect_unprefixed_input_format(path.path, bytes).map_err(|_| {
        Diagnostic::new(
            "input.unsupported_format",
            format!("unsupported format: {}", path.path),
        )
    })?;
    if let Some(prefix) = path.prefix {
        if prefix != detected {
            return Err(Diagnostic::new(
                "input.format_prefix_mismatch",
                format!(
                    "format prefix {} does not match detected format {}",
                    prefix.name(),
                    detected.name()
                ),
            ));
        }
    }
    Ok(detected)
}

fn detect_unprefixed_input_format(path: &str, bytes: &[u8]) -> Result<Format, String> {
    if bytes.len() >= imx_codec_farbfeld::MAGIC.len()
        && &bytes[..imx_codec_farbfeld::MAGIC.len()] == imx_codec_farbfeld::MAGIC
    {
        return Ok(Format::Farbfeld);
    }
    if bytes.len() >= imx_codec_bmp::MAGIC.len()
        && &bytes[..imx_codec_bmp::MAGIC.len()] == imx_codec_bmp::MAGIC
    {
        return Ok(Format::Bmp);
    }
    if bytes.len() >= imx_codec_qoi::MAGIC.len()
        && bytes[..imx_codec_qoi::MAGIC.len()].eq_ignore_ascii_case(imx_codec_qoi::MAGIC)
    {
        return Ok(Format::Qoi);
    }
    if bytes.len() >= imx_codec_jpeg::MAGIC.len()
        && &bytes[..imx_codec_jpeg::MAGIC.len()] == imx_codec_jpeg::MAGIC
    {
        return Ok(Format::Jpeg);
    }
    if bytes.len() >= imx_codec_png::MAGIC.len()
        && &bytes[..imx_codec_png::MAGIC.len()] == imx_codec_png::MAGIC
    {
        return Ok(Format::Png);
    }
    if bytes.len() >= imx_codec_pnm::P6_MAGIC.len()
        && (&bytes[..imx_codec_pnm::P6_MAGIC.len()] == imx_codec_pnm::P6_MAGIC
            || &bytes[..imx_codec_pnm::P3_MAGIC.len()] == imx_codec_pnm::P3_MAGIC)
    {
        return Ok(Format::Ppm);
    }
    if bytes.len() >= imx_codec_pnm::P4_MAGIC.len()
        && (&bytes[..imx_codec_pnm::P4_MAGIC.len()] == imx_codec_pnm::P4_MAGIC
            || &bytes[..imx_codec_pnm::P1_MAGIC.len()] == imx_codec_pnm::P1_MAGIC)
    {
        return Ok(Format::Pbm);
    }
    if bytes.len() >= imx_codec_pnm::P5_MAGIC.len()
        && (&bytes[..imx_codec_pnm::P5_MAGIC.len()] == imx_codec_pnm::P5_MAGIC
            || &bytes[..imx_codec_pnm::P2_MAGIC.len()] == imx_codec_pnm::P2_MAGIC)
    {
        return Ok(Format::Pgm);
    }
    detect_path_format(path)
}

fn image_diagnostic_code(format: Format, operation: &str, err: &ImageError) -> &'static str {
    match err {
        ImageError::UnsupportedFormat(_) => match (format, operation) {
            (Format::Bmp, _) => "bmp.unsupported_feature",
            (Format::Jpeg, "identify") => "jpeg.identify_failed",
            (Format::Jpeg, "decode") => "jpeg.decode_failed",
            (Format::Png, "identify") => "png.identify_failed",
            (Format::Png, "decode") => "png.decode_failed",
            _ => err.diagnostic_code(),
        },
        _ => err.diagnostic_code(),
    }
}

fn detect_output_format(path: &CliPath<'_>) -> Result<Format, String> {
    let detected = detect_path_format(path.path)?;
    enforce_prefix(path, detected, "path format")
}

fn detect_path_format(path: &str) -> Result<Format, String> {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("ff") | Some("farbfeld") => Ok(Format::Farbfeld),
        Some("bmp") => Ok(Format::Bmp),
        Some("jpg") | Some("jpeg") => Ok(Format::Jpeg),
        Some("pbm") => Ok(Format::Pbm),
        Some("pgm") => Ok(Format::Pgm),
        Some("png") => Ok(Format::Png),
        Some("ppm") => Ok(Format::Ppm),
        Some("qoi") => Ok(Format::Qoi),
        _ => Err(format!("unsupported format: {path}")),
    }
}

fn enforce_prefix(
    path: &CliPath<'_>,
    detected: Format,
    detected_source: &str,
) -> Result<Format, String> {
    if let Some(prefix) = path.prefix {
        if prefix != detected {
            return Err(format!(
                "format prefix {} does not match {detected_source} {} for {}",
                prefix.name(),
                detected.name(),
                path.original
            ));
        }
    }
    Ok(detected)
}
