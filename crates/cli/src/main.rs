use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process;

use imx_core::{Format, ImageError, MAX_PIXEL_BYTES};

const MAX_INPUT_BYTES: u64 = MAX_PIXEL_BYTES as u64 + 1024 * 1024;

fn usage() -> ! {
    eprintln!(
        "usage:\n  imx --help\n  imx --version\n  imx identify [FORMAT:]<input.ff|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>\n  imx [FORMAT:]<input> [FORMAT:]<output>\n\nsupported formats: farbfeld (.ff, .farbfeld), jpeg (.jpg, .jpeg), qoi (.qoi), pbm (.pbm), pgm (.pgm), png (.png), ppm (.ppm)\nsupported prefixes: FARBFELD:, JPEG:, QOI:, PBM:, PGM:, PNG:, PPM:"
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
                "IMX Developer Preview\n\nusage:\n  imx identify [FORMAT:]<input.ff|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>\n  imx [FORMAT:]<input> [FORMAT:]<output>\n\nsupported transcodes: FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM, including deterministic same-format rewrites except lossy JPEG re-encoding\nsupported prefixes: FARBFELD:, JPEG:, QOI:, PBM:, PGM:, PNG:, PPM:\nunsupported: stdin/stdout, transforms, delegates, and formats beyond FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM"
            );
            process::exit(0);
        }
        [_, flag] if flag == "--version" || flag == "-V" || flag == "version" => {
            println!("imx {}", env!("CARGO_PKG_VERSION"));
            process::exit(0);
        }
        [_, command, input] if command == "identify" => identify(input),
        [_, input, output] => transcode(input, output),
        _ => usage(),
    }
}

fn identify(input_path: &str) -> ! {
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let input = read(input_path.path);
    let format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let info = match format {
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

fn decode_image(format: Format, input: &[u8]) -> Result<imx_core::Image, ImageError> {
    match format {
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

#[derive(Debug, Clone, Copy)]
struct CliPath<'a> {
    original: &'a str,
    path: &'a str,
    prefix: Option<Format>,
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
