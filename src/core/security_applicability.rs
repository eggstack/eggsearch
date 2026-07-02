#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

use crate::core::package::PackageEcosystem;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ApplicabilityStatus {
    Affected,
    NotAffected,
    #[default]
    Unknown,
}

/// Internal tri-state result for advisory range evaluation.
///
/// This preserves the distinction between `NotAffected` (the advisory
/// explicitly excludes this version) and `Unknown` (the advisory could
/// not be evaluated for this version, e.g. unparseable range syntax
/// or unsupported range type). Collapsing unknown into not-affected
/// produces dangerous false-negatives for security triage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RangeMatch {
    Affected,
    NotAffected,
    #[default]
    Unknown,
}

impl RangeMatch {
    pub fn is_affected(self) -> bool {
        matches!(self, RangeMatch::Affected)
    }

    /// Combine two range-match results using the rules:
    /// - `Affected` dominates everything.
    /// - All `NotAffected` resolves to `NotAffected` (every range was
    ///   evaluated and excluded this version).
    /// - Any `Unknown` mixed with `NotAffected` resolves to `Unknown`
    ///   (some range could not be evaluated, so we cannot conclude
    ///   the version is safe).
    /// - All `Unknown` stays `Unknown`.
    pub fn combine(self, other: RangeMatch) -> RangeMatch {
        match (self, other) {
            (RangeMatch::Affected, _) | (_, RangeMatch::Affected) => RangeMatch::Affected,
            (RangeMatch::NotAffected, RangeMatch::NotAffected) => RangeMatch::NotAffected,
            (RangeMatch::NotAffected, RangeMatch::Unknown)
            | (RangeMatch::Unknown, RangeMatch::NotAffected) => RangeMatch::Unknown,
            (RangeMatch::Unknown, RangeMatch::Unknown) => RangeMatch::Unknown,
        }
    }

    pub fn from_satisfied(satisfied: Option<bool>) -> RangeMatch {
        match satisfied {
            Some(true) => RangeMatch::Affected,
            Some(false) => RangeMatch::NotAffected,
            None => RangeMatch::Unknown,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ApplicabilityConfidence {
    High,
    Medium,
    #[default]
    Low,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
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
