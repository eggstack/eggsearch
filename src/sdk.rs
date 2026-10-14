#![allow(missing_docs)]

use std::str::FromStr;
use std::sync::Arc;

use crate::core::config::AppConfig;
use crate::mcp::state::ServerState;
use crate::mcp::tools::{
    run_batch_fetch, run_build_evidence_bundle, run_provider_status_async, run_repo_fetch,
    run_repo_map, run_repo_search, run_research_search, run_security_search, run_web_fetch,
    run_web_search, ToolError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolName {
    BatchFetch,
    BuildEvidenceBundle,
    ProviderStatus,
    RepoFetch,
    RepoMap,
    RepoSearch,
    ResearchSearch,
    SecuritySearch,
    WebFetch,
    WebSearch,
}

impl ToolName {
    pub const ALL: [Self; 10] = [
        Self::BatchFetch,
        Self::BuildEvidenceBundle,
        Self::ProviderStatus,
        Self::RepoFetch,
        Self::RepoMap,
        Self::RepoSearch,
        Self::ResearchSearch,
        Self::SecuritySearch,
        Self::WebFetch,
        Self::WebSearch,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BatchFetch => "batch_fetch",
            Self::BuildEvidenceBundle => "build_evidence_bundle",
            Self::ProviderStatus => "provider_status",
            Self::RepoFetch => "repo_fetch",
            Self::RepoMap => "repo_map",
            Self::RepoSearch => "repo_search",
            Self::ResearchSearch => "research_search",
            Self::SecuritySearch => "security_search",
            Self::WebFetch => "web_fetch",
            Self::WebSearch => "web_search",
        }
    }
}

impl FromStr for ToolName {
    type Err = SdkError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !crate::mcp::tool_contract::is_known_tool(value) {
            return Err(SdkError::new(
                "invalid_request",
                format!("unknown tool '{value}'"),
                None,
                None,
            ));
        }
        Self::ALL
            .into_iter()
            .find(|tool| tool.as_str() == value)
            .ok_or_else(|| {
                SdkError::new(
                    "internal",
                    "tool contract mapping is incomplete",
                    None,
                    None,
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SdkError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl SdkError {
    fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        repair: Option<serde_json::Value>,
        data: Option<serde_json::Value>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            repair,
            data,
        }
    }
}

impl From<ToolError> for SdkError {
    fn from(error: ToolError) -> Self {
        let payload = error.error_payload();
        Self {
            code: payload
                .get("code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("internal")
                .to_owned(),
            message: payload
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool execution failed")
                .to_owned(),
            repair: payload.get("repair").cloned(),
            data: payload.get("data").cloned(),
        }
    }
}

#[derive(Clone)]
pub struct EggsearchClient {
    state: Arc<ServerState>,
}

impl EggsearchClient {
    pub fn new(config: AppConfig) -> anyhow::Result<Self> {
        Ok(Self {
            state: Arc::new(ServerState::build(config)?),
        })
    }

    pub fn tool_names() -> impl ExactSizeIterator<Item = &'static str> {
        crate::mcp::tool_contract::ALL_CONTRACTS
            .iter()
            .map(|contract| contract.name)
    }

    pub fn tool_contracts(&self) -> serde_json::Value {
        let server = crate::mcp::server::EggsearchServer::new(self.state.clone());
        let tools = server
            .tool_definitions()
            .into_iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name.as_ref(),
                    "input_schema": tool.input_schema,
                    "output_schema": tool.output_schema,
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "fingerprint": server.tool_fingerprint(),
            "tools": tools,
        })
    }

