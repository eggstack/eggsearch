//! Configuration model and loader for eggsearch.
//!
//! The changeover configuration is intentionally minimal: the default
//! server is a live metasearch-only MCP server. Tantivy and web_fetch
//! are deferred behind feature flags.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// All tools disabled.
    Off,
    /// Live metasearch is allowed.
    Live,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Live
    }
}

impl Mode {
    pub fn from_str(s: &str) -> CoreResult<Self> {
        match s.to_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "live" | "ask" | "local_only" | "localonly" | "local" => Ok(Self::Live),
            other => Err(CoreError::Config(format!("unknown mode: {other}"))),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveConfig {
    pub user_agent: String,
    pub respect_robots_txt: bool,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            user_agent: format!("eggsearch/{}", env!("CARGO_PKG_VERSION")),
            respect_robots_txt: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchSection {
    /// Server mode: `off` or `live`. Defaults to `live`.
    pub mode: Mode,
    /// Default `max_results` for `web_search` when not specified.
    pub max_results: usize,
    /// Hard cap on `max_results` from clients.
    pub max_results_cap: usize,
    /// Maximum accepted query length in characters.
    pub max_query_chars: usize,
    /// Default per-request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Default providers to query when none are specified.
    pub default_providers: Vec<String>,
    /// Per-provider enable/disable flags. Keys are provider ids
    /// (`duckduckgo`, `brave`, `startpage`, `yahoo`).
    #[serde(default)]
    pub providers: std::collections::BTreeMap<String, bool>,
    /// Live network configuration.
    pub live: LiveConfig,
}

impl Default for SearchSection {
    fn default() -> Self {
        let mut providers = std::collections::BTreeMap::new();
        providers.insert("duckduckgo".to_string(), true);
        providers.insert("brave".to_string(), true);
        providers.insert("startpage".to_string(), true);
        providers.insert("yahoo".to_string(), true);
        Self {
            mode: Mode::default(),
            max_results: 10,
            max_results_cap: 50,
            max_query_chars: 512,
            timeout_ms: 8000,
            default_providers: vec![
                "duckduckgo".to_string(),
                "startpage".to_string(),
                "yahoo".to_string(),
            ],
            providers,
            live: LiveConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub search: SearchSection,
}

impl AppConfig {
    /// Load a config from the given TOML file path, falling back to defaults
    /// for any missing sections.
    pub fn load(path: &Path) -> CoreResult<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&text)?;
        Ok(cfg)
    }

    /// Save the config to the given path. Creates parent dirs as needed.
    pub fn save(&self, path: &Path) -> CoreResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| CoreError::Other(e.to_string()))?;
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Resolve the effective provider list for a request, given the
    /// client-supplied override (or empty for "use defaults"). The
    /// returned list is de-duplicated while preserving input order.
    pub fn resolve_providers(&self, override_list: &[String]) -> Vec<String> {
        if override_list.is_empty() {
            return self.search.default_providers.clone();
        }
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for p in override_list {
            if seen.insert(p.clone()) {
                out.push(p.clone());
            }
        }
        out
    }

    /// True if the given provider id is enabled in the config (or if the
    /// provider is unknown to the config, default to enabled).
    pub fn provider_enabled(&self, id: &str) -> bool {
        self.search.providers.get(id).copied().unwrap_or(true)
    }
}

pub fn default_config_path() -> PathBuf {
    if let Some(dir) = dirs::config_dir() {
        return dir.join("eggsearch").join("config.toml");
    }
    PathBuf::from("eggsearch.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_parsing() {
        assert_eq!(Mode::from_str("off").unwrap(), Mode::Off);
        assert_eq!(Mode::from_str("live").unwrap(), Mode::Live);
        assert!(Mode::from_str("nope").is_err());
    }

    #[test]
    fn default_config_loads() {
        let c = AppConfig::default();
        assert!(c.search.max_results > 0);
        assert!(!c.search.default_providers.is_empty());
    }

    #[test]
    fn default_providers_lists_known_engines() {
        let c = AppConfig::default();
        for expected in ["duckduckgo", "brave", "startpage", "yahoo"] {
            assert!(
                c.search.providers.contains_key(expected),
                "missing default provider: {expected}"
            );
        }
    }

    #[test]
    fn round_trip_toml() {
        let c = AppConfig::default();
        let text = toml::to_string(&c).unwrap();
        let parsed: AppConfig = toml::from_str(&text).unwrap();
        assert_eq!(parsed.search.max_results, c.search.max_results);
    }

    #[test]
    fn resolve_providers_uses_default_when_empty() {
        let c = AppConfig::default();
        let out = c.resolve_providers(&[]);
        assert_eq!(out, c.search.default_providers);
    }

    #[test]
    fn resolve_providers_dedupes_override() {
        let c = AppConfig::default();
        let out = c.resolve_providers(&["brave".into(), "brave".into(), "duckduckgo".into()]);
        assert_eq!(out, vec!["brave".to_string(), "duckduckgo".to_string()]);
    }
}
