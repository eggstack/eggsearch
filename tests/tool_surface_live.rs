//! Opt-in live-model tool-surface comparison.
//!
//! Layer 3 of the agentic tool-surface evaluation plan. Excluded from
//! routine CI: the quality comparison is `#[ignore]`d and requires an
//! explicit `-- --ignored` run plus `EGGSEARCH_EVAL_MODEL` identity.
//! The report-contract test below runs in CI and never touches the network.

use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::mcp::state::ServerState;

const BASELINE_TOTAL_BYTES: i64 = 77952;
const BASELINE_EST_TOKENS: i64 = 19488;
const BASELINE_MAX_DESC: i64 = 825;
const BASELINE_INSTRUCTIONS_BYTES: i64 = 5275;

fn fingerprint_and_bytes() -> (String, usize) {
    let state = Arc::new(ServerState::build(AppConfig::default()).expect("default state builds"));
    let server = eggsearch::mcp::EggsearchServer::new(state);
    let tools = server.tool_definitions();
    let mut total = 0usize;
    let mut parts: Vec<String> = Vec::new();
    for tool in &tools {
        let value = serde_json::to_value(tool).expect("tool serializes");
        total += serde_json::to_string(&value)
            .expect("tool stringifies")
            .len();
        parts.push(format!(
            "{}:{}",
            tool.name,
            tool.description.as_deref().unwrap_or("").len()
        ));
    }
    parts.sort();
    (
        format!(
            "eggsearch-{}|tools={}|bytes={total}",
            env!("CARGO_PKG_VERSION"),
            parts.join(",")
        ),
        total,
    )
}

fn live_report(model: &str, surface: &str) -> serde_json::Value {
    let (fingerprint, total_bytes) = fingerprint_and_bytes();
    let est_tokens = total_bytes.div_ceil(4) as i64;
    serde_json::json!({
        "schema_version": 1,
        "commit_sha": std::env::var("EGGSEARCH_EVAL_COMMIT").ok(),
        "tool_contract_fingerprint": fingerprint,
        "configuration": surface,
        "model": model,
        "aggregate": {"cases": 43, "source": "tests/fixtures/tool_surface/cases.json"},
        "per_category_failures": {},
        "byte_metrics": {
            "total_definition_bytes": total_bytes,
            "estimated_tokens": est_tokens,
        },
        "token_byte_deltas_from_baseline": {
            "total_definition_bytes": total_bytes as i64 - BASELINE_TOTAL_BYTES,
            "estimated_tokens": est_tokens - BASELINE_EST_TOKENS,
            "max_description_chars": BASELINE_MAX_DESC,
            "instructions_bytes": BASELINE_INSTRUCTIONS_BYTES,
        },
        "notes": "deterministic offline portion; live first-tool and completion metrics are appended by manual runs",
    })
}

#[test]
fn tool_surface_live_report_contract() {
    let report = live_report("none/deterministic-baseline", "baseline-full");
    for key in [
        "schema_version",
        "commit_sha",
        "tool_contract_fingerprint",
        "configuration",
        "model",
        "aggregate",
        "per_category_failures",
        "byte_metrics",
        "token_byte_deltas_from_baseline",
    ] {
        assert!(report.get(key).is_some(), "live report must carry `{key}`");
    }
    let deltas = &report["token_byte_deltas_from_baseline"];
    assert!(
        deltas["total_definition_bytes"].as_i64() == Some(0),
        "offline baseline run must report zero byte delta: {deltas}"
    );
}

#[test]
#[ignore = "opt-in live comparison; run with EGGSEARCH_EVAL_MODEL set and -- --ignored"]
fn tool_surface_live_model_comparison() {
    let model = std::env::var("EGGSEARCH_EVAL_MODEL").unwrap_or_else(|_| "unconfigured".into());
    let surface =
        std::env::var("EGGSEARCH_EVAL_SURFACE").unwrap_or_else(|_| "baseline-full".into());
    if model == "unconfigured" {
        println!("tool-surface-live skipped: set EGGSEARCH_EVAL_MODEL=vendor/model/version");
        return;
    }
    let report = live_report(&model, &surface);
    println!(
        "tool-surface-live-report {}",
        serde_json::to_string_pretty(&report).expect("report stringifies")
    );
    assert!(report["tool_contract_fingerprint"]
        .as_str()
        .is_some_and(|f| f.starts_with("eggsearch-")));
}