    pub async fn execute(
        &self,
        tool: impl AsRef<str>,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, SdkError> {
        let tool = ToolName::from_str(tool.as_ref())?;
        if !arguments.is_object() {
            return Err(SdkError::new(
                "invalid_request",
                "tool arguments must be a JSON object",
                None,
                None,
            ));
        }
        let result = match tool {
            ToolName::BatchFetch => run_batch_fetch(self.state.clone(), decode(arguments)?).await,
            ToolName::BuildEvidenceBundle => run_build_evidence_bundle(decode(arguments)?),
            ToolName::ProviderStatus => {
                run_provider_status_async(self.state.clone(), decode(arguments)?).await
            }
            ToolName::RepoFetch => run_repo_fetch(self.state.clone(), decode(arguments)?).await,
            ToolName::RepoMap => run_repo_map(self.state.clone(), decode(arguments)?).await,
            ToolName::RepoSearch => run_repo_search(self.state.clone(), decode(arguments)?).await,
            ToolName::ResearchSearch => {
                run_research_search(self.state.clone(), decode(arguments)?).await
            }
            ToolName::SecuritySearch => {
                run_security_search(self.state.clone(), decode(arguments)?).await
            }
            ToolName::WebFetch => run_web_fetch(self.state.clone(), decode(arguments)?).await,
            ToolName::WebSearch => run_web_search(self.state.clone(), decode(arguments)?).await,
        };
        result.map_err(SdkError::from)
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T, SdkError> {
    serde_json::from_value(value).map_err(|_| {
        SdkError::new(
            "invalid_request",
            "tool arguments do not match the operation input shape",
            None,
            None,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Mode;
    use crate::mcp::tools::{
        run_batch_fetch, run_build_evidence_bundle, run_provider_status_async, run_repo_fetch,
        run_repo_map, run_repo_search, run_research_search, run_security_search, run_web_fetch,
        run_web_search,
    };
    use crate::meta::engines::error::EngineError;
    use crate::meta::engines::models::SearchResult;
    use crate::meta::engines::{BoxFuture, EngineSearchRequest, SearchEngine};
    use crate::meta::MetadataSearchAdapter;
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tempfile::TempDir;

    struct CountingEngine {
        calls: Arc<AtomicUsize>,
    }

    impl SearchEngine for CountingEngine {
        fn name(&self) -> &'static str {
            "sdk_counting_engine"
        }

        fn search<'a>(
            &'a self,
            _request: &'a EngineSearchRequest,
        ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    struct BlockingEngine {
        entered: Arc<AtomicUsize>,
        dropped: Arc<AtomicUsize>,
        entered_notify: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }

    struct BlockingGuard {
        dropped: Arc<AtomicUsize>,
    }

    impl Drop for BlockingGuard {
        fn drop(&mut self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl SearchEngine for BlockingEngine {
        fn name(&self) -> &'static str {
            "duckduckgo"
        }

        fn search<'a>(
            &'a self,
            _request: &'a EngineSearchRequest,
        ) -> BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
            let entered = self.entered.clone();
            let dropped = self.dropped.clone();
            let entered_notify = self.entered_notify.clone();
            let release = self.release.clone();
            Box::pin(async move {
                let _guard = BlockingGuard { dropped };
                entered.fetch_add(1, Ordering::SeqCst);
                entered_notify.notify_waiters();
                release.notified().await;
                Ok(Vec::new())
            })
        }
    }

    fn blocking_client() -> (
        EggsearchClient,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
        Arc<tokio::sync::Notify>,
        Arc<tokio::sync::Notify>,
    ) {
        let entered = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let entered_notify = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let mut config = AppConfig::default();
        config.search.mode = Mode::Live;
        config.local.enabled = false;
        let adapter = MetadataSearchAdapter::from_engines(
            vec![Arc::new(BlockingEngine {
                entered: entered.clone(),
                dropped: dropped.clone(),
                entered_notify: entered_notify.clone(),
                release: release.clone(),
            })],
            Duration::from_secs(2),
        );
        let state = ServerState::with_adapter(config, Arc::new(adapter));
        let client = EggsearchClient {
            state: Arc::new(state),
        };
        (client, entered, dropped, entered_notify, release)
    }

    async fn assert_parity<F>(
        client: &EggsearchClient,
        name: &str,
        args: serde_json::Value,
        direct: F,
    ) where
        F: Future<Output = Result<serde_json::Value, ToolError>>,
    {
        let sdk_result = client.execute(name, args).await;
        let direct_result = direct.await.map_err(SdkError::from);
        assert_eq!(sdk_result, direct_result, "parity for {name}");
    }

    #[test]
    fn names_are_exactly_the_canonical_tool_inventory() {
        let facade = ToolName::ALL.map(ToolName::as_str);
        let canonical: Vec<_> = EggsearchClient::tool_names().collect();
        assert_eq!(facade.len(), canonical.len());
        for name in facade {
            assert!(canonical.contains(&name));
            assert_eq!(ToolName::from_str(name).unwrap().as_str(), name);
        }
    }

    #[tokio::test]
    async fn unknown_name_and_non_object_arguments_are_request_errors() {
        assert_eq!(
            ToolName::from_str("local_search").unwrap_err().code,
            "invalid_request"
        );
        let client = EggsearchClient::new(AppConfig::default()).unwrap();
        let err = client
            .execute("web_search", serde_json::json!("not an object"))
            .await
            .unwrap_err();
        assert_eq!(err.code, "invalid_request");
    }

    #[tokio::test]
    async fn all_operations_match_their_existing_handler_results() {
        let mut config = AppConfig::default();
        config.search.mode = Mode::Off;
        config.local.enabled = false;
        let client = EggsearchClient::new(config).unwrap();
        let state = client.state.clone();

        let args = serde_json::json!({"query": "sdk parity"});
        assert_parity(
            &client,
            "web_search",
            args.clone(),
            run_web_search(state.clone(), decode(args).unwrap()),
        )
        .await;

        let args = serde_json::json!({"url": "file:///etc/passwd"});
        assert_parity(
            &client,
            "web_fetch",
            args.clone(),
            run_web_fetch(state.clone(), decode(args).unwrap()),
        )
        .await;

        let args = serde_json::json!({"items": []});
        assert_parity(
            &client,
            "batch_fetch",
            args.clone(),
            run_batch_fetch(state.clone(), decode(args).unwrap()),
        )
        .await;

        let args = serde_json::json!({});
        assert_parity(
            &client,
            "provider_status",
            args.clone(),
            run_provider_status_async(state.clone(), decode(args).unwrap()),
        )
        .await;

        let args = serde_json::json!({"query": "sdk parity"});
        assert_parity(
            &client,
            "repo_search",
            args.clone(),
            run_repo_search(state.clone(), decode(args).unwrap()),
        )
        .await;

        let args =
            serde_json::json!({"owner": "invalid owner", "repo": "bad", "path": "README.md"});
        assert_parity(
            &client,
            "repo_fetch",
            args.clone(),
            run_repo_fetch(state.clone(), decode(args).unwrap()),
        )
        .await;

        let args = serde_json::json!({"owner": "invalid owner", "repo": "bad"});
        assert_parity(
            &client,
            "repo_map",
            args.clone(),
            run_repo_map(state.clone(), decode(args).unwrap()),
        )
        .await;

        let args = serde_json::json!({"query": "CVE-2024-1234"});
        assert_parity(
            &client,
            "security_search",
            args.clone(),
            run_security_search(state.clone(), decode(args).unwrap()),
        )
        .await;

        let args = serde_json::json!({"query": "sdk parity"});
        assert_parity(
            &client,
            "research_search",
            args.clone(),
            run_research_search(state, decode(args).unwrap()),
        )
        .await;

        let args = serde_json::json!({});
        assert_parity(
            &client,
            "build_evidence_bundle",
            args.clone(),
            std::future::ready(run_build_evidence_bundle(decode(args).unwrap())),
        )
        .await;
    }

    #[tokio::test]
    async fn dropping_sdk_execute_aborts_blocked_engine() {
        let (client, entered, dropped, entered_notify, _release) = blocking_client();
        let handle = tokio::spawn(async move {
            client
                .execute(
                    "web_search",
                    serde_json::json!({"query": "sdk cancellation"}),
                )
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), entered_notify.notified())
            .await
            .unwrap();
        assert_eq!(entered.load(Ordering::SeqCst), 1);
        handle.abort();
        let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        tokio::time::timeout(Duration::from_secs(5), async {
            while dropped.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn close_token_select_drops_blocked_engine() {
        let (client, entered, dropped, entered_notify, _release) = blocking_client();
        let token = tokio_util::sync::CancellationToken::new();
        let canceller = token.clone();
        let runner = tokio::spawn(async move {
            tokio::select! {
                _ = canceller.cancelled() => Err(SdkError::new(
                    "client_closed",
                    "Eggsearch client closed while the operation was running",
                    None,
                    None,
                )),
                result = client.execute(
                    "web_search",
                    serde_json::json!({"query": "sdk close race"}),
                ) => result,
            }
        });
        tokio::time::timeout(Duration::from_secs(5), entered_notify.notified())
            .await
            .unwrap();
        assert_eq!(entered.load(Ordering::SeqCst), 1);
        token.cancel();
        let outcome = tokio::time::timeout(Duration::from_secs(5), runner)
            .await
            .unwrap()
            .unwrap();
        let error = outcome.unwrap_err();
        assert_eq!(error.code, "client_closed");
        tokio::time::timeout(Duration::from_secs(5), async {
            while dropped.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn mode_off_repo_search_through_sdk_keeps_remote_dispatch_at_zero() {
        let root = TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::write(
            root.path().join("src/lib.rs"),
            "pub fn sdk_locality_fixture() -> i32 { 42 }",
        )
        .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let mut config = AppConfig::default();
        config.search.mode = Mode::Off;
        config
            .search
            .providers
            .insert("sdk_counting_engine".into(), true);
        config.local.enabled = true;
        config.local.roots = vec![root.path().to_path_buf()];
        config.local.respect_gitignore = false;
        let adapter = MetadataSearchAdapter::from_engines(
            vec![Arc::new(CountingEngine {
                calls: calls.clone(),
            })],
            Duration::from_secs(2),
        );
        let mut state = ServerState::with_adapter(config.clone(), Arc::new(adapter));
        state.local_backend = Some(Arc::new(
            crate::meta::local_backend::LocalWorkspaceBackend::new(config.local).unwrap(),
        ));
        let client = EggsearchClient {
            state: Arc::new(state),
        };

        let result = client
            .execute(
                "repo_search",
                serde_json::json!({
                    "query": "sdk_locality_fixture",
                    "providers": ["sdk_counting_engine"],
                    "include_local": true
                }),
            )
            .await
            .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            result["providers_queried"],
            serde_json::json!(["local_workspace"])
        );
        assert!(result["groups"].as_array().is_some_and(|groups| {
            groups.iter().any(|group| {
                group["results"].as_array().is_some_and(|results| {
                    results.iter().any(|entry| {
                        entry["snippet"]
                            .as_str()
                            .is_some_and(|text| text.contains("sdk_locality_fixture"))
                    })
                })
            })
        }));
    }
}
