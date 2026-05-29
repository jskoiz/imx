use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use imx_core::{
    compare_rgba8, Format, Identify, ImageError, ResizeFilter, ResizeGeometry, MAX_PIXEL_BYTES,
};

mod completions;

const MAX_INPUT_BYTES: u64 = MAX_PIXEL_BYTES as u64 + 1024 * 1024;

/// Whether EXIF/TIFF Orientation is auto-applied on decode. On by default;
/// cleared by the global `--no-auto-orient` flag. Read by [`decode_image`] and
/// [`identify_bytes`] so every decode path honors the same setting.
static AUTO_ORIENT: AtomicBool = AtomicBool::new(true);

fn auto_orient() -> bool {
    AUTO_ORIENT.load(Ordering::Relaxed)
}

/// Whether the embedded ICC color profile is stripped before encoding. Off by
/// default; set by the global `--strip` flag. Consulted by [`encode_image`] and
/// [`encode_image_with_quality`] so every encode path drops the profile when
/// requested, regardless of which subcommand runs.
static STRIP_ICC: AtomicBool = AtomicBool::new(false);

fn strip_icc() -> bool {
    STRIP_ICC.load(Ordering::Relaxed)
}

/// Selected resampling filter for `resize`/`resize-fit`, encoded as the
/// discriminant in [`filter_from_code`]. Defaults to Lanczos3 (code 4) per the
/// product decision; `--filter point` selects the byte-exact nearest path.
/// Read by [`resize`], [`resize_fit`], and the resize pipeline ops.
static RESIZE_FILTER: AtomicU8 = AtomicU8::new(4);

fn filter_code(filter: ResizeFilter) -> u8 {
    match filter {
        ResizeFilter::Point => 0,
        ResizeFilter::Box => 1,
        ResizeFilter::Triangle => 2,
        ResizeFilter::CatmullRom => 3,
        ResizeFilter::Lanczos3 => 4,
    }
}

fn filter_from_code(code: u8) -> ResizeFilter {
    match code {
        0 => ResizeFilter::Point,
        1 => ResizeFilter::Box,
        2 => ResizeFilter::Triangle,
        3 => ResizeFilter::CatmullRom,
        _ => ResizeFilter::Lanczos3,
    }
}

fn resize_filter() -> ResizeFilter {
    filter_from_code(RESIZE_FILTER.load(Ordering::Relaxed))
}

