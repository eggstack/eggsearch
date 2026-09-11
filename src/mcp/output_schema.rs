use std::sync::Arc;

use rmcp::model::JsonObject;

fn schema_from_value(value: serde_json::Value) -> Arc<JsonObject> {
    match value {
        serde_json::Value::Object(map) => Arc::new(map),
        _ => Arc::new(JsonObject::new()),
    }
}

fn permissive_object_schema(
    description: &str,
    properties: serde_json::Value,
    required: &[&str],
) -> Arc<JsonObject> {
    schema_from_value(serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "description": description,
        "properties": properties,
        "required": required,
        "additionalProperties": true,
    }))
}

pub fn output_schema_for(tool: &str) -> Option<Arc<JsonObject>> {
    match tool {
        "batch_fetch" => Some(permissive_object_schema(
            "Bounded multi-target fetch envelope with per-item results and aggregate telemetry.",
            serde_json::json!({
                "results": {"type": "array"},
                "fetched": {"type": "integer"},
                "failed": {"type": "integer"},
                "warnings": {"type": "array"},
                "structured_warnings": {"type": "array"},
                "telemetry": {"type": "object"},
            }),
            &["results"],
        )),
        "build_evidence_bundle" => Some(permissive_object_schema(
            "Deterministic evidence packaging envelope for handoff.",
            serde_json::json!({
                "sources": {"type": "array"},
                "fetches": {"type": "array"},
                "warnings": {"type": "array"},
            }),
            &[],
        )),
        "repo_fetch" => Some(permissive_object_schema(
            "Bounded repository file or span envelope.",
            serde_json::json!({
                "text": {"type": ["string", "null"]},
                "warnings": {"type": "array"},
                "structured_warnings": {"type": "array"},
                "trust_markers": {"type": "object"},
            }),
            &[],
        )),
        "repo_map" => Some(permissive_object_schema(
            "Repository structure discovery envelope without file contents.",
            serde_json::json!({
                "warnings": {"type": "array"},
                "structured_warnings": {"type": "array"},
                "structure_truncated": {"type": "boolean"},
            }),
            &[],
        )),
        "repo_search" => Some(permissive_object_schema(
            "Structured repository evidence envelope with grouped bundles.",
            serde_json::json!({
                "results": {"type": "array"},
                "warnings": {"type": "array"},
                "structured_warnings": {"type": "array"},
                "next_actions": {"type": "array"},
                "retrieval_summary": {"type": "object"},
            }),
            &[],
        )),
        "research_search" => Some(permissive_object_schema(
            "Multi-source research evidence envelope with grouped bundles.",
            serde_json::json!({
                "results": {"type": "array"},
                "warnings": {"type": "array"},
                "structured_warnings": {"type": "array"},
                "next_actions": {"type": "array"},
            }),
            &[],
        )),
        "security_search" => Some(permissive_object_schema(
            "Vulnerability and advisory evidence envelope with applicability context.",
            serde_json::json!({
                "results": {"type": "array"},
                "warnings": {"type": "array"},
                "structured_warnings": {"type": "array"},
                "next_actions": {"type": "array"},
            }),
            &[],
        )),
        "web_fetch" => Some(permissive_object_schema(
            "Bounded single-URL fetch envelope. Stable envelope with permissive transport/cache metadata.",
            serde_json::json!({
                "url": {"type": "string"},
                "final_url": {"type": "string"},
                "status": {"type": "integer"},
                "fetched": {"type": "boolean"},
                "truncated": {"type": "boolean"},
                "text": {"type": ["string", "null"]},
                "warnings": {"type": "array"},
                "structured_warnings": {"type": "array"},
            }),
            &["url", "final_url", "status", "fetched", "truncated"],
        )),
        "web_search" => Some(permissive_object_schema(
            "Web source discovery envelope. Stable query/results envelope with permissive routing metadata.",
            serde_json::json!({
                "query": {"type": "string"},
                "results": {"type": "array"},
                "providers_queried": {"type": "array"},
                "warnings": {"type": "array"},
                "structured_warnings": {"type": "array"},
                "next_actions": {"type": "array"},
            }),
            &["query", "results"],
        )),
        "provider_status" => Some(permissive_object_schema(
            "Diagnostic provider and capability report envelope. Open-ended capability metadata is permissive.",
            serde_json::json!({
                "providers": {"type": "array"},
                "mode": {"type": "string"},
                "server_capabilities": {"type": "object"},
                "probe": {"type": "object"},
            }),
            &["providers"],
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_stable_tools_have_output_schemas() {
        for name in crate::mcp::tool_contract::tool_names() {
            assert!(
                output_schema_for(name).is_some(),
                "tool '{name}' must have an output schema or a documented exception"
            );
        }
    }

    #[test]
    fn output_schemas_are_objects() {
        for name in crate::mcp::tool_contract::tool_names() {
            let schema = output_schema_for(name).expect("schema exists");
            assert_eq!(
                schema.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "tool '{name}' output schema must be an object"
            );
        }
    }

    #[test]
    fn output_schemas_stay_compact() {
        for name in crate::mcp::tool_contract::tool_names() {
            let schema = output_schema_for(name).expect("schema exists");
            let bytes = serde_json::to_string(&schema).unwrap().len();
            assert!(
                bytes <= 1200,
                "tool '{name}' output schema is {bytes} bytes, over 1200 budget"
            );
        }
    }
}
