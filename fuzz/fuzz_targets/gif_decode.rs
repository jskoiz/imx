#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = imx_codec_gif::identify(data);
    let _ = imx_codec_gif::decode(data);
});