/// Parse a `--filter` value into a [`ResizeFilter`].
fn parse_resize_filter(value: &str) -> Result<ResizeFilter, String> {
    match value {
        "point" => Ok(ResizeFilter::Point),
        "box" => Ok(ResizeFilter::Box),
        "triangle" => Ok(ResizeFilter::Triangle),
        "catmull-rom" => Ok(ResizeFilter::CatmullRom),
        "lanczos3" => Ok(ResizeFilter::Lanczos3),
        other => Err(format!(
            "invalid --filter value: {other}; expected one of point, box, triangle, catmull-rom, lanczos3"
        )),
    }
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  imx --help\n  imx --version\n  imx [--no-auto-orient] identify [--frame <N>] [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.gif|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm|input.tif|input.tiff|input.webp|FORMAT:->\n  imx [--no-auto-orient] identify --json [--frame <N>] [FORMAT:]<input|FORMAT:->\n  imx [--no-auto-orient] report --json [--frame <N>] [FORMAT:]<input|FORMAT:->\n  imx [--no-auto-orient] compare [--metric <ae|mae|psnr>] [FORMAT:]<a|FORMAT:-> [FORMAT:]<b>\n  imx [--no-auto-orient] [--strip] [--filter <point|box|triangle|catmull-rom|lanczos3>] resize <width>x<height>|<width>x|x<height>|<percent>% [FORMAT:]<input|FORMAT:-> [FORMAT:]<output|FORMAT:->\n  imx [--no-auto-orient] [--strip] [--filter <point|box|triangle|catmull-rom|lanczos3>] resize-fit <width>x<height> [FORMAT:]<input|FORMAT:-> [FORMAT:]<output|FORMAT:->\n  imx [--no-auto-orient] [--strip] crop <width>x<height>+<x>+<y> [FORMAT:]<input> [FORMAT:]<output>\n  imx [--no-auto-orient] [--strip] rotate <90|180|270> [FORMAT:]<input> [FORMAT:]<output>\n  imx [--no-auto-orient] [--strip] flip [FORMAT:]<input> [FORMAT:]<output>\n  imx [--no-auto-orient] [--strip] flop [FORMAT:]<input> [FORMAT:]<output>\n  imx [--strip] pipeline [FORMAT:]<input|FORMAT:-> [FORMAT:]<output|FORMAT:-> --op <op> [--op <op> ...]\n  imx [--no-auto-orient] [--strip] batch-convert --to <FORMAT> --output-dir <dir> [--resize <width>x<height>|--resize-fit <width>x<height>] [--quality <1..=100>] [FORMAT:]<input>...\n  imx [--no-auto-orient] assemble --delay <centiseconds> [--loop <n>] <output.gif|GIF:-> <frame0> <frame1> ...\n  imx completions <bash|zsh|fish>\n  imx self-test\n  imx [--no-auto-orient] [--strip] [--frame <N>] [--quality <1..=100>] [FORMAT:]<input|FORMAT:-> [FORMAT:]<output|FORMAT:->\n\nsupported formats: bmp (.bmp), farbfeld (.ff, .farbfeld), gif (.gif), jpeg (.jpg, .jpeg), qoi (.qoi), pbm (.pbm), pgm (.pgm), png (.png), ppm (.ppm), tiff (.tif, .tiff), webp (.webp)\nsupported prefixes: BMP:, FARBFELD:, GIF:, JPEG:, QOI:, PBM:, PGM:, PNG:, PPM:, TIFF:, WEBP:\nframe selection: --frame <N> (0-based, default 0) selects which frame to decode; animated GIF/WEBP can be enumerated via report --json (\"frames\" field) and a single frame extracted; non-animated inputs accept only --frame 0\nsupported orientation: EXIF/TIFF Orientation is auto-applied on decode for JPEG and TIFF so images are upright; pass --no-auto-orient to keep raw stored pixels and dimensions\nsupported resize: --filter <point|box|triangle|catmull-rom|lanczos3> selects the resampling kernel (default lanczos3); --filter point is byte-exact nearest-neighbor\nsupported animation: imx assemble writes an animated GIF from same-size input frames with a per-frame --delay (centiseconds) and --loop count (0 = infinite)\nsupported ICC: embedded ICC color profiles are preserved on decode and written back on encode for PNG, JPEG, and TIFF; geometry transforms keep the profile while colorspace conversions drop it; pass --strip to drop the profile before encoding\nstdin/stdout: use - as a path with a FORMAT: prefix (e.g. PNG:-); --quality applies only to JPEG output"
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
    let args = strip_global_flags(env::args().collect::<Vec<_>>());
    let args = strip_resize_filter_flag(args);
    match args.as_slice() {
        [_, flag] if flag == "--help" || flag == "-h" || flag == "help" => {
            println!(
                "IMX Developer Preview\n\nusage:\n  imx [--no-auto-orient] identify [--frame <N>] [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.gif|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm|input.tif|input.tiff|input.webp|FORMAT:->\n  imx [--no-auto-orient] identify --json [--frame <N>] [FORMAT:]<input|FORMAT:->\n  imx [--no-auto-orient] report --json [--frame <N>] [FORMAT:]<input|FORMAT:->\n  imx [--no-auto-orient] compare [--metric <ae|mae|psnr>] [FORMAT:]<a|FORMAT:-> [FORMAT:]<b>\n  imx [--no-auto-orient] [--strip] [--filter <point|box|triangle|catmull-rom|lanczos3>] resize <width>x<height>|<width>x|x<height>|<percent>% [FORMAT:]<input|FORMAT:-> [FORMAT:]<output|FORMAT:->\n  imx [--no-auto-orient] [--strip] [--filter <point|box|triangle|catmull-rom|lanczos3>] resize-fit <width>x<height> [FORMAT:]<input|FORMAT:-> [FORMAT:]<output|FORMAT:->\n  imx [--no-auto-orient] [--strip] crop <width>x<height>+<x>+<y> [FORMAT:]<input> [FORMAT:]<output>\n  imx [--no-auto-orient] [--strip] rotate <90|180|270> [FORMAT:]<input> [FORMAT:]<output>\n  imx [--no-auto-orient] [--strip] flip [FORMAT:]<input> [FORMAT:]<output>\n  imx [--no-auto-orient] [--strip] flop [FORMAT:]<input> [FORMAT:]<output>\n  imx [--strip] pipeline [FORMAT:]<input|FORMAT:-> [FORMAT:]<output|FORMAT:-> --op <op> [--op <op> ...]\n  imx [--no-auto-orient] [--strip] batch-convert --to <FORMAT> --output-dir <dir> [--resize <width>x<height>|--resize-fit <width>x<height>] [--quality <1..=100>] [FORMAT:]<input>...\n  imx [--no-auto-orient] assemble --delay <centiseconds> [--loop <n>] <output.gif|GIF:-> <frame0> <frame1> ...\n  imx completions <bash|zsh|fish>\n  imx self-test\n  imx [--no-auto-orient] [--strip] [--frame <N>] [--quality <1..=100>] [FORMAT:]<input|FORMAT:-> [FORMAT:]<output|FORMAT:->\n\nsupported transcodes: BMP/FARBFELD/GIF/JPEG/QOI/PBM/PGM/PNG/PPM/TIFF/WEBP, including deterministic same-format rewrites except lossy JPEG re-encoding; WEBP output is lossless; GIF output is a single still frame with a deterministic palette of at most 256 colors\nsupported frame selection: --frame <N> (0-based, default 0) selects which frame to decode for identify, report --json, and the single-input transcode; animated GIF/WEBP frames are composited (GIF disposal Keep/Background/Previous honored) so frame N is the displayed canvas; non-animated inputs accept only --frame 0 and reject any N>0\nsupported orientation: EXIF/TIFF Orientation (values 1-8) is auto-applied on decode for JPEG and TIFF so portrait photos come out upright; rotated orientations (5-8) swap reported width and height; pass --no-auto-orient to disable and keep the raw stored pixels and dimensions; missing or malformed orientation metadata is treated as orientation 1 (no-op)\nsupported ICC: embedded ICC color profiles are preserved on decode and written back on encode for PNG (iCCP chunk), JPEG (APP2 ICC_PROFILE segments), and TIFF (tag 34675); the profile bytes survive geometry transforms (resize, resize-fit, crop, rotate, flip, flop) but are dropped by colorspace conversions since they no longer describe the re-encoded samples; pass --strip to drop the profile before encoding\nsupported streaming: read input from stdin and/or write output to stdout via - with a FORMAT: prefix (e.g. PNG:-); only image bytes go to stdout\nsupported JPEG quality: --quality <1..=100> on the single transcode and batch-convert when the output format is JPEG (default 90); rejected for non-JPEG output\nsupported identify JSON: deterministic schema_version/format/width/height/channels/depth over existing identify metadata\nsupported report JSON: single-input supported/unsupported status with stable diagnostic_code values; adds a \"frames\" count (animated GIF/WEBP frame count, 1 otherwise) and uses schema_version 2\nsupported compare: decode two inputs and diff them deterministically; differing dimensions or channels print a single differ: line and exit 1, matching images normalize to RGBA8 and report differing-pixel count, peak per-channel difference (AE), and mean absolute error (MAE); identical inputs print identical and exit 0, otherwise exit 1; --metric <ae|mae|psnr> prints only that single value (psnr is inf for identical inputs); usage errors exit 2\nsupported resize: exact dimensions (<width>x<height>), single-axis aspect-preserving (<width>x or x<height>), and uniform percent (<percent>%) geometries, plus aspect-preserving fit (resize-fit) for existing supported formats; --filter <point|box|triangle|catmull-rom|lanczos3> selects the resampling kernel (default lanczos3, a high-quality windowed-sinc filter), and --filter point is byte-exact center-sampled nearest-neighbor\nsupported geometry: bounds-checked crop (<width>x<height>+<x>+<y>), clockwise rotate (90/180/270), vertical flip, and horizontal flop, all format-preserving\nsupported pipeline: imx pipeline chains ordered --op values in a single decode/encode pass; supported ops are resize:<geometry>, resize-fit:<width>x<height>, crop:<width>x<height>+<x>+<y>, rotate:<90|180|270>, flip, flop, grayscale, invert, brightness:<-255..=255>, contrast:<factor>, gamma:<value>, threshold:<0..=255>, and levels:<black>,<white>,<gamma>; ops apply left-to-right so order matters and at least one --op is required; output is byte-deterministic and equivalent to running the same ops as sequential subcommands\nsupported batch conversion: explicit output format, existing output directory, shell-expanded input paths, optional --quality for JPEG output, no overwrite or collision renaming\nsupported completions: imx completions <bash|zsh|fish> prints a shell completion script to stdout; a roff man page is bundled at man/imx.1\nsupported self-test: offline install confidence check for identify/transcode/resize/resize-fit/batch-convert across supported formats\nsupported animation: imx assemble --delay <centiseconds> [--loop <n>] <output.gif|GIF:-> <frame0> <frame1> ... decodes each input frame (any supported format), then writes an animated GIF with one image block per frame; every frame is quantized independently to its own deterministic local palette of at most 256 colors, all frames must share identical dimensions, --delay sets a uniform inter-frame delay in centiseconds, --loop <n> writes a Netscape looping extension (0 = infinite, the default), and the output is byte-deterministic\nsupported prefixes: BMP:, FARBFELD:, GIF:, JPEG:, QOI:, PBM:, PGM:, PNG:, PPM:, TIFF:, WEBP:\nunsupported: animated WEBP OUTPUT (encode) unsupported; WEBP frame extraction on decode is supported, animated GIF output is supported via assemble while GIF frame extraction on decode remains, recursive directory walking, arbitrary-angle rotation, delegates, and formats beyond BMP/FARBFELD/GIF/JPEG/QOI/PBM/PGM/PNG/PPM/TIFF/WEBP"
            );
            process::exit(0);
        }
        [_, flag] if flag == "--version" || flag == "-V" || flag == "version" => {
            println!("imx {}", env!("CARGO_PKG_VERSION"));
            process::exit(0);
        }
        [_, command, flag] if command == "identify" && flag == "--json" => usage(),
        [_, command, json_flag, frame_flag, frame, input]
            if command == "identify" && json_flag == "--json" && frame_flag == "--frame" =>
        {
            identify_json(
                input,
                parse_frame(frame).unwrap_or_else(|err| fail_usage(err)),
            )
        }
        [_, command, flag, input] if command == "identify" && flag == "--json" => {
            identify_json(input, 0)
        }
        [_, command, frame_flag, frame, input]
            if command == "identify" && frame_flag == "--frame" =>
        {
            identify(
                input,
                parse_frame(frame).unwrap_or_else(|err| fail_usage(err)),
            )
        }
        [_, command, input] if command == "identify" => identify(input, 0),
        [_, command, flag, metric, a, b] if command == "compare" && flag == "--metric" => {
            compare(a, b, Some(metric))
        }
        [_, command, a, b] if command == "compare" => compare(a, b, None),
        [_, command, ..] if command == "compare" => usage(),
        [_, command, json_flag, frame_flag, frame, input]
            if command == "report" && json_flag == "--json" && frame_flag == "--frame" =>
        {
            report_json(
                input,
                parse_frame(frame).unwrap_or_else(|err| fail_usage(err)),
            )
        }
        [_, command, flag, input] if command == "report" && flag == "--json" => {
            report_json(input, 0)
        }
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
        [_, command, rest @ ..] if command == "assemble" => assemble(rest),
        [_, command, rest @ ..] if command == "batch-convert" => batch_convert(rest),
        [_, command, rest @ ..] if command == "pipeline" => pipeline(rest),
        [_, command] if command == "self-test" => self_test(),
        [_, command, ..] if command == "self-test" => usage(),
        [_, command, shell] if command == "completions" => completions(shell),
        [_, command, ..] if command == "completions" => usage(),
        [_, command, ..] if is_unsupported_command_shape(command) => usage(),
        [_, frame_flag, frame, quality_flag, quality, input, output]
            if frame_flag == "--frame" && quality_flag == "--quality" =>
        {
            let frame = parse_frame(frame).unwrap_or_else(|err| fail_usage(err));
            run_transcode(
                input,
                output,
                Some(parse_quality(quality).unwrap_or_else(|err| fail(err))),
                frame,
            )
        }
        [_, quality_flag, quality, frame_flag, frame, input, output]
            if quality_flag == "--quality" && frame_flag == "--frame" =>
        {
            let frame = parse_frame(frame).unwrap_or_else(|err| fail_usage(err));
            run_transcode(
                input,
                output,
                Some(parse_quality(quality).unwrap_or_else(|err| fail(err))),
                frame,
            )
        }
        [_, frame_flag, frame, input, output] if frame_flag == "--frame" => {
            let frame = parse_frame(frame).unwrap_or_else(|err| fail_usage(err));
            run_transcode(input, output, None, frame)
        }
        [_, flag, quality, input, output] if flag == "--quality" => {
            transcode_with_quality(quality, input, output)
        }
        [_, input, output] => transcode(input, output),
        _ => usage(),
    }
}

fn is_unsupported_command_shape(command: &str) -> bool {
    matches!(command, "convert" | "magick" | "mogrify")
}

/// Remove every global flag occurrence from the argument vector, recording
/// whether each was present in its backing atomic.
///
/// The recognized global flags are position-independent and apply to whichever
/// subcommand runs:
/// - `--no-auto-orient` disables EXIF/TIFF Orientation auto-application on
///   decode (recorded in [`AUTO_ORIENT`]).
/// - `--strip` drops the embedded ICC color profile before encoding (recorded
///   in [`STRIP_ICC`]).
///
/// Stripping them here keeps the positional subcommand matching in [`main`]
/// unchanged. The program name (`args[0]`) is always preserved.
fn strip_global_flags(args: Vec<String>) -> Vec<String> {
    if args.iter().any(|arg| arg == "--no-auto-orient") {
        AUTO_ORIENT.store(false, Ordering::Relaxed);
    }
    if args.iter().any(|arg| arg == "--strip") {
        STRIP_ICC.store(true, Ordering::Relaxed);
    }
    args.into_iter()
        .enumerate()
        .filter(|(index, arg)| *index == 0 || (arg != "--no-auto-orient" && arg != "--strip"))
        .map(|(_, arg)| arg)
        .collect()
}

/// Remove a single `--filter <name>` pair from the argument vector, recording
/// the selected resampling filter in [`RESIZE_FILTER`].
///
/// Like `--no-auto-orient`, `--filter` is global and position-independent so the
/// positional subcommand matching in [`main`] stays unchanged; it only affects
/// the `resize` and `resize-fit` paths, which read [`resize_filter`]. The
/// default (when absent) is Lanczos3. A missing or invalid value is a usage
/// error. The program name (`args[0]`) is always preserved.
fn strip_resize_filter_flag(args: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut iter = args.into_iter().enumerate();
    while let Some((index, arg)) = iter.next() {
        if index != 0 && arg == "--filter" {
            let Some((_, value)) = iter.next() else {
                fail_usage("--filter requires a value: <point|box|triangle|catmull-rom|lanczos3>");
            };
            let filter = parse_resize_filter(&value).unwrap_or_else(|err| fail_usage(err));
            RESIZE_FILTER.store(filter_code(filter), Ordering::Relaxed);
        } else {
            out.push(arg);
        }
    }
    out
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

fn completions(shell: &str) -> ! {
    let script = match shell {
        "bash" => completions::BASH,
        "zsh" => completions::ZSH,
        "fish" => completions::FISH,
        other => fail_usage(format!(
            "unsupported shell: {other}; expected bash, zsh, or fish"
        )),
    };
    let mut stdout = std::io::stdout().lock();
    if let Err(err) = stdout.write_all(script.as_bytes()) {
        fail(format!("failed to write stdout: {err}"));
    }
    if let Err(err) = stdout.flush() {
        fail(format!("failed to write stdout: {err}"));
    }
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
        (
            Format::Tiff,
            "input.tiff",
            imx_codec_tiff::encode(&rgb),
            "format=TIFF width=2 height=1 channels=RGB depth=8",
        ),
        (
            Format::Webp,
            "input.webp",
            imx_codec_webp::encode(&rgb),
            "format=WEBP width=2 height=1 channels=RGB depth=8",
        ),
        (
            Format::Gif,
            "input.gif",
            imx_codec_gif::encode(&rgb),
            "format=GIF width=2 height=1 channels=RGBA depth=8",
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
        "{{\"schema_version\":2,\"status\":\"supported\",\"diagnostic_code\":null,\"format\":\"{format}\",\"width\":{width},\"height\":{height},\"channels\":\"{channels}\",\"depth\":{depth},\"frames\":1}}"
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompareMetric {
    Ae,
    Mae,
    Psnr,
}

fn parse_compare_metric(value: &str) -> Result<CompareMetric, String> {
    match value {
        "ae" => Ok(CompareMetric::Ae),
        "mae" => Ok(CompareMetric::Mae),
        "psnr" => Ok(CompareMetric::Psnr),
        other => Err(format!(
            "invalid --metric value: {other}; expected ae, mae, or psnr"
        )),
    }
}

fn load_compare_operand(arg: &str, role: &str) -> imx_core::Image {
    let path = parse_cli_path(arg).unwrap_or_else(|err| fail(err));
    let input = read(path.path);
    let format = detect_input_format(&path, &input).unwrap_or_else(|err| fail(err));
    decode_image(format, &input)
        .unwrap_or_else(|err| fail_image_operation(format, "decode", role, &path, err))
}

fn compare(a_arg: &str, b_arg: &str, metric: Option<&str>) -> ! {
    let metric =
        metric.map(|value| parse_compare_metric(value).unwrap_or_else(|err| fail_usage(err)));

    // At most one operand may be stdin.
    let a_path = parse_cli_path(a_arg).unwrap_or_else(|err| fail(err));
    let b_path = parse_cli_path(b_arg).unwrap_or_else(|err| fail(err));
    if a_path.path == "-" && b_path.path == "-" {
        fail_usage("at most one compare operand may be read from stdin");
    }

    let a = load_compare_operand(a_arg, "first input");
    let b = load_compare_operand(b_arg, "second input");

    // Dimension mismatch: deterministic differ line, exit 1, no pixel diff.
    if a.width() != b.width() || a.height() != b.height() {
        println!(
            "differ: dimensions {}x{} vs {}x{}",
            a.width(),
            a.height(),
            b.width(),
            b.height()
        );
        process::exit(1);
    }

    // Channel/pixel-format mismatch in the comparable representation: report it
    // and exit 1 without attempting a pixel diff.
    let a_channels = a.pixel_format().channels();
    let b_channels = b.pixel_format().channels();
    if a_channels != b_channels {
        println!("differ: channels {a_channels} vs {b_channels}");
        process::exit(1);
    }

    let cmp = compare_rgba8(&a, &b).unwrap_or_else(|err| fail(format!("failed to compare: {err}")));

    if let Some(metric) = metric {
        match metric {
            CompareMetric::Ae => println!("{}", cmp.max_abs_diff),
            CompareMetric::Mae => println!("{:.6}", cmp.mae()),
            CompareMetric::Psnr => {
                let psnr = cmp.psnr();
                if psnr.is_infinite() {
                    println!("inf");
                } else {
                    println!("{psnr:.6}");
                }
            }
        }
        if cmp.is_identical() {
            process::exit(0);
        }
        process::exit(1);
    }

    if cmp.is_identical() {
        println!("identical");
        process::exit(0);
    }

    println!(
        "differ: {}/{} pixels ae={} mae={:.6}",
        cmp.differing_pixels,
        cmp.total_pixels,
        cmp.max_abs_diff,
        cmp.mae()
    );
    process::exit(1);
}

fn identify(input_path: &str, frame: u32) -> ! {
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let input = read(input_path.path);
    let format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let info = identify_bytes(format, &input)
        .unwrap_or_else(|err| fail_image_operation(format, "identify", "input", &input_path, err));
    // Validate the requested frame exists; identify metadata is the canvas, so
    // the line itself is frame-independent, but an out-of-range frame is an error.
    validate_frame_in_range(format, &input, frame)
        .unwrap_or_else(|err| fail_image_operation(format, "identify", "input", &input_path, err));
    println!("{}", info.stable_line());
    process::exit(0);
}

fn identify_json(input_path: &str, frame: u32) -> ! {
    match try_identify(input_path, frame) {
        Ok((info, _frames)) => {
            println!("{}", identify_json_object(info));
            process::exit(0);
        }
        Err(err) => {
            eprintln!("{}", diagnostic_json_object(&err));
            process::exit(1);
        }
    }
}

fn report_json(input_path: &str, frame: u32) -> ! {
    match try_identify(input_path, frame) {
        Ok((info, frames)) => println!("{}", report_supported_json_object(info, frames)),
        Err(err) => println!("{}", report_unsupported_json_object(&err)),
    }
    process::exit(0);
}

/// Validate that `frame` is a decodable index for `input`, returning a clean
/// [`ImageError::FrameIndexOutOfRange`] otherwise.
fn validate_frame_in_range(format: Format, input: &[u8], frame: u32) -> Result<(), ImageError> {
    let frames = frame_count_for(format, input)?;
    if frame >= frames {
        return Err(ImageError::FrameIndexOutOfRange {
            index: frame,
            frame_count: frames,
        });
    }
    Ok(())
}

fn identify_bytes(format: Format, input: &[u8]) -> Result<Identify, ImageError> {
    match format {
        Format::Bmp => imx_codec_bmp::identify(input),
        Format::Farbfeld => imx_codec_farbfeld::identify(input),
        Format::Gif => imx_codec_gif::identify(input),
        Format::Jpeg => imx_codec_jpeg::identify_with_options(input, auto_orient()),
        Format::Pbm => imx_codec_pnm::identify_pbm(input),
        Format::Pgm => imx_codec_pnm::identify_pgm(input),
        Format::Png => imx_codec_png::identify(input),
        Format::Ppm => imx_codec_pnm::identify_ppm(input),
        Format::Qoi => imx_codec_qoi::identify(input),
        Format::Tiff => imx_codec_tiff::identify_with_options(input, auto_orient()),
        Format::Webp => imx_codec_webp::identify(input),
    }
}

fn try_identify(input_path: &str, frame: u32) -> Result<(Identify, u32), Diagnostic> {
    let input_path = parse_cli_path_diagnostic(input_path)?;
    let input = read_diagnostic(input_path.path)?;
    let format = detect_input_format_diagnostic(&input_path, &input)?;
    let info = identify_bytes(format, &input).map_err(|err| {
        Diagnostic::new(
            image_diagnostic_code(format, "identify", &err),
            format!("failed to identify {} input: {err}", format.name()),
        )
    })?;
    let frames = frame_count_for(format, &input).map_err(|err| {
        Diagnostic::new(
            image_diagnostic_code(format, "identify", &err),
            format!("failed to identify {} input: {err}", format.name()),
        )
    })?;
    if frame >= frames {
        let err = ImageError::FrameIndexOutOfRange {
            index: frame,
            frame_count: frames,
        };
        return Err(Diagnostic::new(
            err.diagnostic_code(),
            format!("failed to identify {} input: {err}", format.name()),
        ));
    }
    Ok((info, frames))
}

fn transcode(input_path: &str, output_path: &str) -> ! {
    run_transcode(input_path, output_path, None, 0)
}

fn transcode_with_quality(quality: &str, input_path: &str, output_path: &str) -> ! {
    let quality = parse_quality(quality).unwrap_or_else(|err| fail(err));
    run_transcode(input_path, output_path, Some(quality), 0)
}

fn run_transcode(input_path: &str, output_path: &str, quality: Option<u8>, frame: u32) -> ! {
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let output_path = parse_cli_path(output_path).unwrap_or_else(|err| fail(err));
    reject_same_path(input_path.path, output_path.path);
    let input = read(input_path.path);
    let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));

    if quality.is_some() && output_format != Format::Jpeg {
        fail(format!(
            "--quality only applies to JPEG output, not {}",
            output_format.name()
        ));
    }

    let image = decode_image_frame(input_format, &input, frame).unwrap_or_else(|err| {
        fail_image_operation(input_format, "decode", "input", &input_path, err)
    });
    let output = encode_image_with_quality(output_format, &image, quality).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_output(output_path.path, &output);
    process::exit(0);
}

fn parse_frame(value: &str) -> Result<u32, String> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "invalid --frame value: {value}; expected a 0-based frame index"
        ));
    }
    value
        .parse::<u32>()
        .map_err(|_| format!("invalid --frame value: {value}; expected a 0-based frame index"))
}

