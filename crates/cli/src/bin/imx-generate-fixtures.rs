use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use imx_core::{Image, PixelFormat};

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let output_dir = match args.as_slice() {
        [_, output_dir] => PathBuf::from(output_dir),
        _ => {
            eprintln!("usage: imx-generate-fixtures <output-dir>");
            process::exit(2);
        }
    };

    if let Err(err) = generate(&output_dir) {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn generate(output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output_dir)?;

    let gradient = gradient_rgba16be(64, 64)?;
    let gradient_ff = imx_codec_farbfeld::encode(&gradient)?;
    let gradient_rgba8 = gradient.to_rgba8()?.into_pixels();
    let gradient_rgb8 = gradient.to_rgb8()?.into_pixels();
    let gradient_qoi = imx_codec_qoi::encode_image(&gradient, imx_codec_qoi::QOI_SRGB)?;
    let gradient_ppm = imx_codec_ppm::encode(&gradient)?;
    let gradient_rgba16be = gradient.pixels().to_vec();

    let quantization = Image::new(
        2,
        2,
        PixelFormat::Rgba16Be,
        vec![
            0x00, 0x01, 0x12, 0x34, 0x7f, 0xff, 0xff, 0xfe, 0x01, 0x00, 0x80, 0x01, 0xaa, 0x55,
            0x40, 0x00, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0x11, 0x11, 0x22, 0x22,
            0x33, 0x33, 0x44, 0x44,
        ],
    )?;
    let quantization_ff = imx_codec_farbfeld::encode(&quantization)?;

    let qoi_rgba_pixels = [
        0, 255, 0, 255, 255, 0, 0, 128, 18, 52, 86, 120, 255, 255, 255, 0,
    ];
    let qoi_rgba = imx_codec_qoi::encode(2, 2, 4, imx_codec_qoi::QOI_SRGB, &qoi_rgba_pixels)?;
    let qoi_rgb_pixels = [0, 255, 0, 255, 0, 0, 18, 52, 86, 255, 255, 255];
    let qoi_rgb = imx_codec_qoi::encode(2, 2, 3, imx_codec_qoi::QOI_LINEAR, &qoi_rgb_pixels)?;

    let files = [
        ("gradient-64.ff", gradient_ff),
        ("gradient-64.qoi", gradient_qoi),
        ("gradient-64.ppm", gradient_ppm),
        ("gradient-64.rgba", gradient_rgba8),
        ("gradient-64.rgb", gradient_rgb8),
        ("gradient-64.rgba16be", gradient_rgba16be),
        ("quantization-2x2.ff", quantization_ff),
        ("qoi-rgba-2x2.qoi", qoi_rgba),
        ("qoi-rgb-2x2.qoi", qoi_rgb),
    ];

    let mut manifest = String::from("# IMX generated fixtures\n");
    let mut manifest_json = String::from("{\n  \"schema_version\": 1,\n  \"fixtures\": [\n");
    for (name, bytes) in files {
        let path = output_dir.join(name);
        fs::write(&path, &bytes)?;
        let hash = fnv64(&bytes);
        manifest.push_str(&format!(
            "{name} bytes={} fnv64={:016x}\n",
            bytes.len(),
            hash
        ));
        if !manifest_json.ends_with("[\n") {
            manifest_json.push_str(",\n");
        }
        manifest_json.push_str(&format!(
            "    {{ \"path\": \"{}\", \"bytes\": {}, \"fnv64\": \"{:016x}\" }}",
            json_escape(name),
            bytes.len(),
            hash
        ));
    }
    manifest_json.push_str("\n  ]\n}\n");
    fs::write(output_dir.join("manifest.txt"), manifest)?;
    fs::write(output_dir.join("manifest.json"), manifest_json)?;

    Ok(())
}

fn gradient_rgba16be(width: u32, height: u32) -> Result<Image, imx_core::ImageError> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 8);
    for y in 0..height {
        for x in 0..width {
            let red = ((x * 1021 + y * 17) & 0xffff) as u16;
            let green = ((x * 37 + y * 2039) & 0xffff) as u16;
            let blue = ((x * 499 + y * 313) & 0xffff) as u16;
            let alpha = (0xffff_u32 - ((x * 257 + y * 911) & 0xffff)) as u16;
            for channel in [red, green, blue, alpha] {
                pixels.extend_from_slice(&channel.to_be_bytes());
            }
        }
    }
    Image::new(width, height, PixelFormat::Rgba16Be, pixels)
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}
