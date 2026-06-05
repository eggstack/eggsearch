//! Configuration model and loader for eggsearch.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Off,
    LocalOnly,
    Live,
    Ask,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Ask
    }
}

impl Mode {
    pub fn from_str(s: &str) -> CoreResult<Self> {
        match s.to_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "local_only" | "localonly" | "local" => Ok(Self::LocalOnly),
            "live" => Ok(Self::Live),
            "ask" => Ok(Self::Ask),
            other => Err(CoreError::Config(format!("unknown mode: {other}"))),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub extra: std::collections::BTreeMap<String, String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: None,
            api_key_env: None,
            extra: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveConfig {
    pub enabled: bool,
    pub max_concurrency: usize,
    pub timeout_ms: u64,
    pub user_agent: String,
    pub respect_robots_txt: bool,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrency: 4,
            timeout_ms: 8000,
            user_agent: format!("eggsearch/{}", env!("CARGO_PKG_VERSION")),
            respect_robots_txt: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalConfig {
    pub enabled: bool,
    pub backend: String,
    pub index_dir: PathBuf,
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: "tantivy".to_string(),
            index_dir: default_index_dir(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchSection {
    pub mode: Mode,
    pub max_results: usize,
    pub cache_dir: PathBuf,
    pub artifact_dir: PathBuf,
    pub live: LiveConfig,
    pub local: LocalConfig,
    #[serde(default)]
    pub providers: std::collections::BTreeMap<String, ProviderConfig>,
}

impl Default for SearchSection {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            max_results: 8,
            cache_dir: default_cache_dir(),
            artifact_dir: default_artifact_dir(),
            live: LiveConfig::default(),
            local: LocalConfig::default(),
            providers: default_providers(),
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

    /// Returns true if any provider is enabled.
    pub fn any_provider_enabled(&self) -> bool {
        self.search.providers.values().any(|p| p.enabled)
    }
}

pub fn default_config_path() -> PathBuf {
    if let Some(dir) = dirs::config_dir() {
        return dir.join("eggsearch").join("config.toml");
    }
    PathBuf::from("eggsearch.toml")
}

pub fn default_cache_dir() -> PathBuf {
    if let Some(dir) = dirs::data_local_dir() {
        return dir.join("eggsearch").join("cache");
    }
    PathBuf::from(".eggsearch/cache")
}

pub fn default_artifact_dir() -> PathBuf {
    if let Some(dir) = dirs::data_local_dir() {
        return dir.join("eggsearch").join("artifacts");
    }
    PathBuf::from(".eggsearch/artifacts")
}

pub fn default_index_dir() -> PathBuf {
    if let Some(dir) = dirs::data_local_dir() {
        return dir.join("eggsearch").join("index");
    }
    PathBuf::from(".eggsearch/index")
}

pub fn default_providers() -> std::collections::BTreeMap<String, ProviderConfig> {
    let mut m = std::collections::BTreeMap::new();
    m.insert(
        "duckduckgo_html".into(),
        ProviderConfig {
            enabled: true,
            ..Default::default()
        },
    );
    m.insert(
        "wikipedia".into(),
        ProviderConfig {
            enabled: true,
            ..Default::default()
        },
    );
    m.insert(
        "crates_io".into(),
        ProviderConfig {
            enabled: true,
            ..Default::default()
        },
    );
    m.insert(
        "docs_rs".into(),
        ProviderConfig {
            enabled: true,
            ..Default::default()
        },
    );
    m.insert(
        "searxng".into(),
        ProviderConfig {
            enabled: false,
            ..Default::default()
        },
    );
    m.insert(
        "brave".into(),
        ProviderConfig {
            enabled: false,
            ..Default::default()
        },
    );
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_parsing() {
        assert_eq!(Mode::from_str("off").unwrap(), Mode::Off);
        assert_eq!(Mode::from_str("live").unwrap(), Mode::Live);
        assert_eq!(Mode::from_str("local_only").unwrap(), Mode::LocalOnly);
        assert_eq!(Mode::from_str("ask").unwrap(), Mode::Ask);
        assert!(Mode::from_str("nope").is_err());
    }

    #[test]
    fn default_config_loads() {
        let c = AppConfig::default();
        assert!(c.search.max_results > 0);
        assert!(c.any_provider_enabled());
    }

    #[test]
    fn round_trip_toml() {
        let c = AppConfig::default();
        let text = toml::to_string(&c).unwrap();
        let parsed: AppConfig = toml::from_str(&text).unwrap();
        assert_eq!(parsed.search.max_results, c.search.max_results);
    }
}
