#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = data.trim().parse::<u64>();
    let _ = data.trim().parse::<usize>();
    let _ = data.trim().parse::<i64>();
});
