use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ResponseDetail {
    Compact,
    Standard,
    #[default]
    Diagnostic,
}

impl ResponseDetail {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Standard => "standard",
            Self::Diagnostic => "diagnostic",
        }
    }

    pub fn from_opt(opt: Option<ResponseDetail>) -> Self {
        opt.unwrap_or(Self::Diagnostic)
    }

    pub fn parse_raw(raw: Option<&str>) -> Result<Option<Self>, String> {
        match raw {
            None => Ok(None),
            Some(s) => match s.to_ascii_lowercase().as_str() {
                "compact" => Ok(Some(Self::Compact)),
                "standard" => Ok(Some(Self::Standard)),
                "diagnostic" => Ok(Some(Self::Diagnostic)),
                _ => Err(format!(
                    "invalid response_detail '{s}'; accepted values: compact, standard, diagnostic"
                )),
            },
        }
    }
}

pub fn project(tool: &str, value: serde_json::Value, detail: ResponseDetail) -> serde_json::Value {
    match detail {
        ResponseDetail::Diagnostic => value,
        ResponseDetail::Compact => match tool {
            "web_search" => project_web_search(value, true),
            "repo_search" => project_repo_search(value, true),
            "research_search" => project_research_search(value, true),
            "security_search" => project_security_search(value, true),
            "web_fetch" => project_web_fetch(value, true),
            "repo_fetch" => project_repo_fetch(value, true),
            "repo_map" => project_repo_map(value, true),
            "batch_fetch" => project_batch_fetch(value, true),
            "build_evidence_bundle" => value,
            _ => value,
        },
        ResponseDetail::Standard => match tool {
            "web_search" => project_web_search(value, false),
            "repo_search" => project_repo_search(value, false),
            "research_search" => project_research_search(value, false),
            "security_search" => project_security_search(value, false),
            "web_fetch" => project_web_fetch(value, false),
            "repo_fetch" => project_repo_fetch(value, false),
            "repo_map" => project_repo_map(value, false),
            "batch_fetch" => project_batch_fetch(value, false),
            "build_evidence_bundle" => value,
            _ => value,
        },
    }
}

fn retrieval_status_minimal(summary: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "has_failures": summary.get("has_failures").and_then(|v| v.as_bool()).unwrap_or(false),
        "has_absences": summary.get("has_absences").and_then(|v| v.as_bool()).unwrap_or(false),
        "has_truncation": summary.get("has_truncation").and_then(|v| v.as_bool()).unwrap_or(false),
        "attempted_job_count": summary.get("attempted_job_count").cloned().unwrap_or(serde_json::Value::Null),
        "completed_job_count": summary.get("completed_job_count").cloned().unwrap_or(serde_json::Value::Null),
        "failed_job_count": summary.get("failed_job_count").cloned().unwrap_or(serde_json::Value::Null),
    })
}

fn conflict_indicator(conflicts: &serde_json::Value) -> serde_json::Value {
    let count = conflicts.as_array().map(|a| a.len()).unwrap_or(0);
    serde_json::json!({
        "has_conflicts": count > 0,
        "conflict_count": count,
    })
}