fn parse_quality(value: &str) -> Result<u8, String> {
    let quality = value
        .parse::<u8>()
        .map_err(|_| format!("invalid --quality value: {value}; expected 1..=100"))?;
    if !(1..=100).contains(&quality) {
        return Err(format!(
            "invalid --quality value: {value}; expected 1..=100"
        ));
    }
    Ok(quality)
}

fn resize(dimensions: &str, input_path: &str, output_path: &str) -> ! {
    let geometry = ResizeGeometry::parse(dimensions).unwrap_or_else(|err| fail_usage(err));
    let input_path = parse_cli_path(input_path).unwrap_or_else(|err| fail(err));
    let output_path = parse_cli_path(output_path).unwrap_or_else(|err| fail(err));
    reject_same_path(input_path.path, output_path.path);
    let input = read(input_path.path);
    let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));

    let image = decode_image(input_format, &input).unwrap_or_else(|err| {
        fail_image_operation(input_format, "decode", "input", &input_path, err)
    });
    let (width, height) = geometry
        .resolve(image.width(), image.height())
        .unwrap_or_else(|err| {
            fail_image_operation(input_format, "resize", "input", &input_path, err)
        });
    let image = image
        .resize_filtered(width, height, resize_filter())
        .unwrap_or_else(|err| {
            fail_image_operation(input_format, "resize", "input", &input_path, err)
        });
    let output = encode_image(output_format, &image).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_output(output_path.path, &output);
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
        .resize_filtered_fit(dimensions.width, dimensions.height, resize_filter())
        .unwrap_or_else(|err| {
            fail_image_operation(input_format, "resize-fit", "input", &input_path, err)
        });
    let output = encode_image(output_format, &image).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_output(output_path.path, &output);
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

