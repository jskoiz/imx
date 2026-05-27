use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use imx_core::{Image, PixelFormat};

#[path = "../progressive_jpeg_fixtures.rs"]
mod progressive_jpeg_fixtures;

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
    let gradient_rgb16be = gradient.to_rgb16be()?.into_pixels();
    let gradient_qoi = imx_codec_qoi::encode_image(&gradient, imx_codec_qoi::QOI_SRGB)?;
    let gradient_jpeg = imx_codec_jpeg::encode(&gradient.to_rgb8()?)?;
    let gradient_png = imx_codec_png::encode(&gradient.to_rgba8()?)?;
    let gradient_png16 = imx_codec_png::encode(&gradient)?;
    let gradient_bmp = imx_codec_bmp::encode(&gradient.to_rgb8()?)?;
    let gradient_pbm = imx_codec_pnm::encode_pbm(&gradient)?;
    let gradient_ppm = imx_codec_pnm::encode_ppm(&gradient.to_rgb8()?)?;
    let gradient_ppm16 = imx_codec_pnm::encode_ppm(&gradient)?;
    let gradient_pgm = imx_codec_pnm::encode_pgm(&gradient)?;
    let gradient_gray8 = gradient.to_gray8()?.into_pixels();
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
    let pbm_ascii = b"P1\n# pbm 1=black 0=white\n5 3\n01010\n10101\n00110\n".to_vec();
    let pbm_ascii_gray = imx_codec_pnm::decode_pbm(&pbm_ascii)?
        .to_gray8()?
        .into_pixels();
    let pbm_binary = b"P4\n10 2\n\x55\x40\xcc\x80".to_vec();
    let pbm_binary_gray = imx_codec_pnm::decode_pbm(&pbm_binary)?
        .to_gray8()?
        .into_pixels();
    let threshold = Image::new(
        4,
        1,
        PixelFormat::Rgba16Be,
        vec![
            0, 0, 0, 0, 0, 0, 0xff, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0xff, 0xff, 0x80,
            0x00, 0x80, 0x00, 0x80, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff,
        ],
    )?;
    let threshold_ff = imx_codec_farbfeld::encode(&threshold)?;
    let threshold_pbm = imx_codec_pnm::encode_pbm(&threshold)?;

    let qoi_rgba_pixels = [
        0, 255, 0, 255, 255, 0, 0, 128, 18, 52, 86, 120, 255, 255, 255, 0,
    ];
    let qoi_rgba = imx_codec_qoi::encode(2, 2, 4, imx_codec_qoi::QOI_SRGB, &qoi_rgba_pixels)?;
    let qoi_rgb_pixels = [0, 255, 0, 255, 0, 0, 18, 52, 86, 255, 255, 255];
    let qoi_rgb = imx_codec_qoi::encode(2, 2, 3, imx_codec_qoi::QOI_LINEAR, &qoi_rgb_pixels)?;
    let gray_jpeg = imx_codec_jpeg::encode(&Image::new(
        4,
        1,
        PixelFormat::Gray8,
        vec![0, 85, 170, 255],
    )?)?;
    let photo_orientation = Image::new(
        3,
        2,
        PixelFormat::Rgb8,
        vec![
            32, 64, 96, 96, 128, 160, 160, 192, 224, 224, 192, 160, 160, 128, 96, 96, 64, 32,
        ],
    )?;
    let photo_orientation_jpeg = imx_codec_jpeg::encode(&photo_orientation)?;
    let orientation_1 = jpeg_with_exif_orientation(&photo_orientation_jpeg, 1)?;
    let orientation_2 = jpeg_with_exif_orientation(&photo_orientation_jpeg, 2)?;
    let orientation_3 = jpeg_with_exif_orientation(&photo_orientation_jpeg, 3)?;
    let orientation_4 = jpeg_with_exif_orientation(&photo_orientation_jpeg, 4)?;
    let orientation_5 = jpeg_with_exif_orientation(&photo_orientation_jpeg, 5)?;
    let orientation_6 = jpeg_with_exif_orientation(&photo_orientation_jpeg, 6)?;
    let orientation_7 = jpeg_with_exif_orientation(&photo_orientation_jpeg, 7)?;
    let orientation_8 = jpeg_with_exif_orientation(&photo_orientation_jpeg, 8)?;
    let progressive_rgb_jpeg = progressive_jpeg_fixtures::progressive_rgb_jpeg();
    let progressive_gray_jpeg = progressive_jpeg_fixtures::progressive_gray_jpeg();
    assert!(progressive_jpeg_fixtures::is_progressive_jpeg(
        &progressive_rgb_jpeg
    ));
    assert!(progressive_jpeg_fixtures::is_progressive_jpeg(
        &progressive_gray_jpeg
    ));
    let progressive_orientation_6 = jpeg_with_exif_orientation(&progressive_rgb_jpeg, 6)?;
    let intake_farbfeld = imx_codec_farbfeld::encode(&Image::new(
        2,
        2,
        PixelFormat::Rgba16Be,
        vec![
            0x00, 0x01, 0x12, 0x34, 0x7f, 0xff, 0xff, 0xfe, 0x01, 0x00, 0x80, 0x01, 0xaa, 0x55,
            0x40, 0x00, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0x11, 0x11, 0x22, 0x22,
            0x33, 0x33, 0x44, 0x44,
        ],
    )?)?;
    let intake_qoi_rgb_linear = imx_codec_qoi::encode(
        2,
        2,
        3,
        imx_codec_qoi::QOI_LINEAR,
        &[0, 255, 0, 255, 0, 0, 18, 52, 86, 255, 255, 255],
    )?;
    let intake_ppm_comments =
        b"P3\n# v0.12 intake fixture: comments, whitespace, maxval 1023\n2 1\n1023\n0 512 1023\n1023 256 128\n"
            .to_vec();
    let intake_pgm16 = b"P5\n2 1\n65535\n\x12\x34\xff\xff".to_vec();
    let intake_png_rgba16 = imx_codec_png::encode(&Image::new(
        1,
        1,
        PixelFormat::Rgba16Be,
        vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0],
    )?)?;
    let intake_bmp_rgb24 = imx_codec_bmp::encode(&Image::new(
        3,
        2,
        PixelFormat::Rgb8,
        vec![
            255, 0, 0, 0, 255, 0, 0, 0, 255, 12, 34, 56, 78, 90, 123, 222, 111, 3,
        ],
    )?)?;
    let intake_bmp_rgba32 = imx_codec_bmp::encode(&Image::new(
        2,
        2,
        PixelFormat::Rgba8,
        vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 12, 34, 56, 0],
    )?)?;

    let files = [
        ("gradient-64.ff", gradient_ff),
        ("gradient-64.jpg", gradient_jpeg),
        ("gradient-64.qoi", gradient_qoi),
        ("gradient-64.png", gradient_png),
        ("gradient-64-png16.png", gradient_png16),
        ("gradient-64.bmp", gradient_bmp),
        ("gradient-64.pbm", gradient_pbm),
        ("gradient-64.ppm", gradient_ppm),
        ("gradient-64-ppm16.ppm", gradient_ppm16),
        ("gradient-64.pgm", gradient_pgm),
        ("gradient-64.rgba", gradient_rgba8),
        ("gradient-64.rgb", gradient_rgb8),
        ("gradient-64.rgb16be", gradient_rgb16be),
        ("gradient-64.gray", gradient_gray8),
        ("gradient-64.rgba16be", gradient_rgba16be),
        ("quantization-2x2.ff", quantization_ff),
        ("pbm-ascii-5x3.pbm", pbm_ascii),
        ("pbm-ascii-5x3.gray", pbm_ascii_gray),
        ("pbm-binary-10x2.pbm", pbm_binary),
        ("pbm-binary-10x2.gray", pbm_binary_gray),
        ("pbm-threshold-4x1.ff", threshold_ff),
        ("pbm-threshold-4x1.pbm", threshold_pbm),
        ("qoi-rgba-2x2.qoi", qoi_rgba),
        ("qoi-rgb-2x2.qoi", qoi_rgb),
        ("gray-4x1.jpg", gray_jpeg),
        ("progressive-rgb-4x3.jpg", progressive_rgb_jpeg),
        ("progressive-gray-4x2.jpg", progressive_gray_jpeg),
        ("progressive-orientation-o6.jpg", progressive_orientation_6),
        ("photo-orientation-o1.jpg", orientation_1),
        ("photo-orientation-o2.jpg", orientation_2),
        ("photo-orientation-o3.jpg", orientation_3),
        ("photo-orientation-o4.jpg", orientation_4),
        ("photo-orientation-o5.jpg", orientation_5),
        ("photo-orientation-o6.jpg", orientation_6),
        ("photo-orientation-o7.jpg", orientation_7),
        ("photo-orientation-o8.jpg", orientation_8),
        ("intake-farbfeld-rgba16-2x2.ff", intake_farbfeld),
        ("intake-qoi-rgb-linear-2x2.qoi", intake_qoi_rgb_linear),
        ("intake-comments-2x1.ppm", intake_ppm_comments),
        ("intake-pgm16-2x1.pgm", intake_pgm16),
        ("intake-rgba16-1x1.png", intake_png_rgba16),
        ("intake-rgb24-3x2.bmp", intake_bmp_rgb24),
        ("intake-rgba32-2x2.bmp", intake_bmp_rgba32),
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

fn jpeg_with_exif_orientation(
    jpeg: &[u8],
    orientation: u16,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut app1 = Vec::from(b"Exif\0\0MM\0*\0\0\0\x08".as_slice());
    app1.extend_from_slice(&1_u16.to_be_bytes());
    app1.extend_from_slice(&0x0112_u16.to_be_bytes());
    app1.extend_from_slice(&3_u16.to_be_bytes());
    app1.extend_from_slice(&1_u32.to_be_bytes());
    app1.extend_from_slice(&orientation.to_be_bytes());
    app1.extend_from_slice(&[0, 0]);
    app1.extend_from_slice(&0_u32.to_be_bytes());

    let segment_len = u16::try_from(app1.len() + 2)?;
    let mut out = Vec::new();
    out.extend_from_slice(&jpeg[..2]);
    out.extend_from_slice(&[0xff, 0xe1]);
    out.extend_from_slice(&segment_len.to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&jpeg[2..]);
    Ok(out)
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