fn routing_summary(routing: &serde_json::Value) -> serde_json::Value {
    let selected = routing
        .get("selected_providers")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let skipped = routing
        .get("skipped_providers")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    serde_json::json!({
        "selected_count": selected,
        "skipped_count": skipped,
        "degraded": routing.get("degraded").and_then(|v| v.as_bool()).unwrap_or(false),
        "partial": routing.get("partial").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

fn capability_summary(cap: &serde_json::Value) -> serde_json::Value {
    let count = |key: &str| {
        cap.get(key)
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    };
    serde_json::json!({
        "enforced_count": count("enforced"),
        "approximated_count": count("approximated"),
        "not_enforced_count": count("not_enforced"),
    })
}

fn trim_card_excerpts(card: &mut serde_json::Value, keep: usize) {
    if let Some(obj) = card.as_object_mut() {
        if let Some(excerpts) = obj.get_mut("excerpts").and_then(|v| v.as_array_mut()) {
            if excerpts.len() > keep {
                excerpts.truncate(keep);
                obj.insert(
                    "excerpts_truncated".to_string(),
                    serde_json::Value::Bool(true),
                );
            }
        }
    }
}

fn trim_results_excerpts(results: &mut [serde_json::Value], keep: usize) -> bool {
    let mut trimmed = false;
    for card in results.iter_mut() {
        let before = card
            .get("excerpts")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        trim_card_excerpts(card, keep);
        let after = card
            .get("excerpts")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        if after < before {
            trimmed = true;
        }
    }
    trimmed
}

fn trim_groups_excerpts(groups: &mut [serde_json::Value], keep: usize) -> bool {
    let mut trimmed = false;
    for group in groups.iter_mut() {
        if let Some(results) = group.get_mut("results").and_then(|v| v.as_array_mut()) {
            let mut owned: Vec<serde_json::Value> = std::mem::take(results);
            if trim_results_excerpts(&mut owned, keep) {
                trimmed = true;
            }
            *results = owned;
        }
    }
    trimmed
}

fn stamp_detail(
    mut value: serde_json::Value,
    detail: &str,
    extra: serde_json::Value,
) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "response_detail".to_string(),
            serde_json::Value::String(detail.to_string()),
        );
        if let Some(pobj) = extra.as_object() {
            for (k, v) in pobj {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    value
}

fn project_web_search(mut value: serde_json::Value, compact: bool) -> serde_json::Value {
    let mut excerpts_trimmed = false;
    if compact {
        if let Some(results) = value.get_mut("results").and_then(|v| v.as_array_mut()) {
            let mut owned: Vec<serde_json::Value> = std::mem::take(results);
            excerpts_trimmed = trim_results_excerpts(&mut owned, 1);
            *results = owned;
        }
    }
    let retrieval = value.get("retrieval_summary").cloned();
    let conflicts = value
        .get("conflict_metadata")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let routing = value.get("routing_decision").cloned();
    let capability = value.get("capability_enforcement").cloned();
    if let Some(obj) = value.as_object_mut() {
        obj.remove("routing_decision");
        if compact {
            obj.remove("workflow_coverage");
            obj.remove("evidence_role_summary");
            obj.remove("capability_enforcement");
            if let Some(r) = retrieval {
                obj.insert("retrieval_status".to_string(), retrieval_status_minimal(&r));
            }
            obj.remove("retrieval_summary");
            obj.insert(
                "conflict_indicator".to_string(),
                conflict_indicator(&conflicts),
            );
            obj.remove("conflict_metadata");
            if let Some(r) = routing {
                obj.insert("routing_summary".to_string(), routing_summary(&r));
            }
            if let Some(c) = capability {
                if !c.is_null() {
                    obj.insert("capability_summary".to_string(), capability_summary(&c));
                }
            }
        } else {
            if let Some(r) = routing {
                obj.insert("routing_summary".to_string(), routing_summary(&r));
                obj.remove("routing_decision");
            }
        }
    }
    let mode = if compact { "compact" } else { "standard" };
    stamp_detail(
        value,
        mode,
        serde_json::json!({"projection_excerpts_trimmed": excerpts_trimmed}),
    )
}

fn project_repo_search(mut value: serde_json::Value, compact: bool) -> serde_json::Value {
    let mut excerpts_trimmed = false;
    if compact {
        if let Some(groups) = value.get_mut("groups").and_then(|v| v.as_array_mut()) {
            let mut owned: Vec<serde_json::Value> = std::mem::take(groups);
            excerpts_trimmed = trim_groups_excerpts(&mut owned, 1);
            *groups = owned;
        }
    }
    let retrieval = value.get("retrieval_summary").cloned();
    let conflicts = value
        .get("conflict_metadata")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    if let Some(obj) = value.as_object_mut() {
        if compact {
            obj.remove("resolved_hints");
            obj.remove("telemetry");
            obj.remove("package_resolution");
            obj.remove("security_context");
            obj.remove("error_context");
            obj.remove("workflow_coverage");
            obj.remove("evidence_role_summary");
            if let Some(r) = retrieval {
                obj.insert("retrieval_status".to_string(), retrieval_status_minimal(&r));
            }
            obj.remove("retrieval_summary");
            obj.insert(
                "conflict_indicator".to_string(),
                conflict_indicator(&conflicts),
            );
            obj.remove("conflict_metadata");
        } else if let Some(telem) = obj.get_mut("telemetry").and_then(|v| v.as_object_mut()) {
            telem.remove("routing_decision");
        }
    }
    let mode = if compact { "compact" } else { "standard" };
    stamp_detail(
        value,
        mode,
        serde_json::json!({"projection_excerpts_trimmed": excerpts_trimmed}),
    )
}

fn project_research_search(mut value: serde_json::Value, compact: bool) -> serde_json::Value {
    let mut excerpts_trimmed = false;
    if compact {
        if let Some(groups) = value.get_mut("groups").and_then(|v| v.as_array_mut()) {
            let mut owned: Vec<serde_json::Value> = std::mem::take(groups);
            excerpts_trimmed = trim_groups_excerpts(&mut owned, 1);
            *groups = owned;
        }
    }
    let retrieval = value.get("retrieval_summary").cloned();
    let conflicts = value
        .get("conflict_metadata")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let conflicts2_count = value
        .get("conflicts")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if let Some(obj) = value.as_object_mut() {
        if compact {
            obj.remove("subqueries");
            obj.remove("telemetry");
            obj.remove("workflow_context");
            obj.remove("source_quality");
            obj.remove("workflow_coverage");
            obj.remove("evidence_role_summary");
            if let Some(r) = retrieval {
                obj.insert("retrieval_status".to_string(), retrieval_status_minimal(&r));
            }
            obj.remove("retrieval_summary");
            let mut indicator = conflict_indicator(&conflicts);
            if conflicts2_count > 0 {
                indicator["has_conflicts"] = serde_json::Value::Bool(true);
                let prev = indicator
                    .get("conflict_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                indicator["conflict_count"] =
                    serde_json::Value::from(prev + conflicts2_count as u64);
            }
            obj.insert("conflict_indicator".to_string(), indicator);
            obj.remove("conflict_metadata");
        } else if let Some(telem) = obj.get_mut("telemetry").and_then(|v| v.as_object_mut()) {
            telem.remove("routing_decision");
        }
    }
    let mode = if compact { "compact" } else { "standard" };
    stamp_detail(
        value,
        mode,
        serde_json::json!({"projection_excerpts_trimmed": excerpts_trimmed}),
    )
}

fn project_security_search(mut value: serde_json::Value, compact: bool) -> serde_json::Value {
    let mut excerpts_trimmed = false;
    if compact {
        if let Some(groups) = value.get_mut("groups").and_then(|v| v.as_array_mut()) {
            let mut owned: Vec<serde_json::Value> = std::mem::take(groups);
            excerpts_trimmed = trim_groups_excerpts(&mut owned, 1);
            *groups = owned;
        }
    }
    if let Some(vulns) = value
        .get_mut("vulnerabilities")
        .and_then(|v| v.as_array_mut())
    {
        if compact {
            for vuln in vulns.iter_mut() {
                if let Some(obj) = vuln.as_object_mut() {
                    obj.remove("references");
                    obj.remove("details");
                }
            }
        }
    }
    let retrieval = value.get("retrieval_summary").cloned();
    let conflicts = value
        .get("conflict_metadata")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let routing = value.get("routing_decision").cloned();
    let capability = value.get("capability_enforcement").cloned();
    if let Some(obj) = value.as_object_mut() {
        if compact {
            obj.remove("routing_decision");
            obj.remove("capability_enforcement");
            obj.remove("workflow_coverage");
            obj.remove("evidence_role_summary");
            obj.remove("security_evidence_summary");
            if let Some(r) = retrieval {
                obj.insert("retrieval_status".to_string(), retrieval_status_minimal(&r));
            }
            obj.remove("retrieval_summary");
            obj.insert(
                "conflict_indicator".to_string(),
                conflict_indicator(&conflicts),
            );
            obj.remove("conflict_metadata");
            if let Some(r) = routing {
                obj.insert("routing_summary".to_string(), routing_summary(&r));
            }
            if let Some(c) = capability {
                if !c.is_null() {
                    obj.insert("capability_summary".to_string(), capability_summary(&c));
                }
            }
        } else {
            if let Some(r) = routing {
                obj.insert("routing_summary".to_string(), routing_summary(&r));
                obj.remove("routing_decision");
            }
        }
    }
    let mode = if compact { "compact" } else { "standard" };
    stamp_detail(
        value,
        mode,
        serde_json::json!({"projection_excerpts_trimmed": excerpts_trimmed}),
    )
}

fn project_web_fetch(mut value: serde_json::Value, compact: bool) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        obj.remove("raw_text");
        obj.remove("raw_text_chars_returned");
        obj.remove("raw_text_truncated");
        obj.remove("raw_text_cap");
        obj.remove("raw_body");
        obj.remove("response_headers");
        obj.remove("fetch_transform");
        if compact {
            obj.remove("document");
            obj.remove("description");
            if let Some(links) = obj.get("links").and_then(|v| v.as_array()).cloned() {
                let seen = obj
                    .get("links_seen")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(links.len() as u64);
                obj.remove("links");
                obj.insert("links_seen".to_string(), serde_json::Value::from(seen));
                obj.insert(
                    "links_omitted_in_compact".to_string(),
                    serde_json::Value::Bool(true),
                );
            }
            obj.remove("browser_profile");
            obj.remove("origin_backoff_ms");
            obj.remove("retry_after_ms");
        }
    }
    let mode = if compact { "compact" } else { "standard" };
    stamp_detail(value, mode, serde_json::json!({}))
}

fn project_repo_fetch(mut value: serde_json::Value, compact: bool) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        if compact {
            obj.remove("document");
            obj.remove("lines");
            obj.remove("code_context");
            obj.remove("fetched_url");
            obj.remove("raw_url");
        }
    }
    let mode = if compact { "compact" } else { "standard" };
    stamp_detail(value, mode, serde_json::json!({}))
}

