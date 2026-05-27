use std::hint::black_box;
use std::time::Instant;

use imx_core::{Image, PixelFormat};

fn fixture(width: u32, height: u32) -> Image {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 8);
    for y in 0..height {
        for x in 0..width {
            for value in [
                ((x * 17 + y * 3) & 0xff) as u8,
                ((x * 5 + y * 29) & 0xff) as u8,
                ((x * 11 + y * 7) & 0xff) as u8,
                (255 - ((x * 13 + y * 19) & 0xff)) as u8,
            ] {
                pixels.push(value);
                pixels.push(value);
            }
        }
    }
    Image::new(width, height, PixelFormat::Rgba16Be, pixels).unwrap()
}

fn time(label: &str, bytes: usize, iterations: usize, mut f: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed().as_secs_f64();
    let mib_s = (bytes as f64 * iterations as f64 / (1024.0 * 1024.0)) / elapsed;
    println!("{label}_secs={elapsed:.6} {label}_mib_s={mib_s:.2}");
}

fn main() {
    let iterations = std::env::var("IMX_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(50);
    let image = fixture(256, 256);
    let ff = imx_codec_farbfeld::encode(&image).unwrap();
    let opaque_rgb = image.to_rgb8().unwrap();
    let opaque_ff = imx_codec_farbfeld::encode(&opaque_rgb).unwrap();
    let jpeg = imx_codec_jpeg::encode(&opaque_rgb).unwrap();
    let qoi = imx_codec_qoi::encode_image(&image, imx_codec_qoi::QOI_SRGB).unwrap();
    let png = imx_codec_png::encode(&image.to_rgba8().unwrap()).unwrap();
    let png16 = imx_codec_png::encode(&image).unwrap();
    let pbm = imx_codec_pnm::encode_pbm(&image).unwrap();
    let ppm = imx_codec_pnm::encode_ppm(&image.to_rgb8().unwrap()).unwrap();
    let ppm16 = imx_codec_pnm::encode_ppm(&image).unwrap();
    let pgm = imx_codec_pnm::encode_pgm(&image).unwrap();

    println!("iterations={iterations}");
    println!("farbfeld_bytes={}", ff.len());
    println!("jpeg_bytes={}", jpeg.len());
    println!("qoi_bytes={}", qoi.len());
    println!("png_bytes={}", png.len());
    println!("png16_bytes={}", png16.len());
    println!("pbm_bytes={}", pbm.len());
    println!("ppm_bytes={}", ppm.len());
    println!("ppm16_bytes={}", ppm16.len());
    println!("pgm_bytes={}", pgm.len());

    time("farbfeld_decode", ff.len(), iterations, || {
        black_box(imx_codec_farbfeld::decode(black_box(&ff)).unwrap());
    });
    time("farbfeld_encode", ff.len(), iterations, || {
        black_box(imx_codec_farbfeld::encode(black_box(&image)).unwrap());
    });
    time("qoi_decode", qoi.len(), iterations, || {
        black_box(imx_codec_qoi::decode(black_box(&qoi)).unwrap());
    });
    time("qoi_encode", qoi.len(), iterations, || {
        black_box(imx_codec_qoi::encode_image(black_box(&image), imx_codec_qoi::QOI_SRGB).unwrap());
    });
    time("jpeg_decode", jpeg.len(), iterations, || {
        black_box(imx_codec_jpeg::decode(black_box(&jpeg)).unwrap());
    });
    time("jpeg_encode", jpeg.len(), iterations, || {
        black_box(imx_codec_jpeg::encode(black_box(&opaque_rgb)).unwrap());
    });
    time("png_decode", png.len(), iterations, || {
        black_box(imx_codec_png::decode(black_box(&png)).unwrap());
    });
    time("png_encode", png.len(), iterations, || {
        let rgba = image.to_rgba8().unwrap();
        black_box(imx_codec_png::encode(black_box(&rgba)).unwrap());
    });
    time("png16_decode", png16.len(), iterations, || {
        black_box(imx_codec_png::decode(black_box(&png16)).unwrap());
    });
    time("png16_encode", png16.len(), iterations, || {
        black_box(imx_codec_png::encode(black_box(&image)).unwrap());
    });
    time("ppm_decode", ppm.len(), iterations, || {
        black_box(imx_codec_pnm::decode_ppm(black_box(&ppm)).unwrap());
    });
    time("ppm_encode", ppm.len(), iterations, || {
        let rgb = image.to_rgb8().unwrap();
        black_box(imx_codec_pnm::encode_ppm(black_box(&rgb)).unwrap());
    });
    time("ppm16_decode", ppm16.len(), iterations, || {
        black_box(imx_codec_pnm::decode_ppm(black_box(&ppm16)).unwrap());
    });
    time("ppm16_encode", ppm16.len(), iterations, || {
        black_box(imx_codec_pnm::encode_ppm(black_box(&image)).unwrap());
    });
    time("pbm_decode", pbm.len(), iterations, || {
        black_box(imx_codec_pnm::decode_pbm(black_box(&pbm)).unwrap());
    });
    time("pbm_encode", pbm.len(), iterations, || {
        black_box(imx_codec_pnm::encode_pbm(black_box(&image)).unwrap());
    });
    time("pgm_decode", pgm.len(), iterations, || {
        black_box(imx_codec_pnm::decode_pgm(black_box(&pgm)).unwrap());
    });
    time("pgm_encode", pgm.len(), iterations, || {
        black_box(imx_codec_pnm::encode_pgm(black_box(&image)).unwrap());
    });
    time("ff_to_qoi", ff.len(), iterations, || {
        let decoded = imx_codec_farbfeld::decode(black_box(&ff)).unwrap();
        black_box(imx_codec_qoi::encode_image(&decoded, imx_codec_qoi::QOI_SRGB).unwrap());
    });
    time("qoi_to_ff", qoi.len(), iterations, || {
        let decoded = imx_codec_qoi::decode(black_box(&qoi))
            .and_then(|decoded| decoded.into_core_image())
            .unwrap();
        black_box(imx_codec_farbfeld::encode(&decoded).unwrap());
    });
    time("jpeg_to_ff", jpeg.len(), iterations, || {
        let decoded = imx_codec_jpeg::decode(black_box(&jpeg)).unwrap();
        black_box(imx_codec_farbfeld::encode(&decoded).unwrap());
    });
    time("ff_to_jpeg", opaque_ff.len(), iterations, || {
        let decoded = imx_codec_farbfeld::decode(black_box(&opaque_ff)).unwrap();
        black_box(imx_codec_jpeg::encode(&decoded).unwrap());
    });
    time("png_to_ff", png.len(), iterations, || {
        let decoded = imx_codec_png::decode(black_box(&png)).unwrap();
        black_box(imx_codec_farbfeld::encode(&decoded).unwrap());
    });
    time("ff_to_png", ff.len(), iterations, || {
        let decoded = imx_codec_farbfeld::decode(black_box(&ff)).unwrap();
        black_box(imx_codec_png::encode(&decoded).unwrap());
    });
    time("ppm_to_ff", ppm.len(), iterations, || {
        let decoded = imx_codec_pnm::decode_ppm(black_box(&ppm)).unwrap();
        black_box(imx_codec_farbfeld::encode(&decoded).unwrap());
    });
    time("ff_to_ppm", ff.len(), iterations, || {
        let decoded = imx_codec_farbfeld::decode(black_box(&ff)).unwrap();
        black_box(imx_codec_pnm::encode_ppm(&decoded).unwrap());
    });
    time("ppm16_to_ff", ppm16.len(), iterations, || {
        let decoded = imx_codec_pnm::decode_ppm(black_box(&ppm16)).unwrap();
        black_box(imx_codec_farbfeld::encode(&decoded).unwrap());
    });
    time("pbm_to_ff", pbm.len(), iterations, || {
        let decoded = imx_codec_pnm::decode_pbm(black_box(&pbm)).unwrap();
        black_box(imx_codec_farbfeld::encode(&decoded).unwrap());
    });
    time("ff_to_pbm", ff.len(), iterations, || {
        let decoded = imx_codec_farbfeld::decode(black_box(&ff)).unwrap();
        black_box(imx_codec_pnm::encode_pbm(&decoded).unwrap());
    });
    time("pgm_to_ff", pgm.len(), iterations, || {
        let decoded = imx_codec_pnm::decode_pgm(black_box(&pgm)).unwrap();
        black_box(imx_codec_farbfeld::encode(&decoded).unwrap());
    });
    time("ff_to_pgm", ff.len(), iterations, || {
        let decoded = imx_codec_farbfeld::decode(black_box(&ff)).unwrap();
        black_box(imx_codec_pnm::encode_pgm(&decoded).unwrap());
    });
    time("resize_nearest", image.pixels().len(), iterations, || {
        black_box(black_box(&image).resize_nearest(128, 96).unwrap());
    });

    println!("max_rss_bytes={}", max_rss_bytes().unwrap_or(0));
}

fn max_rss_bytes() -> Option<u64> {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
        if status != 0 {
            return None;
        }
        let usage = unsafe { usage.assume_init() };
        let value = usage.ru_maxrss as u64;
        #[cfg(target_os = "macos")]
        {
            Some(value)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Some(value * 1024)
        }
    }
    #[cfg(not(unix))]
    {
        None
    }
}
