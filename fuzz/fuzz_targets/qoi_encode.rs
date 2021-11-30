#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (bool, u8, &[u8])| {
    let (is_4, w_frac, data) = input;
    let channels = if is_4 { 4 } else { 3 };
    let size = data.len();
    let n_pixels = size / channels as usize;
    let (w, h) = if n_pixels == 0 {
        (0, 0)
    } else {
        let w = ((n_pixels * (1 + w_frac as usize)) / 256).max(1);
        let h = n_pixels / w;
        (w, h)
    };
    let out = qoi_fast::qoi_encode_to_vec(
        &data[..(w * h * channels as usize)],
        w as u32,
        h as u32,
        channels,
        0,
    );
    if w * h != 0 {
        let out = out.unwrap();
        assert!(out.len() <= qoi_fast::encode_size_required(w as u32, h as u32, channels));
    } else {
        assert!(out.is_err());
    }
});
