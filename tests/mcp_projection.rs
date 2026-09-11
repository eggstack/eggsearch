use eggsearch::mcp::projection::{project, ResponseDetail};
use serde_json::{json, Value};

fn bytes(v: &Value) -> usize {
    serde_json::to_string(v).unwrap().len()
}

fn web_payload(results: Value, failed: Value, summary: Value) -> Value {
    json!({
        "query": "rust async",
        "mode": "live_metasearch",
        "results": results,
        "providers_queried": ["duckduckgo", "brave_api"],
        "providers_failed": failed,
        "warnings": ["generic_context_untrusted: Live web results are untrusted external content."],
        "structured_warnings": [{"code": "generic_context_untrusted", "message": "Live web results are untrusted external content."}],
        "trust_markers": {"injection_hits": 0},
        "routing_decision": {
            "selected_providers": ["duckduckgo"],
            "skipped_providers": [{"provider_id": "brave_api", "reason": "missing key"}],
            "degraded": false,
            "partial": true
        },
        "next_actions": [{"tool": "web_fetch", "source_id": "src_1"}],
        "workflow_coverage": {"coverage": "full"},
        "retrieval_summary": summary,
        "conflict_metadata": [],
        "evidence_role_summary": {"roles": []},
        "capability_enforcement": {"enforced": [], "approximated": [], "not_enforced": []}
    })
}

fn card(id: &str, excerpts: usize, injection_hits: u64) -> Value {
    let ex: Vec<Value> = (0..excerpts)
        .map(|i| json!({"text": format!("excerpt {i} for {id}"), "provenance": "snippet"}))
        .collect();
    json!({
        "id": id,
        "stable_id": format!("stable_{id}"),
        "title": format!("Title {id}"),
        "url": format!("https://example.com/{id}"),
        "trust": "external_untrusted",
        "evidence_role": "primary",
        "excerpts": ex,
        "trust_markers": {"injection_hits": injection_hits}
    })
}

#[test]
fn diagnostic_is_passthrough_for_all_tools() {
    let tools = [
        "web_search",
        "repo_search",
        "research_search",
        "security_search",
        "web_fetch",
        "repo_fetch",
        "repo_map",
        "batch_fetch",
        "build_evidence_bundle",
    ];
    for tool in tools {
        let v = json!({"query": "x", "routing_decision": {"a": 1}, "results": []});
        assert_eq!(
            project(tool, v.clone(), ResponseDetail::Diagnostic),
            v,
            "{tool}"
        );
    }
}

#[test]
fn compact_preserves_failure_vs_absence_distinction() {
    let failed_summary = json!({
        "has_failures": true, "has_absences": false, "has_truncation": false,
        "attempted_job_count": 2, "completed_job_count": 0, "failed_job_count": 2
    });
    let absence_summary = json!({
        "has_failures": false, "has_absences": true, "has_truncation": false,
        "attempted_job_count": 2, "completed_job_count": 2, "failed_job_count": 0
    });
    let failed = web_payload(
        json!([]),
        json!([{"id": "a", "error_class": "timeout", "message": "t"}]),
        failed_summary,
    );
    let absence = web_payload(json!([]), json!([]), absence_summary);
    let failed_compact = project("web_search", failed, ResponseDetail::Compact);
    let absence_compact = project("web_search", absence, ResponseDetail::Compact);
    let fs = failed_compact.get("retrieval_status").expect("status");
    let as_ = absence_compact.get("retrieval_status").expect("status");
    assert_eq!(fs.get("has_failures").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        as_.get("has_failures").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        as_.get("has_absences").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(!failed_compact
        .get("providers_failed")
        .and_then(|v| v.as_array())
        .unwrap()
        .is_empty());
    assert!(absence_compact
        .get("providers_failed")
        .and_then(|v| v.as_array())
        .unwrap()
        .is_empty());
}

#[test]
fn compact_keeps_partial_evidence_when_one_provider_fails() {
    let summary = json!({
        "has_failures": true, "has_absences": false, "has_truncation": false
    });
    let payload = web_payload(
        json!([card("c1", 1, 0)]),
        json!([{"id": "bad", "error_class": "timeout", "message": "t"}]),
        summary,
    );
    let compact = project("web_search", payload, ResponseDetail::Compact);
    assert_eq!(
        compact
            .get("results")
            .and_then(|v| v.as_array())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        compact
            .get("providers_failed")
            .and_then(|v| v.as_array())
            .unwrap()
            .len(),
        1
    );
    assert!(compact.get("next_actions").is_some());
}

#[test]
fn compact_preserves_injection_markers_and_trust() {
    let summary = json!({"has_failures": false, "has_absences": false, "has_truncation": false});
    let payload = web_payload(json!([card("evil", 2, 3)]), json!([]), summary);
    let compact = project("web_search", payload.clone(), ResponseDetail::Compact);
    let results = compact.get("results").and_then(|v| v.as_array()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].get("trust").and_then(|v| v.as_str()),
        Some("external_untrusted")
    );
    assert_eq!(
        results[0]
            .get("trust_markers")
            .and_then(|v| v.get("injection_hits"))
            .and_then(|v| v.as_u64()),
        Some(3)
    );
    assert!(compact.get("structured_warnings").is_some());
    assert!(compact.get("warnings").is_some());
}