/// A single pipeline operation, parsed from a `--op` spec. Each variant reuses
/// the same geometry/angle parsing as the corresponding standalone subcommand so
/// behavior is identical; the math is delegated to `imx_core::Image` methods.
#[derive(Debug, Clone, Copy)]
enum Op {
    Resize(ResizeGeometry),
    ResizeFit(ResizeDimensions),
    Crop(CropGeometry),
    Rotate(RotateAngle),
    Flip,
    Flop,
    Grayscale,
    Invert,
    Brightness(i16),
    Contrast(f32),
    Gamma(f32),
    Threshold(u8),
    Levels { black: u8, white: u8, gamma: f32 },
}

impl Op {
    /// The operation name used in runtime error messages, matching the
    /// standalone subcommand names.
    fn name(self) -> &'static str {
        match self {
            Op::Resize(_) => "resize",
            Op::ResizeFit(_) => "resize-fit",
            Op::Crop(_) => "crop",
            Op::Rotate(_) => "rotate",
            Op::Flip => "flip",
            Op::Flop => "flop",
            Op::Grayscale => "grayscale",
            Op::Invert => "invert",
            Op::Brightness(_) => "brightness",
            Op::Contrast(_) => "contrast",
            Op::Gamma(_) => "gamma",
            Op::Threshold(_) => "threshold",
            Op::Levels { .. } => "levels",
        }
    }

    /// Apply this operation to `image`, producing a new image. Errors are
    /// runtime/operational errors (e.g. crop out of bounds).
    fn apply(self, image: imx_core::Image) -> Result<imx_core::Image, ImageError> {
        match self {
            Op::Resize(geometry) => {
                let (width, height) = geometry.resolve(image.width(), image.height())?;
                image.resize_filtered(width, height, resize_filter())
            }
            Op::ResizeFit(dimensions) => {
                image.resize_filtered_fit(dimensions.width, dimensions.height, resize_filter())
            }
            Op::Crop(geometry) => {
                image.crop(geometry.x, geometry.y, geometry.width, geometry.height)
            }
            Op::Rotate(RotateAngle::Ninety) => image.rotate_90(),
            Op::Rotate(RotateAngle::OneEighty) => image.rotate_180(),
            Op::Rotate(RotateAngle::TwoSeventy) => image.rotate_270(),
            Op::Flip => image.flip_vertical(),
            Op::Flop => image.flop_horizontal(),
            Op::Grayscale => image.grayscale(),
            Op::Invert => image.invert(),
            Op::Brightness(delta) => image.brightness(delta),
            Op::Contrast(factor) => image.contrast(factor),
            Op::Gamma(value) => image.gamma(value),
            Op::Threshold(level) => image.threshold(level),
            Op::Levels {
                black,
                white,
                gamma,
            } => image.levels(black, white, gamma),
        }
    }
}

