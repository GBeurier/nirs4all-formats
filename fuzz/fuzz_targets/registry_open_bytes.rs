#![no_main]

use libfuzzer_sys::fuzz_target;

const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

// The selector lets the same bytes exercise extension-sensitive sniffers.
// Every name is static: fuzz input is never interpreted as a filesystem path.
const INPUT_NAMES: &[&str] = &[
    "input.jdx",
    "input.sp",
    "input.msa",
    "input.sam",
    "input.animl",
    "input.csv",
    "input.spc",
    "input.npy",
    "input.dpt",
];

fuzz_target!(|input: &[u8]| {
    let Some((&selector, payload)) = input.split_first() else {
        return;
    };
    if payload.len() > MAX_PAYLOAD_BYTES {
        return;
    }

    let name = INPUT_NAMES[usize::from(selector) % INPUT_NAMES.len()];
    let _ = nirs4all_formats::open_bytes(name, payload);
});
