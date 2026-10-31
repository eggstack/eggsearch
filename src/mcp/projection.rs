use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ResponseDetail {
    Compact,
    Standard,
    Brief,
    #[default]
    Diagnostic,
}

pub const DEFAULT_BRIEF_MAX_CHARS: usize = 8_000;

impl ResponseDetail {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Standard => "standard",
            Self::Brief => "brief",
            Self::Diagnostic => "diagnostic",
        }
    }

    pub fn from_opt(opt: Option<ResponseDetail>) -> Self {
        opt.unwrap_or(Self::Diagnostic)
    }

    pub fn from_opt_for_profile(
        opt: Option<ResponseDetail>,
        profile: crate::core::config::McpSchemaProfile,
    ) -> Self {
        opt.unwrap_or(match profile {
            crate::core::config::McpSchemaProfile::Full => Self::Diagnostic,
            crate::core::config::McpSchemaProfile::Compact => Self::Brief,
        })
    }

    pub fn parse_raw(raw: Option<&str>) -> Result<Option<Self>, String> {
        match raw {
            None => Ok(None),
            Some(s) => match s.to_ascii_lowercase().as_str() {
                "compact" => Ok(Some(Self::Compact)),
                "standard" => Ok(Some(Self::Standard)),
                "brief" => Ok(Some(Self::Brief)),
                "diagnostic" => Ok(Some(Self::Diagnostic)),
                _ => Err(format!(
                    "invalid response_detail '{s}'; accepted values: brief, compact, standard, diagnostic"
                )),
            },
        }
    }
}

pub fn project(tool: &str, value: serde_json::Value, detail: ResponseDetail) -> serde_json::Value {
    project_with_budget(tool, value, detail, DEFAULT_BRIEF_MAX_CHARS)
}

pub fn project_with_budget(
    tool: &str,
    value: serde_json::Value,
    detail: ResponseDetail,
    max_chars: usize,
) -> serde_json::Value {
    match detail {
        ResponseDetail::Brief => project_brief(tool, value, max_chars),
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

fn project_brief(tool: &str, value: serde_json::Value, max_chars: usize) -> serde_json::Value {
    if tool == "build_evidence_bundle" {
        return value;
    }
    let mut projected = project(tool, value, ResponseDetail::Compact);
    reduce_brief_fields(&mut projected);
    let applicability_total = add_applicability_summary(&mut projected);
    let provider_failure_total = add_provider_failure_summary(&mut projected);
    if let Some(object) = projected.as_object_mut() {
        object.insert(
            "response_detail".into(),
            serde_json::Value::String("brief".into()),
        );
    }
    let primary_key = ["results", "groups", "vulnerabilities", "items"]
        .into_iter()
        .find(|key| {
            projected
                .get(*key)
                .and_then(|value| value.as_array())
                .is_some()
        });
    let original_count = primary_key
        .and_then(|key| projected.get(key))
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let mut clipped = false;
    let mut omitted = 0usize;
    let mut per_text_limit = max_chars;
    loop {
        clipped |= clip_brief_strings(&mut projected, per_text_limit);
        loop {
            let size = brief_serialized_chars(&projected);
            if size <= max_chars {
                break;
            }
            let Some(key) = primary_key else {
                break;
            };
            let Some(values) = projected
                .get_mut(key)
                .and_then(serde_json::Value::as_array_mut)
            else {
                break;
            };
            let removable = values
                .iter()
                .rposition(|item| !brief_record_is_failure(item));
            let Some(index) = removable else {
                if tool == "batch_fetch" {
                    let removed_failure = projected
                        .get_mut(key)
                        .and_then(serde_json::Value::as_array_mut)
                        .and_then(|values| {
                            values
                                .iter()
                                .rposition(brief_record_is_failure)
                                .map(|index| (index, values.remove(index)))
                        });
                    if let Some((position, member)) = removed_failure {
                        let member_index = member
                            .get("index")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(position as u64);
                        record_failed_batch_member(&mut projected, member_index);
                        omitted += 1;
                        clipped = true;
                        continue;
                    }
                }
                let removed_applicability = projected
                    .get_mut("applicability")
                    .and_then(serde_json::Value::as_array_mut)
                    .and_then(|items| (!items.is_empty()).then(|| items.pop()).flatten())
                    .is_some();
                if removed_applicability {
                    clipped = true;
                    let retained = projected
                        .get("applicability")
                        .and_then(serde_json::Value::as_array)
                        .map_or(0, Vec::len);
                    if let Some(summary) = projected
                        .get_mut("applicability_summary")
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        summary.insert(
                            "omitted".into(),
                            applicability_total.saturating_sub(retained).into(),
                        );
                    }
                    continue;
                }
                let removed_provider_failure = projected
                    .get_mut("providers_failed")
                    .and_then(serde_json::Value::as_array_mut)
                    .and_then(Vec::pop)
                    .is_some();
                if removed_provider_failure {
                    clipped = true;
                    let retained = projected
                        .get("providers_failed")
                        .and_then(serde_json::Value::as_array)
                        .map_or(0, Vec::len);
                    if let Some(summary) = projected
                        .get_mut("providers_failed_summary")
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        summary.insert(
                            "omitted".into(),
                            provider_failure_total.saturating_sub(retained).into(),
                        );
                    }
                    continue;
                }
                break;
            };
            values.remove(index);
            omitted += 1;
        }
        let retained_count = primary_key
            .and_then(|key| projected.get(key))
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        if let Some(object) = projected.as_object_mut() {
            object.insert("returned_count".into(), retained_count.into());
            object.insert("omitted_count".into(), omitted.into());
            object.insert(
                "output_truncated".into(),
                serde_json::Value::Bool(clipped || omitted > 0),
            );
        }
        if brief_serialized_chars(&projected) <= max_chars || per_text_limit == 0 {
            break;
        }
        per_text_limit = (per_text_limit / 2).min(per_text_limit.saturating_sub(1));
    }
    for optional in [
        "next_actions",
        "suggested_fetches",
        "providers_queried",
        "routing_summary",
        "workflow_coverage",
        "capability_enforcement",
        "response_detail",
        "query",
    ] {
        if brief_serialized_chars(&projected) > max_chars {
            if let Some(object) = projected.as_object_mut() {
                clipped |= object.remove(optional).is_some();
                object.insert(
                    "output_truncated".into(),
                    serde_json::Value::Bool(clipped || omitted > 0),
                );
            }
        }
    }
    let retained_count = primary_key
        .and_then(|key| projected.get(key))
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    if let Some(object) = projected.as_object_mut() {
        object.insert("returned_count".into(), retained_count.into());
        object.insert(
            "omitted_count".into(),
            (omitted.max(original_count.saturating_sub(retained_count))).into(),
        );
        object.insert(
            "output_truncated".into(),
            serde_json::Value::Bool(clipped || omitted > 0),
        );
        let retained = object
            .get("applicability")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        if let Some(summary) = object
            .get_mut("applicability_summary")
            .and_then(serde_json::Value::as_object_mut)
        {
            summary.insert(
                "omitted".into(),
                applicability_total.saturating_sub(retained).into(),
            );
        }
        let retained_provider_failures = object
            .get("providers_failed")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        if let Some(summary) = object
            .get_mut("providers_failed_summary")
            .and_then(serde_json::Value::as_object_mut)
        {
            summary.insert(
                "omitted".into(),
                provider_failure_total
                    .saturating_sub(retained_provider_failures)
                    .into(),
            );
        }
    }
    projected
}