/// Parse a single `--op` spec into an [`Op`]. The op name is everything before
/// the first `:`; the remainder (if any) is the argument. An invalid spec or bad
/// geometry/angle is a usage error.
fn parse_pipeline_op(spec: &str) -> Result<Op, String> {
    let (name, arg) = match spec.split_once(':') {
        Some((name, arg)) => (name, Some(arg)),
        None => (spec, None),
    };
    match (name, arg) {
        ("resize", Some(arg)) => ResizeGeometry::parse(arg)
            .map(Op::Resize)
            .map_err(|err| err.to_string()),
        ("resize-fit", Some(arg)) => parse_resize_dimensions(arg).map(Op::ResizeFit),
        ("crop", Some(arg)) => parse_crop_geometry(arg).map(Op::Crop),
        ("rotate", Some(arg)) => parse_rotate_angle(arg).map(Op::Rotate),
        ("flip", None) => Ok(Op::Flip),
        ("flop", None) => Ok(Op::Flop),
        ("grayscale", None) => Ok(Op::Grayscale),
        ("invert", None) => Ok(Op::Invert),
        ("brightness", Some(arg)) => parse_brightness_arg(arg).map(Op::Brightness),
        ("contrast", Some(arg)) => parse_positive_or_zero_f32(arg, "contrast").map(Op::Contrast),
        ("gamma", Some(arg)) => parse_positive_f32(arg, "gamma").map(Op::Gamma),
        ("threshold", Some(arg)) => parse_threshold_arg(arg).map(Op::Threshold),
        ("levels", Some(arg)) => parse_levels_arg(arg),
        ("resize" | "resize-fit" | "crop" | "rotate", None) => {
            Err(format!("pipeline op {name} requires an argument: {name}:<value>"))
        }
        ("brightness" | "contrast" | "gamma" | "threshold" | "levels", None) => {
            Err(format!("pipeline op {name} requires an argument: {name}:<value>"))
        }
        ("flip" | "flop" | "grayscale" | "invert", Some(_)) => {
            Err(format!("pipeline op {name} does not take an argument"))
        }
        _ => Err(format!(
            "unsupported pipeline op: {spec}; expected resize:<geometry>, resize-fit:<width>x<height>, crop:<width>x<height>+<x>+<y>, rotate:<90|180|270>, flip, flop, grayscale, invert, brightness:<-255..=255>, contrast:<factor>, gamma:<value>, threshold:<0..=255>, or levels:<black>,<white>,<gamma>"
        )),
    }
}

