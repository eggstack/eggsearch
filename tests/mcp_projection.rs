use std::collections::HashSet;

use eggsearch::mcp::projection::{project, ResponseDetail};
use serde_json::{json, Value};

const RETRIEVAL_CASES: &str = include_str!("fixtures/retrieval_quality/cases.json");

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
        "canonical_url": format!("https://example.com/{id}"),
        "providers": ["duckduckgo", "brave_api", "startpage"],
        "provider_ids": ["duckduckgo", "brave_api"],
        "score": 0.8734,
        "source_kind": "documentation",
        "source_role": "official_documentation",
        "quality": {"freshness": "recent", "source_authority": "high", "matched_terms": ["async", "runtime"]},
        "metadata": {"evidence_role": "official_documentation", "debug_trace": "redundant provider normalization details".repeat(4)},
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
fn brief_keeps_identity_trust_failures_and_distinguishes_absence() {
    let failed = web_payload(
        json!([card("src_a", 2, 2)]),
        json!([{"provider_id": "offline", "error_class": "timeout", "message": "provider timeout"}]),
        json!({"has_failures": true, "has_absences": false, "has_truncation": false, "attempted_job_count": 2, "completed_job_count": 1, "failed_job_count": 1}),
    );
    let brief = project("web_search", failed, ResponseDetail::Brief);
    assert_eq!(brief["response_detail"], "brief");
    assert_eq!(brief["retrieval_status"]["has_failures"], true);
    assert_eq!(brief["retrieval_status"]["has_absences"], false);
    assert_eq!(brief["results"][0]["stable_id"], "stable_src_a");
    assert_eq!(brief["results"][0]["trust"], "external_untrusted");
    assert_eq!(brief["results"][0]["trust_markers"]["injection_hits"], 2);
    assert_eq!(brief["providers_failed"][0]["error_class"], "timeout");

    let absent = web_payload(
        json!([]),
        json!([]),
        json!({"has_failures": false, "has_absences": true, "has_truncation": false}),
    );
    let brief_absent = project("web_search", absent, ResponseDetail::Brief);
    assert_eq!(brief_absent["retrieval_status"]["has_failures"], false);
    assert_eq!(brief_absent["retrieval_status"]["has_absences"], true);
}

#[test]
fn brief_preserves_security_applicability_states_and_fetch_truncation() {
    let security = json!({
        "query": "openssl",
        "vulnerabilities": [{"id":"CVE-2025-1","title":"Issue","severity":"high"}],
        "applicability": [
            {"package":"openssl","status":"affected","version":"1.0"},
            {"package":"boringssl","status":"not_affected"},
            {"package":"libressl","status":"unknown"}
        ],
        "providers_failed": [{"provider_id":"osv","error_class":"timeout"}],
        "warnings": ["applicability is incomplete"],
        "structured_warnings": [{"code":"applicability_unknown","severity":"high","message":"check package metadata"}],
        "retrieval_summary":{"has_failures":true,"has_absences":false,"has_truncation":false}
    });
    let brief = project("security_search", security, ResponseDetail::Brief);
    let states = brief["applicability"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["status"].as_str())
        .collect::<Vec<_>>();
    assert!(states.contains(&"affected"));
    assert!(states.contains(&"not_affected"));
    assert!(states.contains(&"unknown"));
    assert_eq!(brief["vulnerabilities"][0]["id"], "CVE-2025-1");

    let fetch = json!({
        "url":"https://example.com/file.pdf","final_url":"https://example.com/file.pdf",
        "content_type":"application/pdf","fetched":true,"truncated":true,
        "trust":"external_untrusted","trust_markers":{"injection_hits":1},
        "text":"<<<EXTERNAL_UNTRUSTED field=text id=fetch_1>>>\n".to_owned() + &"quoted text ".repeat(200) + "\n<<<END>>>",
        "focus":{"chunks":[{"text":"focused passage","page":4,"line_start":8,"line_end":9}]},
        "warnings":["content_truncated"],"structured_warnings":[]
    });
    let brief_fetch = project("web_fetch", fetch, ResponseDetail::Brief);
    assert_eq!(brief_fetch["truncated"], true);
    assert_eq!(brief_fetch["trust"], "external_untrusted");
    assert!(brief_fetch["text"].as_str().unwrap().contains("<<<END>>>"));
    assert_eq!(brief_fetch["focus"]["chunks"][0]["page"], 4);
}

