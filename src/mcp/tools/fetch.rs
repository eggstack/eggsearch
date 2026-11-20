use super::batch_fetch::{
    run_batch_fetch_with_item_windows_and_progress, BatchFetchArgs, ItemDocumentWindow,
};
use super::common::{ToolError, ToolOutcome};
use crate::core::batch_fetch::BatchFetchItem;
use crate::mcp::progress::ProgressSink;
use crate::mcp::state::ServerState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

fn is_none_or_false(value: &Option<bool>) -> bool {
    !value.unwrap_or(false)
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum FetchItem {
    Web(FetchWebItem),
    Repo(FetchRepoItem),
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct FetchWebItem {
    #[schemars(description = "The URL to fetch. Must be a valid HTTP(S) URL.")]
    url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Extraction mode: text, markdown, or metadata_only.")]
    extract_mode: Option<crate::core::fetch::ExtractMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Whether to include extracted links.")]
    include_links: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum characters to extract for this item.")]
    max_chars: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Cache policy: default, bypass, or refresh.")]
    cache_policy: Option<crate::core::fetch::FetchCachePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Caller maximum acceptable cache age in seconds.")]
    max_cache_age_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Optional focus query. Does not follow links.")]
    focus: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum focused chunks to return.")]
    focus_max_chunks: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum focused characters to return.")]
    focus_max_chars: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "PDF-specific fetch options.")]
    pdf: Option<crate::core::fetch::PdfFetchOptions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "Option<crate::core::fetch::RenderPolicy>",
        description = "Render policy: http_only, auto, or browser."
    )]
    render: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Named browser profile for persistent session reuse.")]
    browser_profile: Option<String>,
    #[serde(default, skip_serializing_if = "is_none_or_false")]
    #[schemars(description = "Include a bounded viewport screenshot as MCP image content.")]
    include_screenshot: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Zero-based character offset into this web document.")]
    offset: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum characters from this document offset.")]
    limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct FetchRepoItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Code host: github, gitlab, codeberg, gitea, forgejo, or workspace.")]
    host: Option<String>,
    #[schemars(description = "Repository owner or workspace root name.")]
    owner: String,
    #[schemars(description = "Repository name.")]
    repo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Branch, tag, or commit ref.")]
    ref_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Full commit SHA for stable permalink construction.")]
    commit_sha: Option<String>,
    #[schemars(description = "File path relative to the repository or workspace root.")]
    path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "First line to return, 1-indexed and inclusive.")]
    line_start: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Last line to return, 1-indexed and inclusive.")]
    line_end: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Extra context lines before the selected range.")]
    context_before: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Extra context lines after the selected range.")]
    context_after: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum characters to return for this item.")]
    max_chars: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Optional focus query over the bounded repository result.")]
    focus: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum focused chunks to return.")]
    focus_max_chunks: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum focused characters to return.")]
    focus_max_chars: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Symbol name to select.")]
    symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Symbol kind filter applied with symbol.")]
    symbol_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Select the span containing this text.")]
    match_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Expand the selected line or span to its enclosing block.")]
    expand_to_block: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Maximum lines returned when expanding to a block.")]
    max_block_lines: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Prefer the local workspace copy when one exists.")]
    prefer_local: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub(crate) struct NonEmptyFetchItems(#[schemars(length(min = 1))] Vec<FetchItem>);

#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub(crate) enum FetchItems {
    One(Box<FetchItem>),
    Many(NonEmptyFetchItems),
}

impl<'de> Deserialize<'de> for FetchItems {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            One(Box<FetchItem>),
            Many(NonEmptyFetchItems),
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        match &value {
            serde_json::Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    validate_fetch_item_shape(item, Some(index))
                        .map_err(serde::de::Error::custom)?;
                }
            }
            item => validate_fetch_item_shape(item, None).map_err(serde::de::Error::custom)?,
        }
        serde_json::from_value::<Repr>(value)
            .map(|repr| match repr {
                Repr::One(item) => FetchItems::One(item),
                Repr::Many(items) => FetchItems::Many(items),
            })
            .map_err(|_| {
                serde::de::Error::custom(format!(
                    "fetch item does not match an accepted shape; {ACCEPTED_FETCH_ITEM_SHAPES}"
                ))
            })
    }
}

