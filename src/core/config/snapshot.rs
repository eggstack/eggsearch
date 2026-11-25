use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::error::{CoreError, CoreResult};

fn default_max_entries() -> usize {
    10_000
}

fn default_max_bytes() -> usize {
    134_217_728
}

fn default_retention_days() -> u64 {
    30
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
#[allow(missing_docs)]
pub struct FetchSnapshotSection {
    pub enabled: bool,
    pub path: Option<PathBuf>,
    pub max_entries: usize,
    pub max_bytes: usize,
    pub retention_days: u64,
}

impl Default for FetchSnapshotSection {
    fn default() -> Self {
        Self {
            enabled: false,
            path: None,
            max_entries: default_max_entries(),
            max_bytes: default_max_bytes(),
            retention_days: default_retention_days(),
        }
    }
}

impl FetchSnapshotSection {
    pub(crate) fn validate(&self) -> CoreResult<()> {
        if self.max_entries == 0 {
            return Err(CoreError::Config(
                "[fetch.snapshots].max_entries must be > 0".into(),
            ));
        }
        if self.max_bytes == 0 {
            return Err(CoreError::Config(
                "[fetch.snapshots].max_bytes must be > 0".into(),
            ));
        }
        if self.retention_days == 0 {
            return Err(CoreError::Config(
                "[fetch.snapshots].retention_days must be > 0".into(),
            ));
        }
        if self
            .path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err(CoreError::Config(
                "[fetch.snapshots].path must not be empty".into(),
            ));
        }
        Ok(())
    }
}
