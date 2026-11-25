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

pub(crate) fn fetch_tool_error(error: crate::fetch::FetchError) -> ToolError {
    let code = error.error_code();
    let message = format!("{code}: {error}");
    if code.starts_with("proxy_") {
        ToolError::execution_with_data(
            ToolErrorCode::UpstreamFailed,
            message,
            serde_json::json!({"code": code}),
        )
    } else {
        ToolError::upstream_failed(message)
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
                     set [fetch.browser].persistent_profiles_enabled = true"
                        .to_string(),
                )
            })?;
            let parsed_url = url::Url::parse(url)
                .map_err(|error| ToolError::Validation(format!("invalid URL: {error}")))?;
            let request_origin = format!("{}://{}", parsed_url.scheme(), parsed_url.authority());
            mgr.resolve_for_origin(profile_name, &request_origin)
                .map_err(|error| match error {
                    crate::fetch::browser::ProfileError::ProfileNotFound(message) => {
                        ToolError::Validation(format!("browser_profile: {message}"))
                    }
                    crate::fetch::browser::ProfileError::ProfilesDisabled => {
                        ToolError::Validation("browser profiles are not enabled".to_string())
                    }
                    other => ToolError::internal(format!("browser_profile: {other}")),
                })?;
        }
    }

    Ok(())
}
