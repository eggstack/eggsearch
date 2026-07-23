#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::core::workflow_coverage::WorkflowKind;

fuzz_target!(|data: &str| {
    let _ = WorkflowKind::parse(data);
});
