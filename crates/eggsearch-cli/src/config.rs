//! CLI configuration loader.

use std::path::Path;

use eggsearch_core::config::AppConfig;

pub fn load(path: Option<&Path>) -> anyhow::Result<AppConfig> {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => eggsearch_core::config::default_config_path(),
    };
    let cfg = AppConfig::load(&path)?;
    Ok(cfg)
}
