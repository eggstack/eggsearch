use crate::mcp::state::ServerState;
use std::sync::Arc;

/// Error from a tool call, tagged by whether it reflects bad client
/// input (`Validation`) or a server-side/runtime issue (`Internal`).
///
/// `Internal` errors optionally carry structured JSON data for
/// machine-readable error codes (e.g. browser/manual-interaction outcomes).
/// The `data` field is passed through the MCP error response's `data`
/// member when present.
#[derive(Debug)]
#[must_use = "tool errors must be returned to the MCP caller"]
pub enum ToolError {
    Validation(String),
    Internal {
        message: String,
        data: Option<serde_json::Value>,
    },
}

impl ToolError {
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            data: None,
        }
    }

    pub fn internal_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self::Internal {
            message: message.into(),
            data: Some(data),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(msg) | Self::Internal { message: msg, .. } => {
                write!(f, "{msg}")
            }
        }
    }
}

#[cfg(feature = "browser")]
pub(crate) fn browser_manual_interaction_error(
    origin: &str,
    message: &str,
    profile_name: Option<&str>,
    next_action: Option<&str>,
) -> ToolError {
    let mut data = serde_json::json!({
        "code": "browser_manual_interaction_required",
        "message": message,
        "origin": origin,
        "manual_interaction_required": true,
    });
    if let Some(name) = profile_name {
        data["profile_name"] = serde_json::Value::String(name.to_string());
    }
    if let Some(action) = next_action {
        data["next_action"] = serde_json::Value::String(action.to_string());
    }
    let error_msg = format!("manual_interaction_required: {origin}: {message}");
    ToolError::internal_with_data(error_msg, data)
}

#[cfg(feature = "browser")]
pub(crate) fn browser_profile_requires_attention_error(
    origin: &str,
    profile_name: &str,
) -> ToolError {
    let next_action = format!("eggsearch browser-login {origin} --profile {profile_name}");
    let data = serde_json::json!({
        "code": "browser_profile_requires_attention",
        "message": format!("browser profile '{profile_name}' requires manual login for origin {origin}"),
        "origin": origin,
        "profile_name": profile_name,
        "manual_interaction_required": true,
        "next_action": next_action,
    });
    let error_msg = format!(
        "browser_profile_requires_attention: profile '{profile_name}' requires manual login for {origin}; \
         reopen with: {next_action}"
    );
    ToolError::internal_with_data(error_msg, data)
}

#[cfg(feature = "browser")]
pub(crate) fn browser_unavailable_error(reason: &str) -> ToolError {
    let data = serde_json::json!({
        "code": "browser_unavailable",
        "message": reason,
        "manual_interaction_required": false,
    });
    ToolError::internal_with_data(reason.to_string(), data)
}

#[cfg(feature = "browser")]
pub(crate) fn browser_deadline_exceeded_error() -> ToolError {
    let data = serde_json::json!({
        "code": "browser_deadline_exceeded",
        "message": "insufficient time remaining for browser rendering",
        "manual_interaction_required": false,
    });
    ToolError::internal_with_data(
        "insufficient time remaining for browser rendering".to_string(),
        data,
    )
}

pub(crate) fn parse_code_host_arg(
    host: Option<&str>,
) -> Result<Option<crate::core::code_metadata::CodeHost>, ToolError> {
    use crate::core::code_metadata::CodeHost;

    let Some(host) = host else {
        return Ok(None);
    };

    let parsed = CodeHost::parse_alias(host).ok_or_else(|| {
        ToolError::Validation(format!(
            "unknown host '{}'; accepted values: {}",
            host.trim().to_ascii_lowercase(),
            CodeHost::accepted_aliases()
        ))
    })?;

    Ok(Some(parsed))
}

pub(crate) fn parse_symbol_kind_arg(
    symbol_kind: Option<&str>,
) -> Result<Option<crate::core::code_evidence::SymbolKind>, ToolError> {
    use crate::core::code_evidence::SymbolKind;

    let Some(raw) = symbol_kind else {
        return Ok(None);
    };

    let accepted: &[&str] = &[
        "function",
        "fn",
        "method",
        "struct",
        "enum",
        "trait",
        "class",
        "interface",
        "module",
        "mod",
        "constant",
        "const",
        "static",
        "type",
        "typealias",
        "macro",
    ];

    let parsed = match raw.to_ascii_lowercase().as_str() {
        "function" | "fn" => Some(SymbolKind::Function),
        "method" => Some(SymbolKind::Method),
        "struct" => Some(SymbolKind::Struct),
        "enum" => Some(SymbolKind::Enum),
        "trait" => Some(SymbolKind::Trait),
        "class" => Some(SymbolKind::Class),
        "interface" => Some(SymbolKind::Interface),
        "module" | "mod" => Some(SymbolKind::Module),
        "constant" | "const" | "static" => Some(SymbolKind::Constant),
        "type" | "typealias" => Some(SymbolKind::TypeAlias),
        "macro" => Some(SymbolKind::Macro),
        _ => None,
    };

    parsed.map(Some).ok_or_else(|| {
        ToolError::Validation(format!(
            "invalid symbol_kind '{raw}'; accepted values: {}",
            accepted.join(", ")
        ))
    })
}