/// Parse the `brightness` argument: an integer delta in `-255..=255`.
fn parse_brightness_arg(arg: &str) -> Result<i16, String> {
    let value: i16 = arg
        .parse()
        .map_err(|_| format!("brightness delta must be an integer in -255..=255, got {arg}"))?;
    if !(-255..=255).contains(&value) {
        return Err(format!(
            "brightness delta must be an integer in -255..=255, got {value}"
        ));
    }
    Ok(value)
}

/// Parse the `threshold` argument: an integer level in `0..=255`.
fn parse_threshold_arg(arg: &str) -> Result<u8, String> {
    arg.parse()
        .map_err(|_| format!("threshold level must be an integer in 0..=255, got {arg}"))
}

/// Parse a strictly positive, finite float argument (e.g. gamma).
fn parse_positive_f32(arg: &str, op: &str) -> Result<f32, String> {
    let value: f32 = arg
        .parse()
        .map_err(|_| format!("{op} value must be a number, got {arg}"))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("{op} value must be a finite number > 0, got {arg}"));
    }
    Ok(value)
}

/// Parse a non-negative, finite float argument (e.g. contrast factor).
fn parse_positive_or_zero_f32(arg: &str, op: &str) -> Result<f32, String> {
    let value: f32 = arg
        .parse()
        .map_err(|_| format!("{op} value must be a number, got {arg}"))?;
    if !value.is_finite() || value < 0.0 {
        return Err(format!(
            "{op} value must be a finite number >= 0, got {arg}"
        ));
    }
    Ok(value)
}

/// Parse the `levels` argument `<black>,<white>,<gamma>` with `0 <= black <
/// white <= 255` and `gamma > 0`.
fn parse_levels_arg(arg: &str) -> Result<Op, String> {
    let parts: Vec<&str> = arg.split(',').collect();
    let [black, white, gamma] = parts.as_slice() else {
        return Err(format!(
            "levels requires three comma-separated values: levels:<black>,<white>,<gamma>, got {arg}"
        ));
    };
    let black: u8 = black
        .parse()
        .map_err(|_| format!("levels black must be an integer in 0..=255, got {black}"))?;
    let white: u8 = white
        .parse()
        .map_err(|_| format!("levels white must be an integer in 0..=255, got {white}"))?;
    let gamma = parse_positive_f32(gamma, "levels gamma")?;
    if black >= white {
        return Err(format!(
            "levels black ({black}) must be less than white ({white})"
        ));
    }
    Ok(Op::Levels {
        black,
        white,
        gamma,
    })
}

/// Parse the positional input/output arguments and the ordered list of `--op`
/// specs for the pipeline command. At least one `--op` is required.
fn parse_pipeline_args(args: &[String]) -> Result<(&str, &str, Vec<Op>), String> {
    let mut positionals: Vec<&str> = Vec::new();
    let mut ops: Vec<Op> = Vec::new();
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--op" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("pipeline --op requires a value".to_string());
                };
                ops.push(parse_pipeline_op(value)?);
                index += 2;
            }
            option if option.starts_with("--") => {
                return Err(format!("unsupported pipeline option: {option}"));
            }
            value => {
                positionals.push(value);
                index += 1;
            }
        }
    }

    let [input, output] = positionals.as_slice() else {
        return Err("pipeline requires exactly one input and one output path".to_string());
    };
    if ops.is_empty() {
        return Err("pipeline requires at least one --op".to_string());
    }
    Ok((input, output, ops))
}

fn pipeline(args: &[String]) -> ! {
    let (input_arg, output_arg, ops) =
        parse_pipeline_args(args).unwrap_or_else(|err| fail_usage(err));

    let input_path = parse_cli_path(input_arg).unwrap_or_else(|err| fail(err));
    let output_path = parse_cli_path(output_arg).unwrap_or_else(|err| fail(err));
    reject_same_path(input_path.path, output_path.path);
    let input = read(input_path.path);
    let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));

    let mut image = decode_image(input_format, &input).unwrap_or_else(|err| {
        fail_image_operation(input_format, "decode", "input", &input_path, err)
    });
    for op in ops {
        image = op.apply(image).unwrap_or_else(|err| {
            fail_image_operation(input_format, op.name(), "input", &input_path, err)
        });
    }
    let output = encode_image(output_format, &image).unwrap_or_else(|err| {
        fail_image_operation(output_format, "encode", "output", &output_path, err)
    });

    write_output(output_path.path, &output);
    process::exit(0);
}

/// Options parsed for the `assemble` subcommand: a required inter-frame delay,
/// an optional loop count, the GIF output path, and the ordered frame inputs.
struct AssembleOptions<'a> {
    delay_cs: u16,
    loop_count: u16,
    output: &'a str,
    frames: Vec<&'a str>,
}

