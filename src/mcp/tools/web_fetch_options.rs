use super::common::*;
use crate::mcp::state::ServerState;

pub(crate) fn is_none_or_false(value: &Option<bool>) -> bool {
    !value.unwrap_or(false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenderRequest {
    HttpOnly,
    Auto,
    Browser,
}

pub(crate) fn parse_render_request(raw: Option<&str>) -> Result<RenderRequest, ToolError> {
    match raw.unwrap_or("http_only") {
        "http_only" => Ok(RenderRequest::HttpOnly),
        "auto" => Ok(RenderRequest::Auto),
        "browser" => Ok(RenderRequest::Browser),
        other => Err(ToolError::Validation(format!(
            "invalid render policy '{other}': expected http_only, auto, or browser"
        ))),
    }
}

#[cfg(feature = "browser")]
pub(crate) fn browser_fetch_tool_error(
    error: crate::fetch::browser::BrowserFetchError,
    used_profile_name: Option<&str>,
) -> ToolError {
    use crate::fetch::browser::BrowserFetchError;
    match error {
        BrowserFetchError::InteractiveChallenge(required) => {
            let next_action = match used_profile_name {
                Some(name) => {
                    format!(
                        "eggsearch browser-login {} --profile {name}",
                        required.origin
                    )
                }
                None => format!(
                    "eggsearch browser-login {} --profile <name>",
                    required.origin
                ),
            };
            super::common::browser_manual_interaction_error(
                &required.origin,
                &required.message,
                used_profile_name,
                Some(&next_action),
            )
        }
        BrowserFetchError::LaunchFailed(reason) => {
            super::common::browser_unavailable_error(&format!("browser launch failed: {reason}"))
        }
        BrowserFetchError::HttpStatus(status) => {
            ToolError::upstream_failed(format!("http_status: HTTP error: {status}"))
        }
        other => ToolError::internal(format!("browser_fetch_failed: {other}")),
    }
}

pub(crate) fn fetch_tool_error(error: crate::fetch::FetchError) -> ToolError {
    use crate::fetch::FetchErrorKind;
    let code = error.error_code();
    let mut message = format!("{code}: {error}");
    if code == "unsupported_content_type"
        && !cfg!(feature = "documents")
        && ["wordprocessingml", "spreadsheetml"]
            .iter()
            .any(|marker| message.contains(marker))
    {
        message.push_str(
            "; DOCX/XLSX extraction is not compiled into this build; rebuild with --features documents (see docs/features.md)",
        );
    }
    if code == "document_unsupported" {
        return ToolError::execution_with_data(
            ToolErrorCode::CapabilityUnavailable,
            message,
            serde_json::json!({"code": code}),
        );
    }
    if code.starts_with("proxy_") {
        return ToolError::execution_with_data(
            ToolErrorCode::UpstreamFailed,
            message,
            serde_json::json!({"code": code}),
        );
    }
    let mapped = match error.kind() {
        FetchErrorKind::PdfNotCompiledIn
        | FetchErrorKind::PdfDisabled
        | FetchErrorKind::DocumentDisabled
        | FetchErrorKind::BrowserNotCompiledIn
        | FetchErrorKind::BrowserDisabled
        | FetchErrorKind::BrowserNotFound
        | FetchErrorKind::BrowserLaunchFailed
        | FetchErrorKind::PdfOcrUnavailable => ToolErrorCode::CapabilityUnavailable,
        FetchErrorKind::PdfPageSpecInvalid
        | FetchErrorKind::PdfPageOutOfRange
        | FetchErrorKind::PdfPageCapExceeded
        | FetchErrorKind::InvalidUrl
        | FetchErrorKind::UnsupportedScheme => ToolErrorCode::InvalidSemanticValue,
        FetchErrorKind::PrivateNetworkBlocked
        | FetchErrorKind::RedirectTargetBlocked
        | FetchErrorKind::ProxyRouteUnconfigured
        | FetchErrorKind::ProxySchemeUnsupported => ToolErrorCode::PolicyDenied,
        FetchErrorKind::BrowserInteractiveChallenge => ToolErrorCode::ManualInteractionRequired,
        _ => ToolErrorCode::UpstreamFailed,
    };
    match mapped {
        ToolErrorCode::UpstreamFailed => ToolError::upstream_failed(message),
        other => ToolError::execution_with_data(other, message, serde_json::json!({"code": code})),
    }
}

pub(crate) fn selector_validation_error(
    error: crate::fetch::scope::SelectorValidationError,
    item_index: Option<usize>,
) -> ToolError {
    let prefix = item_index
        .map(|index| format!("item {index}: "))
        .unwrap_or_default();
    let mut data = serde_json::json!({
        "code": "invalid_selector",
        "group": error.group,
        "index": error.index,
    });
    if let Some(index) = item_index {
        data["item_index"] = serde_json::Value::from(index);
    }
    ToolError::execution_with_data(
        ToolErrorCode::InvalidSemanticValue,
        format!("{prefix}invalid_selector: {error}"),
        data,
    )
}

pub(crate) fn cache_only_render_conflict(item_index: Option<usize>) -> ToolError {
    let prefix = item_index
        .map(|index| format!("item {index}: "))
        .unwrap_or_default();
    let mut data = serde_json::json!({
        "code": "cache_only_render_conflict",
        "cache_entry_state": "browser_render_requested",
    });
    if let Some(index) = item_index {
        data["item_index"] = serde_json::Value::from(index);
    }
    ToolError::execution_with_data(
        ToolErrorCode::ConflictingArguments,
        format!("{prefix}cache_only_render_conflict: cache_only cannot use browser rendering or screenshots"),
        data,
    )
}

pub(crate) fn cache_only_miss(cache_entry_state: &str) -> ToolError {
    ToolError::execution_with_data(
        ToolErrorCode::DocumentUnavailable,
        format!("cache_only_miss: no matching stored derived cache entry ({cache_entry_state})"),
        serde_json::json!({
            "code": "cache_only_miss",
            "cache_entry_state": cache_entry_state,
        }),
    )
}

pub(crate) fn validate_batch_route_options(
    state: &ServerState,
    url: &str,
    render: Option<&str>,
    browser_profile: Option<&str>,
    include_screenshot: bool,
    _enforce_runtime_availability: bool,
) -> Result<(), ToolError> {
    #[cfg(not(feature = "browser"))]
    let _ = url;

    let configured_render = if state.config.fetch.browser.enabled {
        match state.config.fetch.browser.policy.as_str() {
            "http_only" | "auto" | "browser" => state.config.fetch.browser.policy.as_str(),
            _ => "http_only",
        }
    } else {
        "http_only"
    };
    let parsed_render_request = parse_render_request(Some(render.unwrap_or(configured_render)))?;
    if include_screenshot && render == Some("http_only") {
        return Err(ToolError::conflicting_arguments(
            "include_screenshot=true conflicts with explicit render=http_only",
            Some("render"),
        ));
    }
    let render_request = if include_screenshot {
        RenderRequest::Browser
    } else {
        parsed_render_request
    };

    #[cfg(not(feature = "browser"))]
    {
        if matches!(render_request, RenderRequest::Browser) || browser_profile.is_some() {
            return Err(browser_not_compiled_error());
        }
    }

    #[cfg(feature = "browser")]
    {
        use crate::fetch::browser::RenderPolicy;
        let render_policy = match render_request {
            RenderRequest::HttpOnly => RenderPolicy::HttpOnly,
            RenderRequest::Auto => RenderPolicy::Auto,
            RenderRequest::Browser => RenderPolicy::Browser,
        };
        if matches!(render_policy, RenderPolicy::HttpOnly) && browser_profile.is_some() {
            if !state.config.fetch.browser.enabled {
                return Err(browser_disabled_error());
            }
            return Err(ToolError::Validation(
                "browser_profile is not valid with render=http_only".to_string(),
            ));
        }
        let explicit_browser =
            matches!(render_policy, RenderPolicy::Browser) || browser_profile.is_some();
        match state.browser_lifecycle() {
            None if explicit_browser && _enforce_runtime_availability => {
                return Err(browser_disabled_error())
            }
            Some(lifecycle)
                if !lifecycle.is_available()
                    && explicit_browser
                    && _enforce_runtime_availability =>
            {
                return Err(browser_unavailable_error(
                    &crate::fetch::browser::browser_discovery_reason(
                        &state.browser_discovery_state,
                    )
                    .unwrap_or_else(|| "browser rendering is unavailable".to_string()),
                ));
            }
            _ => {}
        }
        if let Some(profile_name) = browser_profile {
            let mgr = state.profile_manager.as_ref().ok_or_else(|| {
                ToolError::Validation(
                    "browser profiles are not enabled; \
                     set [fetch.browser.persistent_profiles].enabled = true"
                        .to_string(),
                )
            })?;
            let parsed_url = url::Url::parse(url)
                .map_err(|error| ToolError::Validation(format!("invalid URL: {error}")))?;
            let request_origin = format!("{}://{}", parsed_url.scheme(), parsed_url.authority());
            mgr.resolve_for_origin(profile_name, &request_origin)
                .map_err(|error| match error {
                    crate::fetch::browser::ProfileError::ProfileNotFound(_) => {
                        let command = format!("eggsearch browser-login {request_origin} --profile {profile_name}");
                        ToolError::execution_with_repair(
                            ToolErrorCode::ManualInteractionRequired,
                            format!("browser_profile: {profile_name}; run `{command}`"),
                            RepairHint::new(
                                Some("browser_profile"),
                                &["an existing or new persistent browser profile"],
                                Some(&command),
                            ),
                        )
                    }
                    crate::fetch::browser::ProfileError::ProfilesDisabled => {
                        ToolError::Validation("browser profiles are not enabled; set [fetch.browser.persistent_profiles].enabled = true".to_string())
                    }
                    other => ToolError::internal(format!("browser_profile: {other}")),
                })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::FetchError;

    #[test]
    fn capability_misses_are_not_reported_as_upstream_failures() {
        assert_eq!(
            fetch_tool_error(FetchError::PdfNotCompiledIn).code(),
            ToolErrorCode::CapabilityUnavailable
        );
        assert_eq!(
            fetch_tool_error(FetchError::PdfDisabled).code(),
            ToolErrorCode::CapabilityUnavailable
        );
        assert_eq!(
            fetch_tool_error(FetchError::DocumentDisabled).code(),
            ToolErrorCode::CapabilityUnavailable
        );
    }

    #[test]
    fn genuine_upstream_failures_keep_the_upstream_code() {
        assert_eq!(
            fetch_tool_error(FetchError::HttpStatus(503, String::new())).code(),
            ToolErrorCode::UpstreamFailed
        );
    }

    #[test]
    fn http_status_text_is_not_duplicated() {
        let message = fetch_tool_error(FetchError::HttpStatus(404, String::new())).to_string();
        assert_eq!(message, "http_status: HTTP error: 404");
        assert_eq!(message.matches("404").count(), 1);
    }

    #[cfg(not(feature = "documents"))]
    #[test]
    fn a_container_document_without_the_feature_names_the_rebuild() {
        let message = fetch_tool_error(FetchError::UnsupportedContentType(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
        ))
        .to_string();
        assert!(
            message.contains("--features documents"),
            "the hint must name the rebuild: {message}"
        );
    }

    #[cfg(feature = "browser")]
    #[test]
    fn browser_status_failures_keep_the_http_transport_shape() {
        let error = browser_fetch_tool_error(
            crate::fetch::browser::BrowserFetchError::HttpStatus(404),
            None,
        );
        assert_eq!(error.code(), ToolErrorCode::UpstreamFailed);
        assert_eq!(error.to_string(), "http_status: HTTP error: 404");
    }
}
