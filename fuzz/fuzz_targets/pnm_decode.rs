#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = imx_codec_pnm::identify_ppm(data);
    let _ = imx_codec_pnm::identify_pgm(data);
    let _ = imx_codec_pnm::decode_ppm(data);
    let _ = imx_codec_pnm::decode_pgm(data);
});
