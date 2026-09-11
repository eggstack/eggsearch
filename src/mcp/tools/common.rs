use crate::mcp::state::ServerState;
use std::sync::Arc;

/// Stable machine-readable tool error code.
///
/// Codes are part of the repairable error contract. Agents may match on
/// `code` to decide whether a retry with repaired arguments is worthwhile.
/// Codes are stable; new codes are additive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorCode {
    InvalidSemanticValue,
    ConflictingArguments,
    CapabilityUnavailable,
    ProviderUnavailable,
    PolicyDenied,
    BudgetInvalid,
    LocatorInvalid,
    ManualInteractionRequired,
    UpstreamFailed,
    InvalidRequest,
    Internal,
}

impl ToolErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidSemanticValue => "invalid_semantic_value",
            Self::ConflictingArguments => "conflicting_arguments",
            Self::CapabilityUnavailable => "capability_unavailable",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::PolicyDenied => "policy_denied",
            Self::BudgetInvalid => "budget_invalid",
            Self::LocatorInvalid => "locator_invalid",
            Self::ManualInteractionRequired => "manual_interaction_required",
            Self::UpstreamFailed => "upstream_failed",
            Self::InvalidRequest => "invalid_request",
            Self::Internal => "internal",
        }
    }
}

/// Bounded machine-readable repair hint for repairable tool errors.
///
/// `accepted` is capped at 20 entries; each string field is length-bounded
/// by the constructor. Hints never carry hidden reasoning.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RepairHint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default)]
    pub accepted: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_value: Option<String>,
}

impl RepairHint {
    fn bound_str(s: &str, cap: usize) -> String {
        let mut out: String = s.chars().take(cap).collect();
        out.truncate(cap);
        out
    }

    pub fn new(field: Option<&str>, accepted: &[&str], suggested_value: Option<&str>) -> Self {
        Self {
            field: field
                .map(|f| Self::bound_str(f.trim(), 128))
                .filter(|f| !f.is_empty()),
            accepted: accepted
                .iter()
                .take(20)
                .map(|a| Self::bound_str(a.trim(), 128))
                .filter(|a| !a.is_empty())
                .collect(),
            suggested_value: suggested_value
                .map(|s| Self::bound_str(s.trim(), 256))
                .filter(|s| !s.is_empty()),
        }
    }
}

/// Recoverable semantic failure detail. Boxed inside `ToolError::Execution`
/// to keep `Result<_, ToolError>` below Clippy's large-error threshold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolErrorExecution {
    pub code: ToolErrorCode,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub repair: Option<RepairHint>,
}