const ACCEPTED_FETCH_ITEM_SHAPES: &str = r#"accepted shapes: {"type":"web","url":"<url>"} and {"type":"repo","owner":"<owner>","repo":"<repo>","path":"<path>"}"#;

fn validate_fetch_item_shape(
    value: &serde_json::Value,
    index: Option<usize>,
) -> Result<(), String> {
    let prefix = index
        .map(|index| format!("items[{index}]: "))
        .unwrap_or_default();
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    match object.get("type").and_then(|kind| kind.as_str()) {
        Some("repo") => {
            for field in ["owner", "repo", "path"] {
                let present = object
                    .get(field)
                    .and_then(|field| field.as_str())
                    .is_some_and(|text| !text.trim().is_empty());
                if !present {
                    return Err(format!(
                        r#"{prefix}repo item is missing the required `{field}` field; accepted shape: {{"type":"repo","owner":"<owner>","repo":"<repo>","path":"<path>"}} (optional: host, ref_name, commit_sha, line_start, line_end, context_before, context_after, max_chars, focus, focus_max_chunks, focus_max_chars, symbol, symbol_kind, match_text, expand_to_block, max_block_lines, prefer_local)"#
                    ));
                }
            }
        }
        Some("web") => {
            let present = object
                .get("url")
                .and_then(|url| url.as_str())
                .is_some_and(|text| !text.trim().is_empty());
            if !present {
                return Err(format!(
                    r#"{prefix}web item is missing the required `url` field; accepted shape: {{"type":"web","url":"https://example.com"}}"#
                ));
            }
        }
        _ if !object.contains_key("type") => {
            return Err(format!(
                "{prefix}fetch item is missing the required `type` tag; {ACCEPTED_FETCH_ITEM_SHAPES}"
            ));
        }
        _ => {
            return Err(format!(
                "{prefix}fetch item has an unknown `type` tag; accepted values are `web` and `repo`; {ACCEPTED_FETCH_ITEM_SHAPES}"
            ));
        }
    }
    Ok(())
}

impl FetchItems {
    fn into_vec(self) -> Vec<FetchItem> {
        match self {
            Self::One(item) => vec![*item],
            Self::Many(items) => items.0,
        }
    }
}

