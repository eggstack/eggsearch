use super::batch_fetch::{
    batch_payload_budget_warning, batch_payload_chars, batch_total_budget_warning,
    truncate_batch_result_to_budget_for_detail,
};

pub(super) fn enforce_projected_batch_total_budget(
    mut value: serde_json::Value,
    total_cap: usize,
    brief_max_chars: usize,
) -> serde_json::Value {
    let mut total_payload_chars = 0usize;
    let mut new_warnings = Vec::new();
    let mut budget_warning = None;
    let mut last_item_label = None;
    let mut newly_failed = 0usize;
    let mut newly_truncated = 0usize;
    let mut chars_removed = 0usize;
    let mut chars_added = 0usize;
    let mut budget_changed = false;

    {
        let Some(results) = value
            .get_mut("results")
            .and_then(serde_json::Value::as_array_mut)
        else {
            return value;
        };
        for (position, item) in results.iter_mut().enumerate() {
            let Some(object) = item.as_object_mut() else {
                continue;
            };
            let item_type =
                if object.get("item_type").and_then(serde_json::Value::as_str) == Some("repo") {
                    crate::core::batch_fetch::BatchFetchItemType::Repo
                } else {
                    crate::core::batch_fetch::BatchFetchItemType::Web
                };
            let label = object
                .get("label")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            last_item_label = Some(label.clone());
            let current_payload_chars = object
                .get("response")
                .filter(|response| !response.is_null())
                .and_then(batch_payload_chars)
                .unwrap_or(0);
            if object.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
                object.insert("payload_chars".into(), 0usize.into());
                continue;
            }

            let remaining = total_cap.saturating_sub(total_payload_chars);
            if current_payload_chars <= remaining {
                object.insert("payload_chars".into(), current_payload_chars.into());
                total_payload_chars = total_payload_chars.saturating_add(current_payload_chars);
                continue;
            }

            let mut result = crate::core::batch_fetch::BatchFetchResult {
                index: object
                    .get("index")
                    .and_then(serde_json::Value::as_u64)
                    .map(|index| index as usize)
                    .unwrap_or(position),
                item_type,
                label,
                stable_id: object
                    .get("stable_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                ok: true,
                response: object.get("response").cloned(),
                error: object
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                chars_returned: object
                    .get("chars_returned")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as usize,
                payload_chars: current_payload_chars,
                degraded_fields: object
                    .get("degraded_fields")
                    .and_then(serde_json::Value::as_array)
                    .map(|fields| {
                        fields
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
                truncated: object
                    .get("truncated")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            };
            let before_chars_returned = result.chars_returned;
            let before_truncated = result.truncated;
            result = truncate_batch_result_to_budget_for_detail(
                result,
                remaining,
                total_cap,
                crate::mcp::projection::ResponseDetail::Diagnostic,
                brief_max_chars,
            );
            budget_changed = true;
            if result.ok && !result.degraded_fields.is_empty() {
                new_warnings.push(batch_payload_budget_warning(&result, remaining, total_cap));
                if budget_warning.is_none() {
                    budget_warning = Some(batch_total_budget_warning(
                        &result.label,
                        "was degraded",
                        remaining,
                        total_cap,
                    ));
                }
            }
            if !result.ok {
                newly_failed += 1;
                if budget_warning.is_none() {
                    budget_warning = Some(batch_total_budget_warning(
                        &result.label,
                        "was omitted",
                        remaining,
                        total_cap,
                    ));
                }
            }
            if result.chars_returned < before_chars_returned {
                chars_removed =
                    chars_removed.saturating_add(before_chars_returned - result.chars_returned);
            } else {
                chars_added =
                    chars_added.saturating_add(result.chars_returned - before_chars_returned);
            }
            if result.truncated && !before_truncated {
                newly_truncated += 1;
            }
            total_payload_chars = total_payload_chars.saturating_add(result.payload_chars);
            object.insert("ok".into(), result.ok.into());
            object.insert(
                "response".into(),
                result.response.unwrap_or(serde_json::Value::Null),
            );
            if let Some(error) = result.error {
                object.insert("error".into(), error.into());
            } else {
                object.remove("error");
            }
            object.insert("chars_returned".into(), result.chars_returned.into());
            object.insert("payload_chars".into(), result.payload_chars.into());
            if result.degraded_fields.is_empty() {
                object.remove("degraded_fields");
            } else {
                object.insert("degraded_fields".into(), result.degraded_fields.into());
            }
            object.insert("truncated".into(), result.truncated.into());
        }
    }

    if total_payload_chars >= total_cap {
        budget_changed = true;
        if budget_warning.is_none() {
            if let Some(label) = last_item_label.as_deref() {
                budget_warning = Some(batch_total_budget_warning(
                    label,
                    "reached the batch limit",
                    0,
                    total_cap,
                ));
            }
        }
    }
    if let Some(warning) = budget_warning {
        new_warnings.push(warning);
    }

    let structured_warnings = crate::core::warning::convert_fetch_warnings(&new_warnings)
        .into_iter()
        .filter_map(|warning| serde_json::to_value(warning).ok())
        .collect::<Vec<_>>();
    if let Some(object) = value.as_object_mut() {
        let warning_values = object
            .entry("warnings")
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let Some(warnings) = warning_values.as_array_mut() {
            warnings.extend(new_warnings.into_iter().map(serde_json::Value::String));
        }
        let structured_values = object
            .entry("structured_warnings")
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let Some(warnings) = structured_values.as_array_mut() {
            warnings.extend(structured_warnings);
        }
    }

    if let Some(object) = value.as_object_mut() {
        object.insert("total_payload_chars".into(), total_payload_chars.into());
        if newly_failed > 0 {
            let fetched = object
                .get("fetched")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as usize;
            let failed = object
                .get("failed")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as usize;
            object.insert(
                "fetched".into(),
                fetched.saturating_sub(newly_failed).into(),
            );
            object.insert("failed".into(), failed.saturating_add(newly_failed).into());
        }
        if chars_removed > 0 || chars_added > 0 {
            let total = object
                .get("total_chars_returned")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as usize;
            object.insert(
                "total_chars_returned".into(),
                total
                    .saturating_sub(chars_removed)
                    .saturating_add(chars_added)
                    .into(),
            );
        }
        if newly_truncated > 0 {
            object.insert("truncated".into(), true.into());
        }
        if budget_changed {
            object.insert("output_truncated".into(), true.into());
        }
        if let Some(telemetry) = object
            .get_mut("telemetry")
            .and_then(serde_json::Value::as_object_mut)
        {
            if newly_failed > 0 {
                let completed = telemetry
                    .get("items_completed")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as usize;
                let failed = telemetry
                    .get("items_failed")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as usize;
                telemetry.insert(
                    "items_completed".into(),
                    completed.saturating_sub(newly_failed).into(),
                );
                telemetry.insert(
                    "items_failed".into(),
                    failed.saturating_add(newly_failed).into(),
                );
            }
            if newly_truncated > 0 {
                let truncated = telemetry
                    .get("items_truncated")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as usize;
                telemetry.insert(
                    "items_truncated".into(),
                    truncated.saturating_add(newly_truncated).into(),
                );
            }
            if chars_removed > 0 || chars_added > 0 {
                let total = telemetry
                    .get("total_chars_returned")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as usize;
                telemetry.insert(
                    "total_chars_returned".into(),
                    total
                        .saturating_sub(chars_removed)
                        .saturating_add(chars_added)
                        .into(),
                );
            }
            if budget_changed {
                telemetry.insert("aggregate_budget_exhausted".into(), true.into());
            }
        }
    }
    value
}