/// Parse `assemble` flags (`--delay`, optional `--loop`) followed by the output
/// path and the trailing variadic frame list. Returns a usage-style error string
/// on any malformed or missing argument.
fn parse_assemble_options(args: &[String]) -> Result<AssembleOptions<'_>, String> {
    let mut delay_cs: Option<u16> = None;
    let mut loop_count: u16 = 0;
    let mut index = 0;

    // Leading flags. Both flags must precede the positional output/frames.
    while index < args.len() {
        match args[index].as_str() {
            "--delay" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--delay requires a value in centiseconds".to_string())?;
                delay_cs = Some(parse_u16_arg("--delay", value)?);
                index += 2;
            }
            "--loop" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--loop requires a value".to_string())?;
                loop_count = parse_u16_arg("--loop", value)?;
                index += 2;
            }
            // First non-flag argument: the positional output path begins here.
            _ => break,
        }
    }

    let delay_cs =
        delay_cs.ok_or_else(|| "assemble requires --delay <centiseconds>".to_string())?;

    let output = args
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| "assemble requires an output path and at least one frame".to_string())?;
    index += 1;

    let frames: Vec<&str> = args[index..].iter().map(String::as_str).collect();
    if frames.is_empty() {
        return Err("assemble requires at least one frame".to_string());
    }

    Ok(AssembleOptions {
        delay_cs,
        loop_count,
        output,
        frames,
    })
}

fn parse_u16_arg(flag: &str, value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|_| format!("invalid {flag} value: {value}; expected 0..=65535"))
}

/// Decode each input frame and encode them as a single animated GIF.
fn assemble(args: &[String]) -> ! {
    let options = parse_assemble_options(args).unwrap_or_else(|err| fail_usage(err));

    let output_path = parse_cli_path(options.output).unwrap_or_else(|err| fail(err));
    let output_format = detect_output_format(&output_path).unwrap_or_else(|err| fail(err));
    if output_format != Format::Gif {
        fail(format!(
            "assemble only writes GIF output, not {}",
            output_format.name()
        ));
    }

    let mut images = Vec::with_capacity(options.frames.len());
    for frame in &options.frames {
        let input_path = parse_cli_path(frame).unwrap_or_else(|err| fail(err));
        let input = read(input_path.path);
        let input_format = detect_input_format(&input_path, &input).unwrap_or_else(|err| fail(err));
        let image = decode_image(input_format, &input).unwrap_or_else(|err| {
            fail_image_operation(input_format, "decode", "input", &input_path, err)
        });
        images.push(image);
    }

    let output = imx_codec_gif::encode_animation(&images, options.delay_cs, options.loop_count)
        .unwrap_or_else(|err| {
            fail_image_operation(Format::Gif, "encode", "output", &output_path, err)
        });

    write_output(output_path.path, &output);
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
                .resize_filtered(dimensions.width, dimensions.height, resize_filter())
                .unwrap_or_else(|err| {
                    fail_image_operation(input_format, "resize", "input", &plan.input_path, err)
                }),
            Some(BatchTransform::ResizeFit(dimensions)) => image
                .resize_filtered_fit(dimensions.width, dimensions.height, resize_filter())
                .unwrap_or_else(|err| {
                    fail_image_operation(input_format, "resize-fit", "input", &plan.input_path, err)
                }),
            None => image,
        };
        let output = encode_image_with_quality(options.output_format, &image, options.quality)
            .unwrap_or_else(|err| {
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
    decode_image_frame(format, input, 0)
}

/// Decode the requested 0-based frame. Animated GIF/WebP select the Nth
/// composited frame; every other format is single-frame, so only frame 0 is
/// valid and any other index is rejected with [`ImageError::FrameIndexOutOfRange`].
fn decode_image_frame(
    format: Format,
    input: &[u8],
    frame: u32,
) -> Result<imx_core::Image, ImageError> {
    match format {
        Format::Gif => imx_codec_gif::decode_frame(input, frame),
        Format::Webp => imx_codec_webp::decode_frame(input, frame),
        // Single-frame formats: only frame 0 exists.
        _ if frame != 0 => Err(ImageError::FrameIndexOutOfRange {
            index: frame,
            frame_count: 1,
        }),
        Format::Bmp => imx_codec_bmp::decode(input),
        Format::Farbfeld => imx_codec_farbfeld::decode(input),
        Format::Jpeg => imx_codec_jpeg::decode_with_options(input, auto_orient()),
        Format::Pbm => imx_codec_pnm::decode_pbm(input),
        Format::Pgm => imx_codec_pnm::decode_pgm(input),
        Format::Png => imx_codec_png::decode(input),
        Format::Ppm => imx_codec_pnm::decode_ppm(input),
        Format::Qoi => imx_codec_qoi::decode(input).and_then(|decoded| decoded.into_core_image()),
        Format::Tiff => imx_codec_tiff::decode_with_options(input, auto_orient()),
    }
}

/// Number of frames the input declares. Animated GIF/WebP report their true
/// frame count; all other formats are single-frame and report 1.
fn frame_count_for(format: Format, input: &[u8]) -> Result<u32, ImageError> {
    match format {
        Format::Gif => imx_codec_gif::frame_count(input),
        Format::Webp => imx_codec_webp::frame_count(input),
        _ => Ok(1),
    }
}

fn encode_image_with_quality(
    format: Format,
    image: &imx_core::Image,
    quality: Option<u8>,
) -> Result<Vec<u8>, ImageError> {
    match (format, quality) {
        (Format::Jpeg, Some(quality)) => {
            imx_codec_jpeg::encode_with_quality(&maybe_strip_icc(image), quality)
        }
        _ => encode_image(format, image),
    }
}

/// Drop the embedded ICC profile when the global `--strip` flag was set.
///
/// Returns a borrow on the common (no-strip) path so the full pixel buffer is
/// not cloned; only when `--strip` is active is the image cloned (with the
/// profile cleared), since [`imx_core::Image::with_icc`] consumes `self`.
fn maybe_strip_icc(image: &imx_core::Image) -> std::borrow::Cow<'_, imx_core::Image> {
    if strip_icc() {
        std::borrow::Cow::Owned(image.clone().with_icc(None))
    } else {
        std::borrow::Cow::Borrowed(image)
    }
}

fn encode_image(format: Format, image: &imx_core::Image) -> Result<Vec<u8>, ImageError> {
    let image = &*maybe_strip_icc(image);
    match format {
        Format::Bmp => imx_codec_bmp::encode(image),
        Format::Farbfeld => imx_codec_farbfeld::encode(image),
        Format::Jpeg => imx_codec_jpeg::encode(image),
        Format::Pbm => imx_codec_pnm::encode_pbm(image),
        Format::Pgm => imx_codec_pnm::encode_pgm(image),
        Format::Png => imx_codec_png::encode(image),
        Format::Ppm => imx_codec_pnm::encode_ppm(image),
        Format::Qoi => imx_codec_qoi::encode_image(image, imx_codec_qoi::QOI_SRGB),
        Format::Tiff => imx_codec_tiff::encode(image),
        Format::Webp => imx_codec_webp::encode(image),
        Format::Gif => imx_codec_gif::encode(image),
    }
}

