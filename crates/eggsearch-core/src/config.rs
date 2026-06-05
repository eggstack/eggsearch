//! Configuration model and loader for eggsearch.
//!
//! The changeover configuration is intentionally minimal: the default
//! server is a live metasearch-only MCP server. Tantivy and web_fetch
//! are deferred behind feature flags.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// All tools disabled.
    Off,
    /// Live metasearch is allowed.
    #[default]
    Live,
}

impl Mode {
    /// Parse a mode string. Only `"off"` and `"live"` are accepted; `"ask"`
    /// is a host-level policy and is not a valid value at this layer.
    pub fn parse(s: &str) -> CoreResult<Self> {
        match s.to_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "live" => Ok(Self::Live),
            other => Err(CoreError::Config(format!("unknown mode: {other}"))),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LiveConfig {
    /// Reserved for future use. The current build does not allow the
    /// operator to override the upstream HTTP user-agent (the upstream
    /// crate hard-codes a browser-like agent that upstream providers
    /// expect).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Reserved for future use. The current build does not fetch URLs,
    /// so there is nothing to apply a robots policy to. `web_fetch`,
    /// when added, will enforce this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub respect_robots_txt: Option<bool>,
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
    /// Live network configuration. Most fields are reserved for future
    /// use; see `LiveConfig` docs.
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
        let text = toml::to_string_pretty(self).map_err(|e| CoreError::TomlSer(e.to_string()))?;
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
        assert_eq!(Mode::parse("off").unwrap(), Mode::Off);
        assert_eq!(Mode::parse("live").unwrap(), Mode::Live);
        assert!(Mode::parse("nope").is_err());
    }

    #[test]
    fn mode_parsing_rejects_documented_aliases() {
        // The previous build accepted "ask", "local_only", "localonly",
        // and "local" as aliases for Live. The current build is strict
        // and only accepts "off" and "live".
        for alias in ["ask", "local_only", "localonly", "local"] {
            assert!(
                Mode::parse(alias).is_err(),
                "{alias} should be rejected, was accepted as a Live alias"
            );
        }
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

    #[test]
    fn save_load_round_trip_through_filesystem() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let c = AppConfig::default();
        c.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(loaded.search.max_results, c.search.max_results);
        assert_eq!(loaded.search.mode, c.search.mode);
        assert_eq!(
            loaded.search.default_providers,
            c.search.default_providers
        );
    }

    #[test]
    fn load_malformed_toml_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "this is not [valid toml").unwrap();
        let err = AppConfig::load(&path);
        assert!(err.is_err(), "expected error for malformed TOML");
    }

    #[test]
    fn load_missing_file_returns_default() {
        let path = std::path::Path::new("/nonexistent/path/config.toml");
        let cfg = AppConfig::load(path).unwrap();
        assert_eq!(cfg.search.mode, Mode::default());
    }
}
