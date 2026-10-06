#![no_main]

use libfuzzer_sys::fuzz_target;
use eggsearch::fetch::limits::{validate_url, FetchLimits};

// Drives a redirect hop list through the shape validator: every hop must pass
// or the chain aborts, mirroring how the redirect loop bails on the first bad
// `Location`.
//
// Scope: this covers the synchronous shape rules (scheme, credentials,
// private IP, localhost). It does not reach the async per-hop DNS resolution
// in `validate_fetch_target`, which needs real name resolution and is covered
// by unit tests in `src/fetch/limits.rs`.
fuzz_target!(|data: &str| {
    // Default limits: `allow_private_network` / `allow_localhost` are false, so
    // the private-IP, localhost, and credential rules actually engage.
    let limits = FetchLimits::default();

    let mut hops: Vec<String> = Vec::new();
    let mut last_result = None;
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let result = validate_url(line, &limits);
        if result.is_err() {
            break;
        }
        let url = result.expect("checked above");
        assert!(
            matches!(url.scheme(), "http" | "https"),
            "accepted hop must be http(s), got {:?}",
            url.scheme()
        );
        assert!(
            url.username().is_empty() && url.password().is_none(),
            "accepted hop must not carry credentials"
        );
        hops.push(url.to_string());
        last_result = Some(url);
    }

    // The chain stops at the first rejected hop, so accepted hops can never
    // outnumber the non-empty lines supplied.
    let non_empty_lines = data.lines().filter(|l| !l.trim().is_empty()).count();
    assert!(hops.len() <= non_empty_lines);
    assert_eq!(last_result.is_some(), !hops.is_empty());
});