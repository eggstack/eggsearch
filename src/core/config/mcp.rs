//! MCP advertisement and response-shaping configuration.

use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum McpSchemaProfile {
    #[default]
    Full,
    Compact,
}

impl McpSchemaProfile {
    /// Stable machine-readable label used in diagnostics and instructions.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Compact => "compact",
        }
    }
}

/// Which subset of the ten canonical tools `tools/list` advertises.
///
/// A preset filters advertisement only: every tool stays registered and
/// callable. Unknown values fail configuration parsing.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum McpToolPreset {
    #[default]
    Full,
    Research,
    Code,
    Minimal,
}

impl McpToolPreset {
    /// Stable machine-readable label used in diagnostics and instructions.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Research => "research",
            Self::Code => "code",
            Self::Minimal => "minimal",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct McpSection {
    #[serde(default)]
    pub schema_profile: McpSchemaProfile,
    #[serde(default)]
    pub tool_preset: McpToolPreset,
    #[serde(default = "default_brief_max_chars")]
    pub brief_max_chars: usize,
    #[serde(default = "default_excerpt_budget_chars")]
    pub excerpt_budget_chars: usize,
    #[serde(default)]
    pub tasks: McpTasksSection,
}

/// Opt-in process-local MCP Tasks extension settings (`[mcp.tasks]`).
///
/// Disabled by default. Nothing is written to disk; task state lives only
/// in the running process and is bounded by `ttl_ms`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct McpTasksSection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_task_ttl_ms")]
    pub ttl_ms: u64,
    #[serde(default = "default_task_max_concurrent")]
    pub max_concurrent: usize,
    #[serde(default = "default_task_poll_interval_ms")]
    pub poll_interval_ms: u64,
}

/// Default task retention for `[mcp.tasks].ttl_ms`.
pub const DEFAULT_TASK_TTL_MS: u64 = 300_000;

/// Lower bound accepted for `[mcp.tasks].ttl_ms`.
pub const MIN_TASK_TTL_MS: u64 = 1_000;

/// Upper bound accepted for `[mcp.tasks].ttl_ms`.
pub const MAX_TASK_TTL_MS: u64 = 3_600_000;

/// Default concurrent task cap for `[mcp.tasks].max_concurrent`.
pub const DEFAULT_TASK_MAX_CONCURRENT: usize = 4;

/// Upper bound accepted for `[mcp.tasks].max_concurrent`.
pub const MAX_TASK_CONCURRENT: usize = 16;

/// Default suggested poll interval for `[mcp.tasks].poll_interval_ms`.
pub const DEFAULT_TASK_POLL_INTERVAL_MS: u64 = 1_000;

/// Lower bound accepted for `[mcp.tasks].poll_interval_ms`.
pub const MIN_TASK_POLL_INTERVAL_MS: u64 = 250;

/// Upper bound accepted for `[mcp.tasks].poll_interval_ms`.
pub const MAX_TASK_POLL_INTERVAL_MS: u64 = 10_000;

fn default_task_ttl_ms() -> u64 {
    DEFAULT_TASK_TTL_MS
}

fn default_task_max_concurrent() -> usize {
    DEFAULT_TASK_MAX_CONCURRENT
}

fn default_task_poll_interval_ms() -> u64 {
    DEFAULT_TASK_POLL_INTERVAL_MS
}

impl Default for McpTasksSection {
    fn default() -> Self {
        Self {
            enabled: false,
            ttl_ms: default_task_ttl_ms(),
            max_concurrent: default_task_max_concurrent(),
            poll_interval_ms: default_task_poll_interval_ms(),
        }
    }
}

impl McpTasksSection {
    /// Validate configured bounds. Returns the operator-facing message.
    pub fn validate(&self) -> Result<(), String> {
        if !(MIN_TASK_TTL_MS..=MAX_TASK_TTL_MS).contains(&self.ttl_ms) {
            return Err(format!(
                "[mcp.tasks].ttl_ms must be between {MIN_TASK_TTL_MS} and {MAX_TASK_TTL_MS}"
            ));
        }
        if !(1..=MAX_TASK_CONCURRENT).contains(&self.max_concurrent) {
            return Err(format!(
                "[mcp.tasks].max_concurrent must be between 1 and {MAX_TASK_CONCURRENT}"
            ));
        }
        if !(MIN_TASK_POLL_INTERVAL_MS..=MAX_TASK_POLL_INTERVAL_MS).contains(&self.poll_interval_ms)
        {
            return Err(format!(
                "[mcp.tasks].poll_interval_ms must be between {MIN_TASK_POLL_INTERVAL_MS} and {MAX_TASK_POLL_INTERVAL_MS}"
            ));
        }
        Ok(())
    }
}

/// Default response-wide excerpt ceiling for reduced MCP search responses.
pub const DEFAULT_EXCERPT_BUDGET_CHARS: usize = 4_000;

/// Lower bound accepted for `[mcp].excerpt_budget_chars`.
pub const MIN_EXCERPT_BUDGET_CHARS: usize = 256;

/// Upper bound accepted for `[mcp].excerpt_budget_chars`.
pub const MAX_EXCERPT_BUDGET_CHARS: usize = 32_000;

fn default_brief_max_chars() -> usize {
    8_000
}

fn default_excerpt_budget_chars() -> usize {
    DEFAULT_EXCERPT_BUDGET_CHARS
}

impl Default for McpSection {
    fn default() -> Self {
        Self {
            schema_profile: McpSchemaProfile::Full,
            tool_preset: McpToolPreset::Full,
            brief_max_chars: default_brief_max_chars(),
            excerpt_budget_chars: default_excerpt_budget_chars(),
            tasks: McpTasksSection::default(),
        }
    }
}