#[test]
fn brief_keeps_extracted_document_text_when_it_fits_the_budget() {
    let query = "What are two changes in the Rust 1.90.0 announcement?";
    let document_text = format!(
        "<<<EXTERNAL_UNTRUSTED field=text id=fetch_rust_1_90>>>\n{}\n<<<END>>>",
        (0..40)
            .map(|index| format!(
                "Section {index}: Rust 1.90.0 includes a documented language and tooling change."
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let document = json!({
        "query": query,
        "url": "https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/",
        "final_url": "https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/",
        "content_type": "text/html",
        "fetched": true,
        "truncated": false,
        "trust": "external_untrusted",
        "text": document_text,
        "warnings": [],
        "structured_warnings": []
    });

    let diagnostic = project("web_fetch", document.clone(), ResponseDetail::Diagnostic);
    let brief = eggsearch::mcp::projection::project_with_budget(
        "web_fetch",
        document,
        ResponseDetail::Brief,
        8_000,
    );

    assert_eq!(diagnostic["query"], query);
    assert_eq!(brief["query"], query);
    assert_eq!(diagnostic["text"], brief["text"]);
    assert_eq!(brief["truncated"], false);
    assert_eq!(brief["output_truncated"], false);
    assert!(brief["text"].as_str().unwrap().chars().count() > 800);
}

#[test]
fn brief_enforces_character_budget_and_retains_failed_batch_items() {
    let mut payload = web_payload(
        json!([card("a", 1, 0), card("b", 1, 0), card("c", 1, 0)]),
        json!([]),
        json!({"has_failures": false, "has_absences": false, "has_truncation": false}),
    );
    payload["results"][0]["excerpts"][0]["text"] = json!("w".repeat(1200));
    payload["results"][1]["excerpts"][0]["text"] = json!("x".repeat(1200));
    payload["results"][2]["excerpts"][0]["text"] = json!("y".repeat(1200));
    let brief = eggsearch::mcp::projection::project_with_budget(
        "web_search",
        payload,
        ResponseDetail::Brief,
        512,
    );
    assert!(bytes(&brief) <= 512, "{} chars", bytes(&brief));
    assert_eq!(brief["output_truncated"], true);
    assert!(brief["omitted_count"].as_u64().unwrap() > 0);
    assert!(brief["returned_count"].as_u64().unwrap() < 3);

    let batch = json!({
        "fetched":1,"failed":1,"truncated":true,"total_chars_returned":100,
        "results":[
            {"index":0,"item_type":"web","stable_id":"ok","ok":true,"error":null,"response":{"url":"https://example.com/a","fetched":true,"truncated":false,"text":"success"}},
            {"index":1,"item_type":"web","stable_id":null,"ok":false,"error":"timeout","response":null,"truncated":true}
        ],
        "warnings":["batch item failed"],"structured_warnings":[]
    });
    let brief_batch = eggsearch::mcp::projection::project_with_budget(
        "batch_fetch",
        batch,
        ResponseDetail::Brief,
        512,
    );
    assert!(brief_batch["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["error"] == "timeout"));
    assert_eq!(brief_batch["failed"], 1);
}

#[test]
fn brief_budget_and_reduction_cover_repository_and_research_groups() {
    let oversized_failure = json!({
        "results": [],
        "providers_failed": [{
            "provider_id": "fixture",
            "error_class": "timeout",
            "error": "timeout: ".to_owned() + &"failure detail ".repeat(300)
        }],
        "retrieval_summary": {"has_failures": true, "has_absences": false, "has_truncation": false}
    });
    let bounded = eggsearch::mcp::projection::project_with_budget(
        "web_search",
        oversized_failure,
        ResponseDetail::Brief,
        512,
    );
    assert!(bytes(&bounded) <= 512, "{} chars", bytes(&bounded));
    assert_eq!(bounded["retrieval_status"]["has_failures"], true);
    assert_eq!(bounded["providers_failed_summary"]["total"], 1);
    assert_eq!(
        bounded["providers_failed_summary"]["error_classes"]["timeout"],
        1
    );

    for tool in ["repo_search", "research_search"] {
        let item = json!({
            "id": "source-1",
            "stable_id": "stable-source-1",
            "title": "Primary source",
            "url": "https://example.com/source",
            "locator": {"path": "src/lib.rs", "line_start": 12, "line_end": 24},
            "trust": "external_untrusted",
            "source_role": "primary",
            "excerpts": [{"text": "relevant evidence", "provenance": "content"}],
            "metadata": {"discardable": "debug details ".repeat(80)},
            "quality": {"discardable": "quality details ".repeat(80)},
            "providers": ["provider-a", "provider-b"],
            "score": 0.82
        });
        let payload = json!({
            "query": "locate the primary source",
            "groups": [{"group": "primary_sources", "results": [item.clone()]}],
            "retrieval_summary": {"has_failures": false, "has_absences": false, "has_truncation": false},
            "conflict_metadata": [],
            "telemetry": {"discardable": "routing diagnostics ".repeat(100)},
            "workflow_context": {"discardable": "workflow diagnostics ".repeat(100)},
            "source_quality": {"discardable": "source diagnostics ".repeat(100)},
            "evidence_role_summary": {"discardable": "role diagnostics ".repeat(100)}
        });
        let diagnostic_bytes = bytes(&payload);
        let brief = project(tool, payload, ResponseDetail::Brief);
        let brief_bytes = bytes(&brief);
        println!(
            "brief-fixture tool={tool} diagnostic_bytes={diagnostic_bytes} brief_bytes={brief_bytes} reduction_percent={:.1}",
            100.0 * (diagnostic_bytes - brief_bytes) as f64 / diagnostic_bytes as f64
        );
        assert!(
            brief_bytes * 10 <= diagnostic_bytes * 7,
            "{tool} brief={brief_bytes}, diagnostic={diagnostic_bytes}"
        );
        assert_eq!(
            brief["groups"][0]["results"][0]["stable_id"],
            "stable-source-1"
        );
        assert_eq!(
            brief["groups"][0]["results"][0]["locator"]["line_start"],
            12
        );
        assert_eq!(
            brief["groups"][0]["results"][0]["trust"],
            "external_untrusted"
        );
    }
}

#[test]
fn brief_budget_bounds_large_security_applicability_lists() {
    let payload = json!({
        "query": "package ecosystem",
        "vulnerabilities": [],
        "applicability": (0..40).map(|index| json!({
            "package": format!("package-{index}"),
            "version": "1.2.3",
            "status": match index % 3 {
                0 => "affected",
                1 => "not_affected",
                _ => "unknown"
            }
        })).collect::<Vec<_>>(),
        "retrieval_summary": {"has_failures": false, "has_absences": false, "has_truncation": false},
        "warnings": [],
        "structured_warnings": []
    });
    let brief = eggsearch::mcp::projection::project_with_budget(
        "security_search",
        payload,
        ResponseDetail::Brief,
        512,
    );
    assert!(bytes(&brief) <= 512, "{} chars", bytes(&brief));
    assert_eq!(brief["applicability_summary"]["affected"], 14);
    assert_eq!(brief["applicability_summary"]["not_affected"], 13);
    assert_eq!(brief["applicability_summary"]["unknown"], 13);
    assert_eq!(brief["output_truncated"], true);
}

#[test]
fn brief_budget_summarizes_many_provider_and_batch_failures() {
    let providers_failed = (0..40)
        .map(|index| {
            json!({
                "provider_id": format!("provider-{index}"),
                "error_class": "timeout",
                "message": "provider timed out"
            })
        })
        .collect::<Vec<_>>();
    let provider_payload = json!({
        "results": [],
        "providers_failed": providers_failed,
        "retrieval_summary": {"has_failures": true, "has_absences": false, "has_truncation": false},
        "warnings": [],
        "structured_warnings": []
    });
    let provider_brief = eggsearch::mcp::projection::project_with_budget(
        "web_search",
        provider_payload,
        ResponseDetail::Brief,
        512,
    );
    assert!(
        bytes(&provider_brief) <= 512,
        "{} chars",
        bytes(&provider_brief)
    );
    assert_eq!(provider_brief["retrieval_status"]["has_failures"], true);
    assert_eq!(provider_brief["providers_failed_summary"]["total"], 40);
    assert!(
        provider_brief["providers_failed_summary"]["omitted"]
            .as_u64()
            .unwrap()
            > 0
    );

    let results = (0..20)
        .map(|index| {
            json!({
                "index": index,
                "item_type": "web",
                "ok": false,
                "error": "timeout",
                "response": null
            })
        })
        .collect::<Vec<_>>();
    let batch_payload =
        json!({"failed":20,"fetched":0,"results":results,"warnings":[],"structured_warnings":[]});
    let batch_brief = eggsearch::mcp::projection::project_with_budget(
        "batch_fetch",
        batch_payload,
        ResponseDetail::Brief,
        512,
    );
    assert!(bytes(&batch_brief) <= 512, "{} chars", bytes(&batch_brief));
    assert_eq!(batch_brief["failed"], 20);
    let retained_failure_count = batch_brief["results"].as_array().unwrap().len();
    let omitted_failure_indices = batch_brief["failed_member_indices"].as_array().unwrap();
    assert_eq!(retained_failure_count + omitted_failure_indices.len(), 20);
    assert!(batch_brief["results"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["ok"] == false));
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
    let standard_bytes = bytes(&project(
        "web_search",
        web.clone(),
        ResponseDetail::Standard,
    ));
    let brief_bytes = bytes(&project("web_search", web, ResponseDetail::Brief));
    println!(
        "projection-bytes tool=web_search diagnostic={diag_bytes} standard={standard_bytes} compact={compact_bytes} brief={brief_bytes}"
    );
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
    assert!(
        brief_bytes * 10 <= diag_bytes * 7,
        "web brief={brief_bytes}, diagnostic={diag_bytes}"
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
    let f_compact = bytes(&project(
        "web_fetch",
        fetch.clone(),
        ResponseDetail::Compact,
    ));
    let f_brief = bytes(&project("web_fetch", fetch, ResponseDetail::Brief));
    println!(
        "projection-bytes tool=web_fetch diagnostic={f_diag} compact={f_compact} brief={f_brief}"
    );
    assert!(
        f_compact < f_diag,
        "fetch compact {f_compact} < diagnostic {f_diag}"
    );
    assert!(
        f_brief * 10 <= f_diag * 7,
        "fetch brief={f_brief}, diagnostic={f_diag}"
    );
}

#[test]
fn retrieval_quality_host_trace_records_per_case_projection_cost() {
    let corpus: Value = serde_json::from_str(RETRIEVAL_CASES).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    let mut byte_totals = [0usize; 4];
    let mut retained_totals = [0usize; 4];
    let mut simulated_call_total = 0usize;
    for case in cases {
        let results = case["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|result| {
                let passages = result["passages"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                json!({
                    "id": result["id"],
                    "stable_id": result["id"],
                    "url": result["url"],
                    "title": result["title"],
                    "snippet": result["snippet"],
                    "trust": "external_untrusted",
                    "excerpts": passages.into_iter().map(|text| json!({"text": text, "provenance": "fixture"})).collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        let failed = case["expected_state"] == "failure";
        let absent = case["expected_state"] == "absence";
        let mut response = web_payload(
            json!(results),
            if failed {
                json!([{"provider_id": "fixture-provider", "error_class": "unavailable"}])
            } else {
                json!([])
            },
            json!({
                "has_failures": failed,
                "has_absences": absent,
                "has_truncation": false,
                "attempted_job_count": 1,
                "completed_job_count": if failed {0} else {1},
                "failed_job_count": if failed {1} else {0}
            }),
        );
        response["query"] = case["query"].clone();
        let tool = match case["retrieval_scope"].as_str().unwrap() {
            "repository" => "repo_search",
            "advisory" | "advisory_and_package_metadata" => "security_search",
            "multi_source" | "package_registry" => "research_search",
            _ => "web_search",
        };
        let modes = [
            ResponseDetail::Diagnostic,
            ResponseDetail::Standard,
            ResponseDetail::Compact,
            ResponseDetail::Brief,
        ];
        let mut mode_bytes = [0usize; 4];
        let mut retained = [0usize; 4];
        let relevant_ids: HashSet<&str> = case["relevant_documents"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["id"].as_str())
            .collect();
        for (index, mode) in modes.into_iter().enumerate() {
            let projected = project(tool, response.clone(), mode);
            mode_bytes[index] = bytes(&projected);
            byte_totals[index] += mode_bytes[index];
            retained[index] = projected["results"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|result| {
                    result["id"]
                        .as_str()
                        .is_some_and(|id| relevant_ids.contains(id))
                })
                .count();
            retained_totals[index] += retained[index];
            if matches!(mode, ResponseDetail::Compact | ResponseDetail::Brief) && (failed || absent)
            {
                assert_eq!(projected["retrieval_status"]["has_failures"], json!(failed));
                assert_eq!(projected["retrieval_status"]["has_absences"], json!(absent));
            }
        }
        let source_fetches = retained[0];
        let simulated_calls = 1 + source_fetches + usize::from(source_fetches > 0);
        simulated_call_total += simulated_calls;
        println!(
            "retrieval-context-trace case={} tool={} partition={} state={} bytes_diagnostic={} bytes_standard={} bytes_compact={} bytes_brief={} relevant_retained_diagnostic={} relevant_retained_standard={} relevant_retained_compact={} relevant_retained_brief={} simulated_tool_calls={} token_estimates={}/{}/{}/{}",
            case["id"].as_str().unwrap(),
            tool,
            case["partition"].as_str().unwrap(),
            case["expected_state"].as_str().unwrap(),
            mode_bytes[0],
            mode_bytes[1],
            mode_bytes[2],
            mode_bytes[3],
            retained[0],
            retained[1],
            retained[2],
            retained[3],
            simulated_calls,
            mode_bytes[0].div_ceil(4),
            mode_bytes[1].div_ceil(4),
            mode_bytes[2].div_ceil(4)
            ,mode_bytes[3].div_ceil(4)
        );
    }
    assert_eq!(retained_totals, [14, 14, 14, 14]);
    assert_eq!(simulated_call_total, 42);
    println!(
        "retrieval-context-trace-total cases={} bytes_diagnostic={} bytes_standard={} bytes_compact={} bytes_brief={} token_estimates={}/{}/{}/{} retained={:?} simulated_tool_calls={}",
        cases.len(),
        byte_totals[0],
        byte_totals[1],
        byte_totals[2],
        byte_totals[3],
        byte_totals[0].div_ceil(4),
        byte_totals[1].div_ceil(4),
        byte_totals[2].div_ceil(4),
        byte_totals[3].div_ceil(4),
        retained_totals,
        simulated_call_total
    );
}

#[test]
fn response_detail_param_defaults_to_diagnostic_and_roundtrips() {
    assert_eq!(ResponseDetail::from_opt(None), ResponseDetail::Diagnostic);
    assert_eq!(
        ResponseDetail::from_opt_for_profile(
            None,
            eggsearch::core::config::McpSchemaProfile::Compact
        ),
        ResponseDetail::Brief
    );
    assert_eq!(
        ResponseDetail::from_opt_for_profile(
            Some(ResponseDetail::Diagnostic),
            eggsearch::core::config::McpSchemaProfile::Compact
        ),
        ResponseDetail::Diagnostic
    );
    for (raw, expected) in [
        ("brief", ResponseDetail::Brief),
        ("compact", ResponseDetail::Compact),
        ("standard", ResponseDetail::Standard),
        ("diagnostic", ResponseDetail::Diagnostic),
    ] {
        let parsed: ResponseDetail =
            serde_json::from_value(Value::String(raw.to_string())).unwrap();
        assert_eq!(parsed, expected);
    }
}