fn project_repo_map(mut value: serde_json::Value, compact: bool) -> serde_json::Value {
    let mut entries_truncated = false;
    if compact {
        if let Some(obj) = value.as_object_mut() {
            for key in ["entries", "root_entries"] {
                if let Some(arr) = obj.get_mut(key).and_then(|v| v.as_array_mut()) {
                    if arr.len() > 50 {
                        arr.truncate(50);
                        entries_truncated = true;
                    }
                }
            }
            obj.remove("telemetry");
            for key in [
                "language_distribution",
                "test_relationships",
                "build_configs",
            ] {
                obj.remove(key);
            }
        }
    } else if let Some(obj) = value.as_object_mut() {
        obj.remove("telemetry");
    }
    let mode = if compact { "compact" } else { "standard" };
    stamp_detail(
        value,
        mode,
        serde_json::json!({"projection_entries_truncated": entries_truncated}),
    )
}

fn project_batch_fetch(mut value: serde_json::Value, compact: bool) -> serde_json::Value {
    if let Some(results) = value.get_mut("results").and_then(|v| v.as_array_mut()) {
        for result in results.iter_mut() {
            let item_type = result
                .get("item_type")
                .and_then(|v| v.as_str())
                .unwrap_or("web")
                .to_string();
            if let Some(payload) = result.get_mut("response") {
                if !payload.is_null() {
                    let taken = payload.take();
                    let projected = if item_type == "repo" {
                        project_repo_fetch(taken, compact)
                    } else {
                        project_web_fetch(taken, compact)
                    };
                    *payload = projected;
                }
            }
        }
    }
    let mode = if compact { "compact" } else { "standard" };
    stamp_detail(value, mode, serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_defaults_to_diagnostic() {
        assert_eq!(ResponseDetail::from_opt(None), ResponseDetail::Diagnostic);
    }

    #[test]
    fn parse_accepts_all_modes_case_insensitive() {
        assert_eq!(
            ResponseDetail::parse_raw(Some("compact")).unwrap(),
            Some(ResponseDetail::Compact)
        );
        assert_eq!(
            ResponseDetail::parse_raw(Some("STANDARD")).unwrap(),
            Some(ResponseDetail::Standard)
        );
        assert_eq!(ResponseDetail::parse_raw(None).unwrap(), None);
        assert!(ResponseDetail::parse_raw(Some("verbose")).is_err());
    }

    #[test]
    fn diagnostic_is_passthrough() {
        let v = serde_json::json!({"query": "x", "routing_decision": {"a": 1}});
        let out = project("web_search", v.clone(), ResponseDetail::Diagnostic);
        assert_eq!(out, v);
    }
}
