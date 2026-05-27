use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use imx_core::{Format, ImageError, MAX_PIXEL_BYTES};

const MAX_INPUT_BYTES: u64 = MAX_PIXEL_BYTES as u64 + 1024 * 1024;

fn usage() -> ! {
    eprintln!(
        "usage:\n  imx --help\n  imx --version\n  imx identify [FORMAT:]<input.bmp|input.ff|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>\n  imx resize <width>x<height> [FORMAT:]<input> [FORMAT:]<output>\n  imx resize-fit <width>x<height> [FORMAT:]<input> [FORMAT:]<output>\n  imx batch-convert --to <FORMAT> --output-dir <dir> [--resize <width>x<height>|--resize-fit <width>x<height>] [FORMAT:]<input>...\n  imx [FORMAT:]<input> [FORMAT:]<output>\n\nsupported formats: bmp (.bmp), farbfeld (.ff, .farbfeld), jpeg (.jpg, .jpeg), qoi (.qoi), pbm (.pbm), pgm (.pgm), png (.png), ppm (.ppm)\nsupported prefixes: BMP:, FARBFELD:, JPEG:, QOI:, PBM:, PGM:, PNG:, PPM:"
    );
    process::exit(2);
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("error: {message}");
    process::exit(1);
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
                "IMX Developer Preview\n\nusage:\n  imx identify [FORMAT:]<input.bmp|input.ff|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>\n  imx resize <width>x<height> [FORMAT:]<input> [FORMAT:]<output>\n  imx resize-fit <width>x<height> [FORMAT:]<input> [FORMAT:]<output>\n  imx batch-convert --to <FORMAT> --output-dir <dir> [--resize <width>x<height>|--resize-fit <width>x<height>] [FORMAT:]<input>...\n  imx [FORMAT:]<input> [FORMAT:]<output>\n\nsupported transcodes: BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM, including deterministic same-format rewrites except lossy JPEG re-encoding\nsupported resize: nearest-neighbor exact dimensions and aspect-preserving fit for existing supported formats\nsupported batch conversion: explicit output format, existing output directory, shell-expanded input paths, no overwrite or collision renaming\nsupported prefixes: BMP:, FARBFELD:, JPEG:, QOI:, PBM:, PGM:, PNG:, PPM:\nunsupported: stdin/stdout, recursive directory walking, crop/rotate, delegates, color management, and formats beyond BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM"
            );
            process::exit(0);
        }
        [_, flag] if flag == "--version" || flag == "-V" || flag == "version" => {
            println!("imx {}", env!("CARGO_PKG_VERSION"));
            process::exit(0);
        }
        [_, command, input] if command == "identify" => identify(input),
        [_, command, dimensions, input, output] if command == "resize" => {
            resize(dimensions, input, output)
        }
        [_, command, dimensions, input, output] if command == "resize-fit" => {
            resize_fit(dimensions, input, output)
        }
        [_, command, rest @ ..] if command == "batch-convert" => batch_convert(rest),
        [_, input, output] => transcode(input, output),
        _ => usage(),
    }
}

fn identify(input_path: &str) -> ! {
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let input = read(input_path.path);
    let format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let info = match format {
        Format::Bmp => imx_codec_bmp::identify(&input),
        Format::Farbfeld => imx_codec_farbfeld::identify(&input),
        Format::Jpeg => imx_codec_jpeg::identify(&input),
        Format::Pbm => imx_codec_pnm::identify_pbm(&input),
        Format::Pgm => imx_codec_pnm::identify_pgm(&input),
        Format::Png => imx_codec_png::identify(&input),
        Format::Ppm => imx_codec_pnm::identify_ppm(&input),
        Format::Qoi => imx_codec_qoi::identify(&input),
    }
    .unwrap_or_else(|err| fail_image_operation(format, "identify", "input", &input_path, err));
    println!("{}", info.stable_line());
    process::exit(0);
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
    let mut file =
        fs::File::open(path).unwrap_or_else(|err| fail(format!("failed to read {path}: {err}")));
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

fn reject_same_path(input_path: &str, output_path: &str) {
    if let (Ok(input), Ok(output)) = (fs::canonicalize(input_path), fs::canonicalize(output_path)) {
        if input == output {
            fail("input and output paths must be different");
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
        fail(format!(
            "failed to inspect output directory {original}: {err}"
        ))
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

fn detect_input_format(path: &CliPath<'_>, bytes: &[u8]) -> Result<Format, String> {
    let detected = detect_unprefixed_input_format(path.path, bytes)?;
    enforce_prefix(path, detected, "detected format")
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