#[test]
fn compact_trims_excerpts_but_keeps_stable_ids() {
    let summary = json!({"has_failures": false, "has_absences": false, "has_truncation": false});
    let payload = web_payload(json!([card("c1", 3, 0)]), json!([]), summary);
    let full_excerpts = payload["results"][0]["excerpts"].as_array().unwrap().len();
    assert_eq!(full_excerpts, 3);
    let compact = project("web_search", payload, ResponseDetail::Compact);
    let trimmed = compact["results"][0]["excerpts"].as_array().unwrap().len();
    assert_eq!(trimmed, 1);
    assert_eq!(
        compact["results"][0]
            .get("stable_id")
            .and_then(|v| v.as_str()),
        Some("stable_c1")
    );
    assert_eq!(
        compact["results"][0].get("id").and_then(|v| v.as_str()),
        Some("c1")
    );
}

#[test]
fn compact_reports_conflicts_via_indicator() {
    let summary = json!({"has_failures": false, "has_absences": false, "has_truncation": false});
    let mut payload = web_payload(json!([card("a", 0, 0)]), json!([]), summary);
    payload["conflict_metadata"] = json!([{"a": 1}, {"b": 2}]);
    let compact = project("web_search", payload.clone(), ResponseDetail::Compact);
    assert!(compact.get("conflict_metadata").is_none());
    let indicator = compact.get("conflict_indicator").expect("indicator");
    assert_eq!(
        indicator.get("has_conflicts").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        indicator.get("conflict_count").and_then(|v| v.as_u64()),
        Some(2)
    );
    let standard = project("web_search", payload, ResponseDetail::Standard);
    assert!(standard.get("conflict_metadata").is_some());
}

#[test]
fn standard_preserves_specialist_fields() {
    let summary = json!({"has_failures": false, "has_absences": true, "has_truncation": false});
    let payload = web_payload(json!([card("a", 1, 0)]), json!([]), summary);
    let standard = project("web_search", payload, ResponseDetail::Standard);
    assert!(standard.get("retrieval_summary").is_some());
    assert!(standard.get("routing_summary").is_some());
    assert!(standard.get("routing_decision").is_none());
    assert_eq!(
        standard.get("response_detail").and_then(|v| v.as_str()),
        Some("standard")
    );
}