fn read(path: &str) -> Vec<u8> {
    if path == "-" {
        return read_stdin();
    }
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

fn read_stdin() -> Vec<u8> {
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut input)
        .unwrap_or_else(|err| fail(format!("failed to read stdin: {err}")));
    if input.len() as u64 > MAX_INPUT_BYTES {
        fail(format!(
            "input too large: stdin exceeds {MAX_INPUT_BYTES} byte limit"
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
    if path == "-" {
        let mut input = Vec::new();
        std::io::stdin()
            .lock()
            .take(MAX_INPUT_BYTES + 1)
            .read_to_end(&mut input)
            .map_err(|err| {
                Diagnostic::new("input.read_failed", format!("failed to read stdin: {err}"))
            })?;
        if input.len() as u64 > MAX_INPUT_BYTES {
            return Err(Diagnostic::new(
                "input.too_large",
                format!("input too large: stdin exceeds {MAX_INPUT_BYTES} byte limit"),
            ));
        }
        return Ok(input);
    }
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

fn report_supported_json_object(info: Identify, frames: u32) -> String {
    format!(
        "{{\"schema_version\":2,\"status\":\"supported\",\"diagnostic_code\":null,\"format\":\"{}\",\"width\":{},\"height\":{},\"channels\":\"{}\",\"depth\":{},\"frames\":{frames}}}",
        info.format.name(),
        info.width,
        info.height,
        info.pixel_format.channels(),
        info.pixel_format.depth()
    )
}

fn report_unsupported_json_object(diagnostic: &Diagnostic) -> String {
    format!(
        "{{\"schema_version\":2,\"status\":\"unsupported\",\"diagnostic_code\":\"{}\",\"message\":{}}}",
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
    if input_path == "-" || output_path == "-" {
        return;
    }
    if let (Ok(input), Ok(output)) = (fs::canonicalize(input_path), fs::canonicalize(output_path)) {
        if input == output {
            fail(format!(
                "input and output paths must be different: {input_path} and {output_path}"
            ));
        }
    }
}

fn write_output(output_path: &str, bytes: &[u8]) {
    if output_path == "-" {
        write_stdout(bytes);
        return;
    }
    write_atomic(output_path, bytes);
}

fn write_stdout(bytes: &[u8]) {
    let mut stdout = std::io::stdout().lock();
    if let Err(err) = stdout.write_all(bytes) {
        fail(format!("failed to write stdout: {err}"));
    }
    if let Err(err) = stdout.flush() {
        fail(format!("failed to write stdout: {err}"));
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
    quality: Option<u8>,
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
        "GIF" => Some(Format::Gif),
        "JPEG" => Some(Format::Jpeg),
        "PBM" => Some(Format::Pbm),
        "PGM" => Some(Format::Pgm),
        "PNG" => Some(Format::Png),
        "PPM" => Some(Format::Ppm),
        "QOI" => Some(Format::Qoi),
        "TIFF" => Some(Format::Tiff),
        "WEBP" => Some(Format::Webp),
        _ => None,
    }
}

fn parse_batch_options(args: &[String]) -> Result<BatchOptions<'_>, String> {
    let mut output_format = None;
    let mut output_dir = None;
    let mut transform = None;
    let mut quality = None;
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
            "--quality" => {
                if quality.is_some() {
                    return Err("batch-convert --quality may only be supplied once".to_string());
                }
                let Some(value) = args.get(index + 1) else {
                    return Err("batch-convert --quality requires a value".to_string());
                };
                if value.starts_with("--") {
                    return Err("batch-convert --quality requires a value".to_string());
                }
                quality = Some(parse_quality(value)?);
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
    if quality.is_some() && output_format != Format::Jpeg {
        return Err(format!(
            "--quality only applies to JPEG output, not {}",
            output_format.name()
        ));
    }
    let inputs = args[index..].iter().map(String::as_str).collect::<Vec<_>>();
    if inputs.is_empty() {
        return Err("batch-convert requires at least one input".to_string());
    }

    Ok(BatchOptions {
        output_format,
        output_dir,
        transform,
        quality,
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
        Format::Gif => "gif",
        Format::Jpeg => "jpg",
        Format::Pbm => "pbm",
        Format::Pgm => "pgm",
        Format::Png => "png",
        Format::Ppm => "ppm",
        Format::Qoi => "qoi",
        Format::Tiff => "tiff",
        Format::Webp => "webp",
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
    if bytes.len() >= imx_codec_webp::MAGIC_LEN
        && &bytes[..4] == imx_codec_webp::RIFF_MAGIC
        && &bytes[8..12] == imx_codec_webp::WEBP_MAGIC
    {
        return Ok(Format::Webp);
    }
    if bytes.len() >= imx_codec_gif::MAGIC_LEN
        && (&bytes[..imx_codec_gif::MAGIC_LEN] == imx_codec_gif::MAGIC_87A
            || &bytes[..imx_codec_gif::MAGIC_LEN] == imx_codec_gif::MAGIC_89A)
    {
        return Ok(Format::Gif);
    }
    if bytes.len() >= imx_codec_png::MAGIC.len()
        && &bytes[..imx_codec_png::MAGIC.len()] == imx_codec_png::MAGIC
    {
        return Ok(Format::Png);
    }
    if bytes.len() >= imx_codec_tiff::MAGIC_LEN
        && (&bytes[..imx_codec_tiff::MAGIC_LEN] == imx_codec_tiff::MAGIC_LE
            || &bytes[..imx_codec_tiff::MAGIC_LEN] == imx_codec_tiff::MAGIC_BE)
    {
        return Ok(Format::Tiff);
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
            (Format::Gif, "identify") => "gif.identify_failed",
            (Format::Gif, "decode") => "gif.decode_failed",
            (Format::Webp, "identify") => "webp.identify_failed",
            (Format::Webp, "decode") => "webp.decode_failed",
            (Format::Tiff, "identify") => "tiff.identify_failed",
            (Format::Tiff, "decode") => "tiff.decode_failed",
            _ => err.diagnostic_code(),
        },
        _ => err.diagnostic_code(),
    }
}

fn detect_output_format(path: &CliPath<'_>) -> Result<Format, String> {
    if path.path == "-" {
        return path
            .prefix
            .ok_or_else(|| "stdout output (-) requires a format prefix, e.g. PNG:-".to_string());
    }
    let detected = detect_path_format(path.path)?;
    let detected = enforce_prefix(path, detected, "path format")?;
    Ok(detected)
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
        Some("gif") => Ok(Format::Gif),
        Some("jpg") | Some("jpeg") => Ok(Format::Jpeg),
        Some("pbm") => Ok(Format::Pbm),
        Some("pgm") => Ok(Format::Pgm),
        Some("png") => Ok(Format::Png),
        Some("ppm") => Ok(Format::Ppm),
        Some("qoi") => Ok(Format::Qoi),
        Some("tif") | Some("tiff") => Ok(Format::Tiff),
        Some("webp") => Ok(Format::Webp),
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