fn add_applicability_summary(value: &mut serde_json::Value) -> usize {
    let Some(items) = value
        .get("applicability")
        .and_then(serde_json::Value::as_array)
    else {
        return 0;
    };
    let total = items.len();
    let mut summary = serde_json::Map::new();
    for (key, aliases) in [
        ("affected", ["affected", "applicable"]),
        ("not_affected", ["not_affected", "unaffected"]),
        ("unknown", ["unknown", "unknown"]),
    ] {
        let count = items
            .iter()
            .filter(|item| {
                item.get("status")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|status| aliases.contains(&status))
            })
            .count();
        summary.insert(key.into(), count.into());
    }
    summary.insert("total".into(), total.into());
    summary.insert("omitted".into(), 0usize.into());
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "applicability_summary".into(),
            serde_json::Value::Object(summary),
        );
    }
    total
}

fn add_provider_failure_summary(value: &mut serde_json::Value) -> usize {
    let Some(failures) = value
        .get("providers_failed")
        .and_then(serde_json::Value::as_array)
    else {
        return 0;
    };
    let total = failures.len();
    if total > 0 {
        let mut error_classes = serde_json::Map::new();
        for failure in failures {
            let class = failure
                .get("error_class")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let count = error_classes
                .get(class)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            error_classes.insert(class.into(), (count + 1).into());
        }
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "providers_failed_summary".into(),
                serde_json::json!({
                    "total": total,
                    "omitted": 0,
                    "error_classes": error_classes
                }),
            );
        }
    }
    total
}

fn record_failed_batch_member(value: &mut serde_json::Value, index: u64) {
    if let Some(object) = value.as_object_mut() {
        let indices = object
            .entry("failed_member_indices")
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let Some(indices) = indices.as_array_mut() {
            indices.push(index.into());
            indices.sort_by_key(|value| value.as_u64().unwrap_or(u64::MAX));
        }
    }
}