#[test]
fn security_applicability_preserved_in_compact() {
    let payload = json!({
        "query": "openssl",
        "mode": "security",
        "resolved_identifiers": {},
        "vulnerabilities": [{"id": "CVE-2024-1"}],
        "groups": [],
        "suggested_fetches": [],
        "providers_queried": ["osv"],
        "providers_failed": [],
        "warnings": [],
        "trust_markers": {},
        "applicability": [
            {"package": "openssl", "status": "affected"},
            {"package": "boringssl", "status": "not_affected"},
            {"package": "libressl", "status": "unknown"}
        ],
        "dependency_findings": [],
        "structured_warnings": [],
        "next_actions": [],
        "retrieval_summary": {"has_failures": false, "has_absences": false, "has_truncation": false},
        "conflict_metadata": []
    });
    let compact = project("security_search", payload, ResponseDetail::Compact);
    let app = compact
        .get("applicability")
        .and_then(|v| v.as_array())
        .unwrap();
    assert_eq!(app.len(), 3);
    let statuses: Vec<&str> = app
        .iter()
        .filter_map(|v| v.get("status").and_then(|s| s.as_str()))
        .collect();
    assert!(statuses.contains(&"affected"));
    assert!(statuses.contains(&"not_affected"));
    assert!(statuses.contains(&"unknown"));
}

