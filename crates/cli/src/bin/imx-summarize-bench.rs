use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let bench_dir = match args.as_slice() {
        [_, bench_dir] => PathBuf::from(bench_dir),
        _ => {
            eprintln!("usage: imx-summarize-bench <bench-dir>");
            process::exit(2);
        }
    };

    if let Err(err) = summarize(&bench_dir) {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn summarize(bench_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = fs::read_to_string(bench_dir.join("metadata.txt"))?;
    let library = fs::read_to_string(bench_dir.join("standalone-library-bench.stdout"))?;
    let mut measurements = Vec::new();

    for entry in fs::read_dir(bench_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("time") {
            continue;
        }
        let label = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("invalid time file name")?
            .to_string();
        let raw = fs::read_to_string(&path)?;
        let parsed = parse_time_output(&raw);
        measurements.push(Measurement {
            label,
            real_seconds: parsed.real_seconds,
            max_rss_bytes: parsed.max_rss_bytes,
        });
    }
    measurements.sort_by(|a, b| a.label.cmp(&b.label));

    let mut jsonl = String::new();
    for measurement in &measurements {
        jsonl.push_str(&format!(
            "{{\"schema_version\":1,\"case_id\":\"{}\",\"real_seconds\":{},\"max_rss_bytes\":{}}}\n",
            json_escape(&measurement.label),
            json_number(measurement.real_seconds),
            json_u64(measurement.max_rss_bytes)
        ));
    }
    fs::write(bench_dir.join("measurements.jsonl"), jsonl)?;

    let mut run_json = String::from("{\n  \"schema_version\": 1,\n");
    let mut metadata_lines = Vec::new();
    for line in metadata.lines() {
        metadata_lines.push(line);
        if let Some((key, value)) = line.split_once('=') {
            if !matches!(key, "date" | "uname" | "standalone" | "oracle") {
                continue;
            }
            run_json.push_str(&format!(
                "  \"{}\": \"{}\",\n",
                json_escape(key),
                json_escape(value)
            ));
        }
    }
    run_json.push_str("  \"metadata_path\": \"metadata.txt\",\n  \"metadata_lines\": [\n");
    for (index, line) in metadata_lines.iter().enumerate() {
        if index > 0 {
            run_json.push_str(",\n");
        }
        run_json.push_str(&format!("    \"{}\"", json_escape(line)));
    }
    run_json.push_str("\n  ]\n}\n");
    fs::write(bench_dir.join("benchmark-run.json"), run_json)?;

    let mut summary = String::from("{\n  \"schema_version\": 1,\n  \"library\": {\n");
    let mut first_metric = true;
    for line in library.lines() {
        for token in line.split_whitespace() {
            if let Some((key, value)) = token.split_once('=') {
                if !first_metric {
                    summary.push_str(",\n");
                }
                first_metric = false;
                summary.push_str(&format!(
                    "    \"{}\": {}",
                    json_escape(key),
                    json_numeric_or_string(value)
                ));
            }
        }
    }
    summary.push_str("\n  },\n  \"process_measurements\": [\n");
    for (index, measurement) in measurements.iter().enumerate() {
        if index > 0 {
            summary.push_str(",\n");
        }
        summary.push_str(&format!(
            "    {{ \"case_id\": \"{}\", \"real_seconds\": {}, \"max_rss_bytes\": {} }}",
            json_escape(&measurement.label),
            json_number(measurement.real_seconds),
            json_u64(measurement.max_rss_bytes)
        ));
    }
    summary.push_str("\n  ]\n}\n");
    fs::write(bench_dir.join("summary.json"), summary)?;

    Ok(())
}

struct Measurement {
    label: String,
    real_seconds: Option<f64>,
    max_rss_bytes: Option<u64>,
}

#[derive(Default)]
struct ParsedTime {
    real_seconds: Option<f64>,
    max_rss_bytes: Option<u64>,
}

fn parse_time_output(raw: &str) -> ParsedTime {
    let mut parsed = ParsedTime::default();
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_suffix(" maximum resident set size") {
            parsed.max_rss_bytes = value.trim().parse::<u64>().ok();
        } else if trimmed.contains("Maximum resident set size") {
            if let Some((_, value)) = trimmed.rsplit_once(':') {
                parsed.max_rss_bytes = value
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .map(|kilobytes| kilobytes * 1024);
            }
        } else if trimmed.contains(" real") {
            parsed.real_seconds = trimmed
                .split_whitespace()
                .next()
                .and_then(|number| number.parse::<f64>().ok());
        } else if trimmed.contains("Elapsed (wall clock) time") {
            if let Some((_, value)) = trimmed.rsplit_once(':') {
                parsed.real_seconds = parse_elapsed(value.trim());
            }
        }
    }
    parsed
}

fn parse_elapsed(value: &str) -> Option<f64> {
    let parts = value.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [seconds] => seconds.parse().ok(),
        [minutes, seconds] => {
            let minutes = minutes.parse::<f64>().ok()?;
            let seconds = seconds.parse::<f64>().ok()?;
            Some(minutes * 60.0 + seconds)
        }
        [hours, minutes, seconds] => {
            let hours = hours.parse::<f64>().ok()?;
            let minutes = minutes.parse::<f64>().ok()?;
            let seconds = seconds.parse::<f64>().ok()?;
            Some(hours * 3600.0 + minutes * 60.0 + seconds)
        }
        _ => None,
    }
}

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn json_number(value: Option<f64>) -> String {
    value
        .filter(|number| number.is_finite())
        .map(|number| format!("{number:.6}"))
        .unwrap_or_else(|| "null".to_string())
}

fn json_u64(value: Option<u64>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn json_numeric_or_string(value: &str) -> String {
    if value.parse::<f64>().is_ok() {
        value.to_string()
    } else {
        format!("\"{}\"", json_escape(value))
    }
}
