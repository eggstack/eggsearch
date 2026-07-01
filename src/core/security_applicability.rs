#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

use crate::core::package::PackageEcosystem;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicabilityStatus {
    Affected,
    NotAffected,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicabilityConfidence {
    High,
    Medium,
    #[default]
    Low,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependencySource {
    LockFile,
    Manifest,
    Dockerfile,
    WorkflowFile,
    AdvisoryMetadata,
    #[default]
    RequestField,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdvisoryRange {
    pub ecosystem: PackageEcosystem,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_range: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed_versions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub introduced_versions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub last_affected_versions: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DependencyFinding {
    pub ecosystem: PackageEcosystem,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_line: Option<u32>,
    pub source_kind: DependencySource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<ApplicabilityConfidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ApplicabilityAssessment {
    pub status: ApplicabilityStatus,
    pub confidence: ApplicabilityConfidence,
    pub ecosystem: PackageEcosystem,
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisory_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_ranges: Vec<AdvisoryRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}