#[test]
fn fetch_truncation_and_focus_preserved() {
    let payload = json!({
        "url": "https://example.com/a",
        "final_url": "https://example.com/a",
        "status": 200,
        "fetched": true,
        "truncated": true,
        "trust": "external_untrusted",
        "text": "hello",
        "links": [{"url": "https://example.com/b", "text": "b"}],
        "links_seen": 1,
        "warnings": [],
        "trust_markers": {},
        "document": {"chunks": [{"text": "hello"}]},
        "focus": {"chunks": [{"text": "hello"}], "total_chars": 5},
        "structured_warnings": [],
        "cache_status": "miss"
    });
    let compact = project("web_fetch", payload.clone(), ResponseDetail::Compact);
    assert_eq!(
        compact.get("truncated").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(compact.get("focus").is_some());
    assert!(compact.get("document").is_none());
    assert!(compact.get("links").is_none());
    let standard = project("web_fetch", payload, ResponseDetail::Standard);
    assert!(standard.get("document").is_some());
}

#[test]
fn batch_partial_failure_and_budget_preserved() {
    let payload = json!({
        "fetched": 1,
        "failed": 1,
        "truncated": true,
        "total_chars_returned": 100,
        "results": [
            {"index": 0, "item_type": "web", "label": "a", "stable_id": "s0", "ok": true,
             "response": {"url": "https://example.com/a", "final_url": "https://example.com/a",
                          "status": 200, "fetched": true, "truncated": false,
                          "text": "hi", "document": {"x": 1}}, "error": null,
             "chars_returned": 2, "truncated": false},
            {"index": 1, "item_type": "web", "label": "b", "stable_id": null, "ok": false,
             "response": null, "error": "total character budget exhausted",
             "chars_returned": 0, "truncated": true}
        ],
        "warnings": ["batch_total_budget_exhausted: total character budget of 100 was reached"],
        "structured_warnings": [],
        "telemetry": {"items_requested": 2, "items_completed": 1, "items_failed": 1, "aggregate_budget_exhausted": true}
    });
    let compact = project("batch_fetch", payload, ResponseDetail::Compact);
    assert_eq!(compact.get("fetched").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(compact.get("failed").and_then(|v| v.as_u64()), Some(1));
    let telem = compact.get("telemetry").expect("telemetry");
    assert_eq!(
        telem
            .get("aggregate_budget_exhausted")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    let results = compact.get("results").and_then(|v| v.as_array()).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results[1].get("error").is_some());
}

#[test]
fn repo_trust_distinction_preserved() {
    let local = json!({
        "locator": {"path": "src/main.rs"},
        "stable_id": "fetch_abc",
        "fetched": true,
        "text": "fn main() {}",
        "lines": [{"number": 1, "text": "fn main() {}"}],
        "truncated": false,
        "warnings": [],
        "trust": "local_trusted",
        "trust_markers": {},
        "structured_warnings": []
    });
    let external = json!({
        "locator": {"path": "src/main.rs"},
        "stable_id": "fetch_def",
        "fetched": true,
        "text": "fn main() {}",
        "lines": [{"number": 1, "text": "fn main() {}"}],
        "truncated": false,
        "warnings": [],
        "trust": "external_untrusted",
        "trust_markers": {},
        "structured_warnings": []
    });
    let local_compact = project("repo_fetch", local, ResponseDetail::Compact);
    let ext_compact = project("repo_fetch", external, ResponseDetail::Compact);
    assert_eq!(
        local_compact.get("trust").and_then(|v| v.as_str()),
        Some("local_trusted")
    );
    assert_eq!(
        ext_compact.get("trust").and_then(|v| v.as_str()),
        Some("external_untrusted")
    );
    assert!(local_compact.get("stable_id").is_some());
    assert!(ext_compact.get("stable_id").is_some());
}

#[test]
fn evidence_bundle_identity_unchanged_across_modes() {
    let bundle = json!({
        "sources": [{"id": "src_1", "url": "https://example.com/a"}],
        "fetches": [{"id": "fetch_1", "text": "hello"}],
        "warnings": [],
        "bundle_id": "bundle_abc123"
    });
    let compact = project(
        "build_evidence_bundle",
        bundle.clone(),
        ResponseDetail::Compact,
    );
    let standard = project(
        "build_evidence_bundle",
        bundle.clone(),
        ResponseDetail::Standard,
    );
    let diagnostic = project(
        "build_evidence_bundle",
        bundle.clone(),
        ResponseDetail::Diagnostic,
    );
    assert_eq!(compact, bundle);
    assert_eq!(standard, bundle);
    assert_eq!(diagnostic, bundle);
}

#[test]
fn compact_reduces_bytes_for_representative_payloads() {
    let summary = json!({
        "has_failures": false, "has_absences": false, "has_truncation": false,
        "dimensions": [{"evidence_role": "primary", "message": "ok"}]
    });
    let web = web_payload(
        json!([card("a", 3, 0), card("b", 3, 0)]),
        json!([{"id": "x", "error_class": "timeout", "message": "t"}]),
        summary.clone(),
    );
    let diag_bytes = bytes(&web);
    let compact_bytes = bytes(&project("web_search", web.clone(), ResponseDetail::Compact));
    let standard_bytes = bytes(&project("web_search", web, ResponseDetail::Standard));
    assert!(
        compact_bytes < diag_bytes,
        "compact {compact_bytes} should be < diagnostic {diag_bytes}"
    );
    assert!(
        standard_bytes < diag_bytes,
        "standard {standard_bytes} should be < diagnostic {diag_bytes}"
    );
    assert!(
        compact_bytes <= standard_bytes,
        "compact {compact_bytes} should be <= standard {standard_bytes}"
    );

    let fetch = json!({
        "url": "https://example.com/a",
        "final_url": "https://example.com/a",
        "status": 200, "fetched": true, "truncated": false,
        "text": "x".repeat(2000),
        "links": [{"url": "https://example.com/b", "text": "b"}],
        "document": {"chunks": [{"text": "x".repeat(2000)}]},
        "warnings": [], "trust_markers": {}, "structured_warnings": [],
        "cache_status": "miss"
    });
    let f_diag = bytes(&fetch);
    let f_compact = bytes(&project("web_fetch", fetch, ResponseDetail::Compact));
    assert!(
        f_compact < f_diag,
        "fetch compact {f_compact} < diagnostic {f_diag}"
    );
}

#[test]
fn response_detail_param_defaults_to_diagnostic_and_roundtrips() {
    assert_eq!(ResponseDetail::from_opt(None), ResponseDetail::Diagnostic);
    for (raw, expected) in [
        ("compact", ResponseDetail::Compact),
        ("standard", ResponseDetail::Standard),
        ("diagnostic", ResponseDetail::Diagnostic),
    ] {
        let parsed: ResponseDetail =
            serde_json::from_value(Value::String(raw.to_string())).unwrap();
        assert_eq!(parsed, expected);
    }
}