/// Strictly parse a single string argument into an enum-like value.
/// Returns `Ok(Some(value))` when `raw` is `Some` and parses
/// successfully, `Ok(None)` when `raw` is `None`, and `Err` when
/// `raw` is `Some` but does not match a known value. The `accepted`
/// list is shown in the error message.
pub(crate) fn parse_strict_enum_arg<T, F>(
    field: &str,
    raw: Option<&str>,
    parse: F,
    accepted: &[&str],
) -> Result<Option<T>, ToolError>
where
    F: FnOnce(&str) -> Option<T>,
{
    let Some(raw) = raw else {
        return Ok(None);
    };
    match parse(raw) {
        Some(value) => Ok(Some(value)),
        None => {
            let values: Vec<&str> = accepted
                .iter()
                .copied()
                .filter(|v| !v.starts_with("(aliases"))
                .collect();
            let hints: Vec<&str> = accepted
                .iter()
                .copied()
                .filter(|v| v.starts_with("(aliases"))
                .collect();
            let mut msg = format!(
                "invalid {field} '{raw}'; accepted values: {}",
                values.join(", ")
            );
            if !hints.is_empty() {
                msg.push_str("; ");
                msg.push_str(&hints.join(" "));
            }
            Err(ToolError::Validation(msg))
        }
    }
}

pub(crate) fn parse_strict_freshness(
    raw: Option<&str>,
) -> Result<Option<crate::core::query::Freshness>, ToolError> {
    use crate::core::query::Freshness;
    let Some(raw) = raw else {
        return Ok(None);
    };
    serde_json::from_value::<Freshness>(serde_json::Value::String(raw.to_string()))
        .map(Some)
        .map_err(|e| ToolError::Validation(format!("invalid freshness '{raw}': {e}")))
}

pub(crate) fn skip_reason_to_attempt(
    skip: &crate::meta::provider_diagnostics::ProviderSkipReason,
) -> crate::core::retrieval_status::RetrievalAttempt {
    use crate::core::evidence_role::EvidenceRole;
    use crate::core::provider::ProviderSkipCode;
    use crate::core::retrieval_status::{RetrievalAttempt, RetrievalAttemptOutcome};

    let outcome = match skip.skip_code {
        Some(ProviderSkipCode::CooldownActive) => RetrievalAttemptOutcome::SkippedByPolicy,
        Some(ProviderSkipCode::NotBuilt) => RetrievalAttemptOutcome::SkippedCapabilityUnavailable,
        Some(ProviderSkipCode::DisabledByUser) => RetrievalAttemptOutcome::SkippedByPolicy,
        Some(ProviderSkipCode::MissingApiKey)
        | Some(ProviderSkipCode::MissingSearxngConfig)
        | Some(ProviderSkipCode::MissingBaseUrl)
        | Some(ProviderSkipCode::InvalidBaseUrl)
        | Some(ProviderSkipCode::MissingLocalBackend)
        | Some(ProviderSkipCode::CredentialNotConfigured)
        | Some(ProviderSkipCode::CredentialEnvMissing)
        | Some(ProviderSkipCode::CredentialInvalid) => {
            RetrievalAttemptOutcome::SkippedCapabilityUnavailable
        }
        _ => RetrievalAttemptOutcome::SkippedByPolicy,
    };

    RetrievalAttempt {
        provider_id: skip.provider_id.clone(),
        subquery_id: None,
        operation_id: None,
        intended_roles: vec![EvidenceRole::UnknownOrWeakContext],
        outcome,
        result_count: 0,
        error_class: None,
        deadline_interrupted: false,
        truncated: false,
        truncation_evidence: Default::default(),
        query_fingerprint: Some(crate::core::retrieval_status::query_fingerprint_from_query(
            &skip.provider_id,
        )),
        duration_ms: None,
    }
}

pub(crate) fn merge_selection_stage_attempts(
    routing_decision: &crate::meta::provider_diagnostics::ProviderRoutingDecision,
    response_retrieval_summary: &mut Option<
        crate::core::retrieval_status::ResponseRetrievalSummary,
    >,
) {
    if routing_decision.skipped_providers.is_empty() {
        return;
    }

    let selection_attempts: Vec<crate::core::retrieval_status::RetrievalAttempt> = routing_decision
        .skipped_providers
        .iter()
        .map(skip_reason_to_attempt)
        .collect();

    if let Some(ref mut summary) = response_retrieval_summary {
        let selection_dims =
            crate::core::evidence_postprocess::build_retrieval_summary_from_attempts(
                &selection_attempts,
            );
        for dim in selection_dims.dimensions {
            summary.dimensions.push(dim);
        }
        summary.has_failures = summary.has_failures || selection_dims.has_failures;
        summary.has_absences = summary.has_absences || selection_dims.has_absences;
        summary.has_truncation = summary.has_truncation || selection_dims.has_truncation;
        if let Some(sel_attempted) = selection_dims.attempted_job_count {
            summary.attempted_job_count =
                Some(summary.attempted_job_count.unwrap_or(0) + sel_attempted);
        }
        if let Some(sel_completed) = selection_dims.completed_job_count {
            summary.completed_job_count =
                Some(summary.completed_job_count.unwrap_or(0) + sel_completed);
        }
        if let Some(sel_failed) = selection_dims.failed_job_count {
            summary.failed_job_count = Some(summary.failed_job_count.unwrap_or(0) + sel_failed);
        }
    } else {
        *response_retrieval_summary = Some(
            crate::core::evidence_postprocess::build_retrieval_summary_from_attempts(
                &selection_attempts,
            ),
        );
    }
}