fn brief_serialized_chars(value: &serde_json::Value) -> usize {
    serde_json::to_string(value).map_or(usize::MAX, |encoded| encoded.chars().count())
}

fn brief_record_is_failure(value: &serde_json::Value) -> bool {
    value.get("ok").and_then(serde_json::Value::as_bool) == Some(false)
        || value.get("fetched").and_then(serde_json::Value::as_bool) == Some(false)
        || value.get("error").is_some_and(|error| !error.is_null())
        || value.get("error_class").is_some()
        || value.get("failed").and_then(serde_json::Value::as_bool) == Some(true)
}

fn reduce_brief_fields(value: &mut serde_json::Value) {
    const FIELDS: &[&str] = &[
        "affected_versions",
        "aggregate_budget_exhausted",
        "applicability",
        "attempted_job_count",
        "batch_status",
        "build_configs",
        "cache_status",
        "capability_enforcement",
        "code",
        "code_span_id",
        "commit_sha",
        "conflict_count",
        "conflict_indicator",
        "conflicts",
        "content_type",
        "counterpoints",
        "coverage",
        "coverage_status",
        "checkout_state",
        "dirty",
        "degraded",
        "dependency_findings",
        "document_id",
        "ecosystem",
        "error",
        "error_class",
        "evidence_gaps",
        "evidence_role",
        "excerpts",
        "failed",
        "failed_job_count",
        "fetches",
        "fetched",
        "final_url",
        "fixed_versions",
        "focus",
        "gaps",
        "group",
        "groups",
        "has_absences",
        "has_conflicts",
        "has_failures",
        "has_truncation",
        "id",
        "injection_hits",
        "item_type",
        "items",
        "items_completed",
        "items_failed",
        "items_requested",
        "chars_returned",
        "total_chars_returned",
        "chunks",
        "page",
        "page_number",
        "label",
        "language",
        "line_end",
        "line_start",
        "locator",
        "message",
        "mode",
        "next_actions",
        "omitted_count",
        "output_truncated",
        "page_end",
        "page_start",
        "package",
        "partial",
        "path",
        "provider_id",
        "providers_failed",
        "providers_queried",
        "query",
        "rank",
        "ref",
        "redirected",
        "response",
        "response_detail",
        "ok",
        "results",
        "retrieval_status",
        "retrieval_summary",
        "returned_count",
        "routing_summary",
        "telemetry",
        "severity",
        "snippet",
        "source_id",
        "source_kind",
        "source_role",
        "sources",
        "stable_id",
        "status",
        "structured_warnings",
        "suggested_fetches",
        "text",
        "title",
        "total_chars",
        "truncated",
        "untracked",
        "trust",
        "trust_level",
        "trust_markers",
        "type",
        "unknown",
        "url",
        "version",
        "workspace_id",
        "workspace_root",
        "remote_mismatch",
        "vulnerabilities",
        "warnings",
        "workflow_coverage",
        "affected",
        "unaffected",
        "applicable",
        "not_applicable",
        "confidence",
        "advisory_id",
        "vulnerability_id",
        "source",
        "reason",
        "tool",
        "items_truncated",
        "truncation_evidence",
        "completed_job_count",
        "returned_sources",
        "omitted_sources",
        "conflict_metadata",
    ];
    match value {
        serde_json::Value::Object(object) => {
            object.retain(|key, _| FIELDS.contains(&key.as_str()));
            for child in object.values_mut() {
                reduce_brief_fields(child);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                reduce_brief_fields(child);
            }
        }
        _ => {}
    }
}

fn clip_brief_strings(value: &mut serde_json::Value, max_chars: usize) -> bool {
    let mut changed = false;
    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if key == "warnings" {
                    if let Some(warnings) = child.as_array_mut() {
                        for warning in warnings {
                            if let Some(text) = warning.as_str() {
                                let warning_limit = max_chars.min(96);
                                if text.chars().count() > warning_limit {
                                    *warning = serde_json::Value::String(clip_warning_text(
                                        text,
                                        warning_limit,
                                    ));
                                    changed = true;
                                }
                            }
                        }
                    }
                }
                if matches!(key.as_str(), "snippet" | "text" | "message" | "error") {
                    if let Some(text) = child.as_str() {
                        if text.chars().count() > max_chars {
                            *child = serde_json::Value::String(clip_framed_text(text, max_chars));
                            changed = true;
                        }
                    }
                }
                changed |= clip_brief_strings(child, max_chars);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                changed |= clip_brief_strings(child, max_chars);
            }
        }
        _ => {}
    }
    changed
}