impl FetchItem {
    fn into_batch_parts(self) -> (BatchFetchItem, Option<ItemDocumentWindow>) {
        match self {
            Self::Web(item) => {
                let window =
                    (item.offset.is_some() || item.limit.is_some()).then_some(ItemDocumentWindow {
                        offset: item.offset,
                        limit: item.limit,
                    });
                (
                    BatchFetchItem::Web {
                        url: item.url,
                        extract_mode: item.extract_mode,
                        include_links: item.include_links,
                        max_chars: item.max_chars,
                        cache_policy: item.cache_policy,
                        max_cache_age_seconds: item.max_cache_age_seconds,
                        focus: item.focus,
                        focus_max_chunks: item.focus_max_chunks,
                        focus_max_chars: item.focus_max_chars,
                        pdf: item.pdf,
                        render: item.render,
                        browser_profile: item.browser_profile,
                        include_screenshot: item.include_screenshot,
                    },
                    window,
                )
            }
            Self::Repo(item) => (
                BatchFetchItem::Repo {
                    host: item.host,
                    owner: item.owner,
                    repo: item.repo,
                    ref_name: item.ref_name,
                    commit_sha: item.commit_sha,
                    path: item.path,
                    line_start: item.line_start,
                    line_end: item.line_end,
                    context_before: item.context_before,
                    context_after: item.context_after,
                    max_chars: item.max_chars,
                    focus: item.focus,
                    focus_max_chunks: item.focus_max_chunks,
                    focus_max_chars: item.focus_max_chars,
                    symbol: item.symbol,
                    symbol_kind: item.symbol_kind,
                    match_text: item.match_text,
                    expand_to_block: item.expand_to_block,
                    max_block_lines: item.max_block_lines,
                    prefer_local: item.prefer_local,
                },
                None,
            ),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub(crate) struct FetchArgs {
    pub items: FetchItems,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chars_per_item: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_chars: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continue_on_error: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_detail: Option<crate::mcp::projection::ResponseDetail>,
}

impl FetchArgs {
    pub(crate) fn into_batch_parts(self) -> (BatchFetchArgs, Vec<Option<ItemDocumentWindow>>) {
        let mut items = Vec::new();
        let mut windows = Vec::new();
        for item in self.items.into_vec() {
            let (item, window) = item.into_batch_parts();
            items.push(item);
            windows.push(window);
        }
        (
            BatchFetchArgs {
                items,
                max_items: self.max_items,
                max_chars_per_item: self.max_chars_per_item,
                max_total_chars: self.max_total_chars,
                timeout_ms: self.timeout_ms,
                continue_on_error: self.continue_on_error,
                offset: self.offset,
                limit: self.limit,
                response_detail: self.response_detail,
            },
            windows,
        )
    }
}

pub(crate) async fn run_fetch_with_host_scope_and_progress(
    state: Arc<ServerState>,
    args: FetchArgs,
    host_local_scope: Option<serde_json::Value>,
    progress: &mut ProgressSink,
) -> Result<ToolOutcome, ToolError> {
    let (batch_args, windows) = args.into_batch_parts();
    run_batch_fetch_with_item_windows_and_progress(
        state,
        batch_args,
        windows,
        host_local_scope,
        progress,
    )
    .await
}

pub(crate) async fn run_fetch(
    state: Arc<ServerState>,
    args: FetchArgs,
) -> Result<serde_json::Value, ToolError> {
    let mut progress = ProgressSink::disabled();
    run_fetch_with_host_scope_and_progress(state, args, None, &mut progress)
        .await
        .map(|outcome| outcome.value)
}

#[cfg(test)]
mod tests {
    use super::{run_fetch, FetchArgs, FetchItem, FetchItems};
    use crate::core::batch_fetch::BatchFetchItem;
    use crate::core::config::AppConfig;
    use crate::mcp::state::ServerState;
    use std::sync::Arc;

    #[test]
    fn fetch_items_accept_scalar_and_ordered_mixed_array() {
        let scalar: FetchArgs = serde_json::from_value(serde_json::json!({
            "items": {"type": "web", "url": "https://example.com", "offset": 12, "limit": 34}
        }))
        .expect("single item deserializes");
        let scalar_for_windows = scalar.clone();
        let scalar_items = scalar.items.into_vec();
        assert_eq!(scalar_items.len(), 1);
        match &scalar_items[0] {
            FetchItem::Web(item) => {
                assert_eq!(item.offset, Some(12));
                assert_eq!(item.limit, Some(34));
            }
            FetchItem::Repo(_) => panic!("single item must remain a web target"),
        }

        let (_, scalar_windows) = scalar_for_windows.into_batch_parts();
        let scalar_window = scalar_windows[0].expect("web item window");
        assert_eq!(scalar_window.offset, Some(12));
        assert_eq!(scalar_window.limit, Some(34));

        let array: FetchArgs = serde_json::from_value(serde_json::json!({
            "items": [
                {"type": "web", "url": "https://example.com"},
                {"type": "repo", "owner": "owner", "repo": "repo", "path": "src/lib.rs"}
            ]
        }))
        .expect("mixed item array deserializes");
        let (batch, windows) = array.into_batch_parts();
        assert_eq!(batch.items.len(), 2);
        assert!(matches!(&batch.items[0], BatchFetchItem::Web { .. }));
        assert!(matches!(&batch.items[1], BatchFetchItem::Repo { .. }));
        assert!(windows.iter().all(|window| window.is_none()));
    }

    #[test]
    fn fetch_rejects_unknown_or_malformed_targets_during_deserialization() {
        for value in [
            serde_json::json!({"items": {"type": "filesystem", "path": "/tmp/file"}}),
            serde_json::json!({"items": {"type": "web"}}),
            serde_json::json!({"items": {"type": "repo", "owner": "owner", "repo": "repo"}}),
            serde_json::json!({
                "items": {"type": "web", "url": "https://example.com", "max_chars": "many"}
            }),
        ] {
            let error = serde_json::from_value::<FetchArgs>(value)
                .expect_err("malformed fetch target must fail before dispatch");
            assert!(!error.to_string().contains("Repr"), "{error}");
        }
    }

    #[test]
    fn fetch_item_tag_errors_name_accepted_shapes_for_scalar_and_array() {
        let cases = [
            (
                serde_json::json!({"url": "https://example.com/"}),
                "missing the required `type` tag",
            ),
            (
                serde_json::json!({"type": "filesystem"}),
                "unknown `type` tag",
            ),
        ];
        let valid_web = serde_json::json!({"type": "web", "url": "https://example.com/"});
        for (item, expected) in cases {
            let scalar_error = serde_json::from_value::<FetchArgs>(serde_json::json!({
                "items": item.clone()
            }))
            .expect_err("invalid item tag must fail");
            let scalar_message = scalar_error.to_string();
            assert!(scalar_message.contains(expected), "{scalar_message}");
            assert!(
                scalar_message.contains(r#"{"type":"web","url":"<url>"}"#),
                "{scalar_message}"
            );
            assert!(
                scalar_message.contains(
                    r#"{"type":"repo","owner":"<owner>","repo":"<repo>","path":"<path>"}"#
                ),
                "{scalar_message}"
            );
            assert!(!scalar_message.contains("Repr"), "{scalar_message}");
            if expected.contains("unknown") {
                assert!(
                    scalar_message.contains("accepted values are `web` and `repo`"),
                    "{scalar_message}"
                );
            }

            let array_error = serde_json::from_value::<FetchArgs>(serde_json::json!({
                "items": [valid_web.clone(), item]
            }))
            .expect_err("invalid array item tag must fail");
            let array_message = array_error.to_string();
            assert!(array_message.contains("items[1]"), "{array_message}");
            assert!(array_message.contains(expected), "{array_message}");
            assert!(!array_message.contains("Repr"), "{array_message}");
        }
    }

    #[test]
    fn fetch_locator_errors_name_the_missing_field() {
        let error = serde_json::from_value::<FetchArgs>(serde_json::json!({
            "items": [{"type": "repo", "owner": "owner", "path": "src/lib.rs"}]
        }))
        .expect_err("repo item without repo must fail");
        let message = error.to_string();
        assert_eq!(
            message,
            r#"items[0]: repo item is missing the required `repo` field; accepted shape: {"type":"repo","owner":"<owner>","repo":"<repo>","path":"<path>"} (optional: host, ref_name, commit_sha, line_start, line_end, context_before, context_after, max_chars, focus, focus_max_chunks, focus_max_chars, symbol, symbol_kind, match_text, expand_to_block, max_block_lines, prefer_local)"#
        );

        let error = serde_json::from_value::<FetchArgs>(serde_json::json!({
            "items": {"type": "web"}
        }))
        .expect_err("web item without url must fail");
        assert_eq!(
            error.to_string(),
            r#"web item is missing the required `url` field; accepted shape: {"type":"web","url":"https://example.com"}"#
        );
    }

    #[tokio::test]
    async fn fetch_rejects_empty_arrays_before_io() {
        let args: FetchArgs = serde_json::from_value(serde_json::json!({"items": []}))
            .expect("empty list has valid JSON shape");
        assert!(matches!(args.items, FetchItems::Many(_)));
        let state = Arc::new(ServerState::build(AppConfig::default()).expect("state builds"));
        let error = run_fetch(state, args)
            .await
            .expect_err("empty list rejected");
        assert!(
            error.to_string().contains("items must not be empty"),
            "{error}"
        );
    }
}