#[cfg(feature = "browser")]
pub(crate) async fn run_browser_fetch(
    state: &ServerState,
    url: &str,
    sanitize_output: bool,
    policy: &crate::fetch::browser::RenderPolicy,
    profile_dir: Option<&std::path::Path>,
) -> Result<crate::fetch::browser::BrowserFetchResult, crate::fetch::browser::BrowserFetchError> {
    let shared = state.browser_lifecycle().ok_or_else(|| {
        crate::fetch::browser::BrowserFetchError::LaunchFailed(
            "browser lifecycle unavailable".to_string(),
        )
    })?;
    let config = shared.config();
    let request_lifecycle = profile_dir.map_or_else(
        || shared.clone(),
        |path| {
            Arc::new(
                crate::fetch::browser::BrowserLifecycle::for_persistent_profile(
                    shared.discovery().cloned(),
                    config.clone(),
                    path.to_path_buf(),
                ),
            )
        },
    );
    let result = crate::fetch::browser::browser_fetch_with_policy(
        &request_lifecycle,
        url,
        &config,
        sanitize_output,
        policy,
    )
    .await;
    if profile_dir.is_some() {
        request_lifecycle.close().await;
    }
    result
}

pub(crate) fn cached_document_response(
    requested_url: &str,
    raw: &crate::fetch::cache::RawFetchCacheEntry,
    document: &crate::fetch::cache::CachedExtractedDocument,
) -> crate::core::fetch::WebFetchResponse {
    let structured_document = document.document.clone().map(|mut structured_document| {
        if let Some(metadata) = structured_document.metadata.as_mut() {
            metadata.content_length = raw.content_length_header;
            metadata.redirects_followed = raw.redirect_count;
        }
        structured_document
    });
    crate::core::fetch::WebFetchResponse {
        url: requested_url.to_string(),
        final_url: raw.final_url.clone(),
        stable_id: Some(crate::core::identity::fetch_id(
            Some(requested_url),
            None,
            None,
            None,
            None,
        )),
        source_id: None,
        title: document.title.clone(),
        description: document.description.clone(),
        content_type: raw.content_type.clone(),
        status: raw.status,
        fetched: true,
        truncated: document.truncated,
        trust: crate::core::fetch::FetchTrust::ExternalUntrusted,
        text: document.text.clone(),
        raw_text: document.raw_text.clone(),
        raw_text_chars_returned: document.raw_text.as_ref().map(|text| text.chars().count()),
        raw_text_truncated: false,
        raw_text_cap: None,
        links: document.links.clone(),
        links_seen: document.links_seen,
        links_truncated: document.links_truncated,
        warnings: vec![crate::core::fetch::WebFetchResponse::untrusted_warning()],
        trust_markers: document.trust_markers.clone(),
        document: structured_document,
        fetch_transform: None,
        structured_warnings: Vec::new(),
        pdf_page_metadata: None,
        pdf_document_metadata: None,
        pdf_quality_score: None,
        pdf_content_ok: None,
        cache_status: crate::fetch::cache::CacheStatus::Hit,
        attempt_count: Some(1),
        retry_after_ms: None,
        origin_backoff_ms: None,
        response_headers: (!raw.headers.is_empty()).then(|| raw.headers.clone()),
        transport: document.transport.clone(),
        browser_escalated: document.browser_escalated,
        manual_interaction_required: false,
        focus: None,
        raw_body: None,
    }
}

pub(crate) fn derived_cache_entry(
    raw_hash: u64,
    key: &crate::fetch::cache::DerivedCacheKey,
    response: &crate::core::fetch::WebFetchResponse,
) -> crate::fetch::cache::DerivedDocumentCacheEntry {
    crate::fetch::cache::DerivedDocumentCacheEntry {
        raw_content_hash: raw_hash,
        extraction_key: key.extraction_key.clone(),
        response: crate::fetch::cache::CachedExtractedDocument {
            title: response.title.clone(),
            description: response.description.clone(),
            text: response.text.clone(),
            raw_text: response.raw_text.clone(),
            links: response.links.clone(),
            links_seen: response.links_seen,
            links_truncated: response.links_truncated,
            truncated: response.truncated,
            document: response.document.clone(),
            trust_markers: response.trust_markers.clone(),
            transport: response.transport.clone(),
            browser_escalated: response.browser_escalated,
        },
        created_at: std::time::SystemTime::now(),
    }
}