fn clip_warning_text(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return text
            .split_once(':')
            .map_or_else(String::new, |(code, _)| code.to_string());
    }
    let prefix = text.split_once(':').map_or(text, |(code, _)| code);
    if prefix.chars().count() <= max_chars {
        prefix.to_string()
    } else {
        prefix.chars().take(max_chars).collect()
    }
}

fn clip_framed_text(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let start = "<<<EXTERNAL_UNTRUSTED";
    let end = "<<<END>>>";
    if let Some(header_end) = text.find(">>>") {
        if text.starts_with(start) && text.rfind(end).is_some() {
            let header_end = header_end + 3;
            let header = &text[..header_end];
            let body_end = text.rfind(end).unwrap_or(text.len());
            let body = text[header_end..body_end].trim_matches('\n');
            let body_chars = max_chars.saturating_sub(header.chars().count() + end.len() + 2);
            let clipped: String = body.chars().take(body_chars).collect();
            return format!("{header}\n{clipped}\n{end}");
        }
    }
    let clipped: String = text.chars().take(max_chars).collect();
    clipped
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
            if trim_results_excerpts(results, keep) {
                trimmed = true;
            }
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
            excerpts_trimmed = trim_results_excerpts(results, 1);
        }
    }
    if let Some(obj) = value.as_object_mut() {
        let routing = obj.remove("routing_decision");
        if compact {
            let retrieval = obj.remove("retrieval_summary");
            let conflicts = obj
                .remove("conflict_metadata")
                .unwrap_or(serde_json::Value::Null);
            let capability = obj.remove("capability_enforcement");
            obj.remove("workflow_coverage");
            obj.remove("evidence_role_summary");
            if let Some(r) = retrieval {
                obj.insert("retrieval_status".to_string(), retrieval_status_minimal(&r));
            }
            obj.insert(
                "conflict_indicator".to_string(),
                conflict_indicator(&conflicts),
            );
            if let Some(r) = routing.as_ref() {
                obj.insert("routing_summary".to_string(), routing_summary(r));
            }
            if let Some(c) = capability {
                if !c.is_null() {
                    obj.insert("capability_summary".to_string(), capability_summary(&c));
                }
            }
        } else if let Some(r) = routing {
            obj.insert("routing_summary".to_string(), routing_summary(&r));
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
            excerpts_trimmed = trim_groups_excerpts(groups, 1);
        }
    }
    if let Some(obj) = value.as_object_mut() {
        if compact {
            let retrieval = obj.remove("retrieval_summary");
            let conflicts = obj
                .remove("conflict_metadata")
                .unwrap_or(serde_json::Value::Null);
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
            obj.insert(
                "conflict_indicator".to_string(),
                conflict_indicator(&conflicts),
            );
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
            excerpts_trimmed = trim_groups_excerpts(groups, 1);
        }
    }
    let conflicts2_count = value
        .get("conflicts")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if let Some(obj) = value.as_object_mut() {
        if compact {
            let retrieval = obj.remove("retrieval_summary");
            let conflicts = obj
                .remove("conflict_metadata")
                .unwrap_or(serde_json::Value::Null);
            obj.remove("subqueries");
            obj.remove("telemetry");
            obj.remove("workflow_context");
            obj.remove("source_quality");
            obj.remove("workflow_coverage");
            obj.remove("evidence_role_summary");
            if let Some(r) = retrieval {
                obj.insert("retrieval_status".to_string(), retrieval_status_minimal(&r));
            }
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
            excerpts_trimmed = trim_groups_excerpts(groups, 1);
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
    if let Some(obj) = value.as_object_mut() {
        if compact {
            let routing = obj.remove("routing_decision");
            let capability = obj.remove("capability_enforcement");
            let retrieval = obj.remove("retrieval_summary");
            let conflicts = obj
                .remove("conflict_metadata")
                .unwrap_or(serde_json::Value::Null);
            obj.remove("workflow_coverage");
            obj.remove("evidence_role_summary");
            obj.remove("security_evidence_summary");
            if let Some(r) = retrieval {
                obj.insert("retrieval_status".to_string(), retrieval_status_minimal(&r));
            }
            obj.insert(
                "conflict_indicator".to_string(),
                conflict_indicator(&conflicts),
            );
            if let Some(r) = routing {
                obj.insert("routing_summary".to_string(), routing_summary(&r));
            }
            if let Some(c) = capability {
                if !c.is_null() {
                    obj.insert("capability_summary".to_string(), capability_summary(&c));
                }
            }
        } else if let Some(r) = obj.remove("routing_decision") {
            obj.insert("routing_summary".to_string(), routing_summary(&r));
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
            if let Some(links) = obj.remove("links") {
                let seen = obj
                    .get("links_seen")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_else(|| links.as_array().map_or(0, |links| links.len()) as u64);
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