/// Error from a tool call.
///
/// - `Validation` is the legacy semantic-failure variant. It is preserved
///   for compatibility and maps to a repairable `isError` tool result, not
///   to JSON-RPC `invalid_params`. New code should use `Execution`.
/// - `InvalidRequest` is a true protocol/input-shape failure that prevents
///   interpreting the invocation. It maps to JSON-RPC `invalid_params`.
/// - `Execution` is a recoverable semantic failure the caller can repair
///   and retry. It maps to an MCP tool error (`isError: true`) with a
///   stable `code` and bounded `repair` hint.
/// - `Internal` is a server-side/runtime failure. It maps to JSON-RPC
///   `internal_error` and never leaks stack traces.
#[derive(Debug)]
#[must_use = "tool errors must be returned to the MCP caller"]
pub enum ToolError {
    Validation(String),
    InvalidRequest(String),
    Execution(Box<ToolErrorExecution>),
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

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest(message.into())
    }

    pub fn execution(code: ToolErrorCode, message: impl Into<String>) -> Self {
        Self::Execution(Box::new(ToolErrorExecution {
            code,
            message: message.into(),
            data: None,
            repair: None,
        }))
    }

    pub fn execution_with_repair(
        code: ToolErrorCode,
        message: impl Into<String>,
        repair: RepairHint,
    ) -> Self {
        Self::Execution(Box::new(ToolErrorExecution {
            code,
            message: message.into(),
            data: None,
            repair: Some(repair),
        }))
    }

    pub fn execution_with_data(
        code: ToolErrorCode,
        message: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self::Execution(Box::new(ToolErrorExecution {
            code,
            message: message.into(),
            data: Some(data),
            repair: None,
        }))
    }

    pub fn invalid_semantic_value(field: &str, raw: &str, accepted: &[&str]) -> Self {
        let message = format!(
            "invalid {field} '{raw}'; accepted values: {}",
            accepted.join(", ")
        );
        Self::execution_with_repair(
            ToolErrorCode::InvalidSemanticValue,
            message,
            RepairHint::new(Some(field), accepted, None),
        )
    }

    pub fn conflicting_arguments(message: impl Into<String>, field: Option<&str>) -> Self {
        Self::execution_with_repair(
            ToolErrorCode::ConflictingArguments,
            message,
            RepairHint::new(field, &[], None),
        )
    }

    pub fn capability_unavailable(message: impl Into<String>) -> Self {
        Self::execution(ToolErrorCode::CapabilityUnavailable, message)
    }

    pub fn provider_unavailable(message: impl Into<String>) -> Self {
        Self::execution(ToolErrorCode::ProviderUnavailable, message)
    }

    pub fn policy_denied(message: impl Into<String>) -> Self {
        Self::execution(ToolErrorCode::PolicyDenied, message)
    }

    pub fn budget_invalid(message: impl Into<String>) -> Self {
        Self::execution(ToolErrorCode::BudgetInvalid, message)
    }

    pub fn locator_invalid(message: impl Into<String>) -> Self {
        Self::execution(ToolErrorCode::LocatorInvalid, message)
    }

    pub fn upstream_failed(message: impl Into<String>) -> Self {
        Self::execution(ToolErrorCode::UpstreamFailed, message)
    }

    pub fn code(&self) -> ToolErrorCode {
        match self {
            Self::Validation(_) => ToolErrorCode::InvalidSemanticValue,
            Self::InvalidRequest(_) => ToolErrorCode::InvalidRequest,
            Self::Execution(inner) => inner.code,
            Self::Internal { .. } => ToolErrorCode::Internal,
        }
    }

    fn inferred_code_for_legacy(message: &str) -> ToolErrorCode {
        let lower = message.to_ascii_lowercase();
        if lower.contains("conflicting") {
            ToolErrorCode::ConflictingArguments
        } else if lower.contains("budget")
            || lower.contains("must be > 0")
            || lower.contains("must not exceed")
            || lower.contains("max_")
        {
            ToolErrorCode::BudgetInvalid
        } else if lower.contains("unknown host")
            || lower.contains("unknown provider")
            || lower.contains("unsupported provider")
            || lower.contains("provider")
        {
            ToolErrorCode::ProviderUnavailable
        } else if lower.contains("not enabled")
            || lower.contains("disabled")
            || lower.contains("unavailable")
            || lower.contains("not compiled")
        {
            ToolErrorCode::CapabilityUnavailable
        } else if lower.contains("denied") || lower.contains("not allowed") {
            ToolErrorCode::PolicyDenied
        } else if lower.contains("locator")
            || lower.contains("url scheme")
            || lower.contains("url must")
            || lower.contains("path")
            || lower.contains("ref ")
            || lower.contains("commit")
        {
            ToolErrorCode::LocatorInvalid
        } else if lower.contains("manual_interaction") || lower.contains("browser") {
            ToolErrorCode::ManualInteractionRequired
        } else if lower.contains("all providers failed") || lower.contains("upstream") {
            ToolErrorCode::UpstreamFailed
        } else {
            ToolErrorCode::InvalidSemanticValue
        }
    }

    pub fn error_payload(&self) -> serde_json::Value {
        match self {
            Self::Validation(message) => {
                let code = Self::inferred_code_for_legacy(message);
                serde_json::json!({
                    "code": code.as_str(),
                    "message": message,
                })
            }
            Self::InvalidRequest(message) => serde_json::json!({
                "code": ToolErrorCode::InvalidRequest.as_str(),
                "message": message,
            }),
            Self::Execution(inner) => {
                let mut payload = serde_json::json!({
                    "code": inner.code.as_str(),
                    "message": inner.message,
                });
                if let Some(data) = &inner.data {
                    payload["data"] = data.clone();
                }
                if let Some(repair) = &inner.repair {
                    payload["repair"] =
                        serde_json::to_value(repair).unwrap_or(serde_json::Value::Null);
                }
                payload
            }
            Self::Internal { message, data } => {
                let mut payload = serde_json::json!({
                    "code": ToolErrorCode::Internal.as_str(),
                    "message": message,
                });
                if let Some(data) = data {
                    payload["data"] = data.clone();
                }
                payload
            }
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(msg) | Self::InvalidRequest(msg) => {
                write!(f, "{msg}")
            }
            Self::Execution(inner) => write!(f, "{}", inner.message),
            Self::Internal { message: msg, .. } => {
                write!(f, "{msg}")
            }
        }
    }
}

/// Central MCP error/result conversion seam.
///
/// - `Ok(value)` becomes a native structured success (`structuredContent`
///   plus a text fallback for older clients).
/// - `InvalidRequest` becomes JSON-RPC `invalid_params` (the invocation
///   shape cannot be interpreted).
/// - `Validation` and `Execution` become repairable MCP tool errors
///   (`isError: true`) with a stable `code` and bounded `repair` hint.
/// - `Internal` becomes JSON-RPC `internal_error` without stack traces.
pub fn map_tool_result(
    result: Result<serde_json::Value, ToolError>,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    match result {
        Ok(value) => Ok(rmcp::model::CallToolResult::structured(value)),
        Err(ToolError::InvalidRequest(message)) => {
            Err(rmcp::ErrorData::invalid_params(message, None))
        }
        Err(e @ ToolError::Validation(_)) | Err(e @ ToolError::Execution(_)) => Ok(
            rmcp::model::CallToolResult::structured_error(e.error_payload()),
        ),
        Err(ToolError::Internal { message, data }) => {
            Err(rmcp::ErrorData::internal_error(message, data))
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
    ToolError::execution_with_data(ToolErrorCode::ManualInteractionRequired, error_msg, data)
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
    ToolError::execution_with_data(ToolErrorCode::ManualInteractionRequired, error_msg, data)
}

#[cfg(feature = "browser")]
pub(crate) fn browser_unavailable_error(reason: &str) -> ToolError {
    let data = serde_json::json!({
        "code": "browser_unavailable",
        "message": reason,
        "manual_interaction_required": false,
    });
    ToolError::execution_with_data(
        ToolErrorCode::CapabilityUnavailable,
        reason.to_string(),
        data,
    )
}

#[cfg(feature = "browser")]
pub(crate) fn browser_deadline_exceeded_error() -> ToolError {
    let data = serde_json::json!({
        "code": "browser_deadline_exceeded",
        "message": "insufficient time remaining for browser rendering",
        "manual_interaction_required": false,
    });
    ToolError::execution_with_data(
        ToolErrorCode::BudgetInvalid,
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
