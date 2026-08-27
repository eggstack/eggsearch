use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[allow(missing_docs)]
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ConflictClass {
    DifferingVersionRanges,
    ConflictingReleaseDates,
    MutuallyExclusiveStatusFields,
    DivergentBenchmarkNumbers,
    DocumentationImplementationMismatch,
    MutableVsCommitPinnedContent,
    DifferentProviderMetadata,
    #[default]
    Unknown,
}

#[allow(missing_docs)]
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ConflictSeverity {
    Critical,
    High,
    Medium,
    Low,
    #[default]
    Informational,
}

#[allow(missing_docs)]
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    PreferCommitPinned,
    PreferAuthoritativeSource,
    PreferNewerDate,
    PreferHigherVersion,
    ManualReviewRequired,
    #[default]
    NoRecommendation,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EvidenceConflict {
    pub id: String,
    pub source_ids: Vec<String>,
    pub conflict_class: ConflictClass,
    pub compared_fields: Vec<String>,
    pub values: Vec<String>,
    pub directly_comparable: bool,
    pub severity: ConflictSeverity,
    pub resolution: ConflictResolution,
    pub message: String,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Default)]
pub struct ConflictDetector {
    conflicts: Vec<EvidenceConflict>,
}

#[allow(missing_docs)]
impl ConflictDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_conflict(&mut self, conflict: EvidenceConflict) {
        self.conflicts.push(conflict);
    }

    pub fn is_empty(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.conflicts.len()
    }

    pub fn conflicts(&self) -> &[EvidenceConflict] {
        &self.conflicts
    }

    pub fn into_conflicts(self) -> Vec<EvidenceConflict> {
        self.conflicts
    }
}

/// A value annotated with its source provenance for conflict comparison.
#[derive(Clone, Debug)]
pub struct SourcedValue {
    /// The stable ID of the source card this value came from.
    pub source_id: String,
    /// The provider IDs that contributed to the source card.
    pub provider_ids: Vec<String>,
    /// The comparable value (e.g. a patched version string, a date).
    pub value: String,
}

fn compute_conflict_id(source_ids: &[String], field: &str) -> String {
    use super::identity::{write_entity_prefix, write_str, FnvHasher};

    let mut hasher = FnvHasher::new();
    write_entity_prefix(&mut hasher, "conflict");
    write_str(&mut hasher, &source_ids.join(","));
    write_str(&mut hasher, field);
    format!("conflict_{:016x}", hasher.finish())
}

/// Entity type for scoped conflict grouping.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum ConflictEntityType {
    Vulnerability,
    Package,
    Benchmark,
    Repository,
    Documentation,
}

/// Composite key for entity-scoped conflict grouping.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConflictEntityKey {
    /// The type of entity being compared.
    pub entity_type: ConflictEntityType,
    /// Canonical identifier (e.g. CVE ID, package name, benchmark name).
    pub canonical_id: String,
    /// The field on which values conflict.
    pub field: String,
}

/// Extract the canonical entity key for a source card.
///
/// Returns `None` when the card does not belong to a known entity type
/// or when the entity cannot be uniquely identified.
pub fn extract_entity_key(
    card: &crate::core::source_card::SourceCard,
) -> Option<ConflictEntityKey> {
    if let Some(ref vuln) = card.metadata.vulnerability {
        let canonical_id = vuln.cve_ids.first().cloned().or_else(|| {
            vuln.ghsa_ids
                .first()
                .cloned()
                .or_else(|| vuln.osv_ids.first().cloned())
        })?;
        if canonical_id.is_empty() {
            return None;
        }
        return Some(ConflictEntityKey {
            entity_type: ConflictEntityType::Vulnerability,
            canonical_id,
            field: String::new(),
        });
    }

    if let Some(ref code) = card.metadata.code_evidence {
        if let Some(ref repo) = code.repo {
            if !repo.is_empty() {
                return Some(ConflictEntityKey {
                    entity_type: ConflictEntityType::Repository,
                    canonical_id: repo.clone(),
                    field: String::new(),
                });
            }
        }
    }

    None
}

#[allow(missing_docs)]
pub fn detect_version_range_conflicts(
    ids_a: &[String],
    ids_b: &[String],
    field: &str,
    val_a: &str,
    val_b: &str,
) -> Option<EvidenceConflict> {
    if val_a == val_b {
        return None;
    }
    let mut source_ids = ids_a.to_vec();
    source_ids.extend_from_slice(ids_b);
    source_ids.sort();
    source_ids.dedup();
    Some(EvidenceConflict {
        id: compute_conflict_id(&source_ids, field),
        source_ids,
        conflict_class: ConflictClass::DifferingVersionRanges,
        compared_fields: vec![field.to_string()],
        values: vec![val_a.to_string(), val_b.to_string()],
        directly_comparable: true,
        severity: ConflictSeverity::Medium,
        resolution: ConflictResolution::PreferHigherVersion,
        message: format!("Sources disagree on version range for `{field}`: `{val_a}` vs `{val_b}`"),
    })
}

#[allow(missing_docs)]
pub fn detect_date_conflicts(
    ids_a: &[String],
    ids_b: &[String],
    field: &str,
    val_a: &str,
    val_b: &str,
) -> Option<EvidenceConflict> {
    if val_a == val_b {
        return None;
    }
    let mut source_ids = ids_a.to_vec();
    source_ids.extend_from_slice(ids_b);
    source_ids.sort();
    source_ids.dedup();
    Some(EvidenceConflict {
        id: compute_conflict_id(&source_ids, field),
        source_ids,
        conflict_class: ConflictClass::ConflictingReleaseDates,
        compared_fields: vec![field.to_string()],
        values: vec![val_a.to_string(), val_b.to_string()],
        directly_comparable: true,
        severity: ConflictSeverity::Medium,
        resolution: ConflictResolution::PreferNewerDate,
        message: format!("Sources disagree on date for `{field}`: `{val_a}` vs `{val_b}`"),
    })
}

#[allow(missing_docs)]
pub fn detect_provider_metadata_conflicts(
    ids_a: &[String],
    ids_b: &[String],
    field: &str,
    val_a: &str,
    val_b: &str,
) -> Option<EvidenceConflict> {
    if val_a == val_b {
        return None;
    }
    let mut source_ids = ids_a.to_vec();
    source_ids.extend_from_slice(ids_b);
    source_ids.sort();
    source_ids.dedup();
    Some(EvidenceConflict {
        id: compute_conflict_id(&source_ids, field),
        source_ids,
        conflict_class: ConflictClass::DifferentProviderMetadata,
        compared_fields: vec![field.to_string()],
        values: vec![val_a.to_string(), val_b.to_string()],
        directly_comparable: true,
        severity: ConflictSeverity::Medium,
        resolution: ConflictResolution::PreferAuthoritativeSource,
        message: format!("Providers disagree on metadata for `{field}`: `{val_a}` vs `{val_b}`"),
    })
}

#[allow(missing_docs)]
pub fn detect_mutable_vs_pinned(
    ids_mutable: &[String],
    ids_pinned: &[String],
) -> Option<EvidenceConflict> {
    if ids_mutable.is_empty() || ids_pinned.is_empty() {
        return None;
    }
    let field = "content_source";
    let mut source_ids = ids_mutable.to_vec();
    source_ids.extend_from_slice(ids_pinned);
    source_ids.sort();
    source_ids.dedup();
    Some(EvidenceConflict {
        id: compute_conflict_id(&source_ids, field),
        source_ids,
        conflict_class: ConflictClass::MutableVsCommitPinnedContent,
        compared_fields: vec![field.to_string()],
        values: vec![
            "mutable branch content".to_string(),
            "commit-pinned content".to_string(),
        ],
        directly_comparable: false,
        severity: ConflictSeverity::Critical,
        resolution: ConflictResolution::PreferCommitPinned,
        message: "Mutable branch content conflicts with commit-pinned content".to_string(),
    })
}

fn has_distinct_sources(sourced: &[SourcedValue]) -> bool {
    let mut seen = BTreeSet::new();
    for sv in sourced {
        seen.insert(&sv.source_id);
    }
    seen.len() >= 2
}

fn sourced_versions_for_cards(
    group: &[&crate::core::source_card::SourceCard],
) -> Vec<SourcedValue> {
    let mut result = Vec::new();
    for card in group {
        let source_id = match card.stable_id.as_deref() {
            Some(id) => id.to_string(),
            None => continue,
        };
        let providers = card.providers.clone();
        if let Some(ref vuln) = card.metadata.vulnerability {
            for v in &vuln.patched_versions {
                result.push(SourcedValue {
                    source_id: source_id.clone(),
                    provider_ids: providers.clone(),
                    value: v.clone(),
                });
            }
        }
    }
    result
}

fn sourced_dates_for_cards(group: &[&crate::core::source_card::SourceCard]) -> Vec<SourcedValue> {
    let mut result = Vec::new();
    for card in group {
        let source_id = match card.stable_id.as_deref() {
            Some(id) => id.to_string(),
            None => continue,
        };
        let providers = card.providers.clone();
        if let Some(ref vuln) = card.metadata.vulnerability {
            if let Some(ref published) = vuln.published_at {
                result.push(SourcedValue {
                    source_id,
                    provider_ids: providers,
                    value: published.clone(),
                });
            }
        }
    }
    result
}

fn package_key_for_card(card: &crate::core::source_card::SourceCard) -> Option<(String, String)> {
    let vuln = card.metadata.vulnerability.as_ref()?;
    let ecosystem = vuln.ecosystem.as_deref().unwrap_or("");
    let package = vuln.package.as_deref().unwrap_or("");
    Some((ecosystem.to_string(), package.to_string()))
}

fn repo_host_key(card: &crate::core::source_card::SourceCard) -> Option<(String, String, String)> {
    let code = card.metadata.code_evidence.as_ref()?;
    let host = code.host.as_ref().map(|h| format!("{h:?}"))?;
    let owner = code.owner.as_deref().unwrap_or("");
    let repo = code.repo.as_deref().unwrap_or("");
    Some((host, owner.to_string(), repo.to_string()))
}

/// Group source cards by their entity key and detect conflicts within each group.
///
/// Scoped conflict detection avoids false-positive cross-entity conflicts
/// by only comparing cards that share the same entity (e.g. the same CVE,
/// the same repository, or the same benchmark).
pub fn detect_entity_scoped_conflicts(
    cards: &[crate::core::source_card::SourceCard],
) -> Vec<EvidenceConflict> {
    let mut groups: BTreeMap<
        (ConflictEntityType, String),
        Vec<&crate::core::source_card::SourceCard>,
    > = BTreeMap::new();
    for card in cards {
        if let Some(key) = extract_entity_key(card) {
            groups
                .entry((key.entity_type, key.canonical_id))
                .or_default()
                .push(card);
        }
    }

    let mut conflicts = Vec::new();

    for ((entity_type, _canonical_id), group) in &groups {
        if group.len() < 2 {
            continue;
        }

        let ids: Vec<String> = group.iter().filter_map(|c| c.stable_id.clone()).collect();
        if ids.len() < 2 {
            continue;
        }

        match entity_type {
            ConflictEntityType::Vulnerability => {
                let mut per_package: BTreeMap<
                    (String, String),
                    Vec<&crate::core::source_card::SourceCard>,
                > = BTreeMap::new();
                for card in group {
                    let key = package_key_for_card(card)
                        .unwrap_or_else(|| (String::new(), String::new()));
                    per_package.entry(key).or_default().push(card);
                }

                for pkg_group in per_package.values() {
                    if pkg_group.len() < 2 {
                        continue;
                    }

                    let sourced = sourced_versions_for_cards(pkg_group);
                    if sourced.is_empty() {
                        continue;
                    }

                    let per_source_sets: BTreeMap<String, BTreeSet<String>> = {
                        let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
                        for sv in &sourced {
                            map.entry(sv.source_id.clone())
                                .or_default()
                                .insert(sv.value.clone());
                        }
                        map
                    };

                    let mut source_set_keys: Vec<(String, BTreeSet<String>)> =
                        per_source_sets.into_iter().collect();
                    source_set_keys.sort_by(|a, b| a.0.cmp(&b.0));

                    for i in 0..source_set_keys.len() {
                        for j in (i + 1)..source_set_keys.len() {
                            let (ref id_a, ref set_a) = source_set_keys[i];
                            let (ref id_b, ref set_b) = source_set_keys[j];
                            if set_a != set_b {
                                let mut vals_a: Vec<String> = set_a.iter().cloned().collect();
                                vals_a.sort();
                                let mut vals_b: Vec<String> = set_b.iter().cloned().collect();
                                vals_b.sort();
                                let val_a = vals_a.join(", ");
                                let val_b = vals_b.join(", ");
                                let ids_a = vec![id_a.clone()];
                                let ids_b = vec![id_b.clone()];
                                if let Some(conflict) = detect_version_range_conflicts(
                                    &ids_a,
                                    &ids_b,
                                    "patched_versions",
                                    &val_a,
                                    &val_b,
                                ) {
                                    conflicts.push(conflict);
                                }
                            }
                        }
                    }
                }

                let sourced_dates = sourced_dates_for_cards(group);
                if sourced_dates.len() >= 2 && has_distinct_sources(&sourced_dates) {
                    let per_source_dates: BTreeMap<String, BTreeSet<String>> = {
                        let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
                        for sv in &sourced_dates {
                            map.entry(sv.source_id.clone())
                                .or_default()
                                .insert(sv.value.clone());
                        }
                        map
                    };

                    let mut date_source_keys: Vec<(String, BTreeSet<String>)> =
                        per_source_dates.into_iter().collect();
                    date_source_keys.sort_by(|a, b| a.0.cmp(&b.0));

                    if date_source_keys.len() >= 2 {
                        for i in 0..date_source_keys.len() {
                            for j in (i + 1)..date_source_keys.len() {
                                let (ref id_a, ref dates_a) = date_source_keys[i];
                                let (ref id_b, ref dates_b) = date_source_keys[j];
                                if dates_a != dates_b {
                                    let mut vals_a: Vec<String> = dates_a.iter().cloned().collect();
                                    vals_a.sort();
                                    let val_a = vals_a.join(", ");
                                    let mut vals_b: Vec<String> = dates_b.iter().cloned().collect();
                                    vals_b.sort();
                                    let val_b = vals_b.join(", ");
                                    let ids_a = vec![id_a.clone()];
                                    let ids_b = vec![id_b.clone()];
                                    if let Some(conflict) = detect_date_conflicts(
                                        &ids_a,
                                        &ids_b,
                                        "published_at",
                                        &val_a,
                                        &val_b,
                                    ) {
                                        conflicts.push(conflict);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ConflictEntityType::Repository => {
                let mut per_repo_host: BTreeMap<
                    (String, String, String),
                    Vec<&crate::core::source_card::SourceCard>,
                > = BTreeMap::new();
                for card in group {
                    if let Some(key) = repo_host_key(card) {
                        per_repo_host.entry(key).or_default().push(card);
                    }
                }

                for repo_group in per_repo_host.values() {
                    if repo_group.len() < 2 {
                        continue;
                    }

                    let repo_ids: Vec<String> = repo_group
                        .iter()
                        .filter_map(|c| c.stable_id.clone())
                        .collect();
                    if repo_ids.len() < 2 {
                        continue;
                    }

                    let mut mutable_ids = Vec::new();
                    let mut pinned_ids = Vec::new();
                    for card in repo_group {
                        let id = match card.stable_id.as_deref() {
                            Some(id) => id.to_string(),
                            None => continue,
                        };
                        let is_pinned = card
                            .metadata
                            .code_evidence
                            .as_ref()
                            .is_some_and(|c| c.commit_sha.is_some());
                        if is_pinned {
                            pinned_ids.push(id);
                        } else {
                            mutable_ids.push(id);
                        }
                    }

                    if let Some(conflict) = detect_mutable_vs_pinned(&mutable_ids, &pinned_ids) {
                        conflicts.push(conflict);
                    }
                }
            }
            _ => {}
        }
    }

    conflicts
}

#[allow(missing_docs)]
pub fn detect_benchmark_conflicts(
    ids_a: &[String],
    ids_b: &[String],
    benchmark_name: &str,
    val_a: &str,
    val_b: &str,
) -> Option<EvidenceConflict> {
    if val_a == val_b {
        return None;
    }
    let mut source_ids = ids_a.to_vec();
    source_ids.extend_from_slice(ids_b);
    source_ids.sort();
    source_ids.dedup();
    Some(EvidenceConflict {
        id: compute_conflict_id(&source_ids, benchmark_name),
        source_ids,
        conflict_class: ConflictClass::DivergentBenchmarkNumbers,
        compared_fields: vec![benchmark_name.to_string()],
        values: vec![val_a.to_string(), val_b.to_string()],
        directly_comparable: true,
        severity: ConflictSeverity::High,
        resolution: ConflictResolution::ManualReviewRequired,
        message: format!(
            "Divergent benchmark numbers for `{benchmark_name}`: `{val_a}` vs `{val_b}`"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_version_range_conflicts_returns_conflict() {
        let a = vec!["src_a".to_string()];
        let b = vec!["src_b".to_string()];
        let c =
            detect_version_range_conflicts(&a, &b, "affected_versions", ">=1.0 <2.0", ">=1.5 <3.0");
        let c = c.unwrap();
        assert_eq!(c.conflict_class, ConflictClass::DifferingVersionRanges);
        assert_eq!(c.severity, ConflictSeverity::Medium);
        assert_eq!(c.resolution, ConflictResolution::PreferHigherVersion);
        assert!(c.directly_comparable);
        assert_eq!(c.compared_fields, vec!["affected_versions"]);
        assert_eq!(c.values, vec![">=1.0 <2.0", ">=1.5 <3.0"]);
        assert!(c.id.starts_with("conflict_"));
    }

    #[test]
    fn detect_version_range_conflicts_no_conflict_when_equal() {
        let a = vec!["src_a".to_string()];
        let b = vec!["src_b".to_string()];
        assert!(detect_version_range_conflicts(&a, &b, "f", "1.0", "1.0").is_none());
    }

    #[test]
    fn detect_date_conflicts_returns_conflict() {
        let a = vec!["src_a".to_string()];
        let b = vec!["src_b".to_string()];
        let c = detect_date_conflicts(&a, &b, "release_date", "2024-01-01", "2024-06-15");
        let c = c.unwrap();
        assert_eq!(c.conflict_class, ConflictClass::ConflictingReleaseDates);
        assert_eq!(c.severity, ConflictSeverity::Medium);
        assert_eq!(c.resolution, ConflictResolution::PreferNewerDate);
        assert!(c.directly_comparable);
    }

    #[test]
    fn detect_date_conflicts_no_conflict_when_equal() {
        let a = vec!["src_a".to_string()];
        let b = vec!["src_b".to_string()];
        assert!(detect_date_conflicts(&a, &b, "date", "2024-01-01", "2024-01-01").is_none());
    }

    #[test]
    fn detect_provider_metadata_conflicts_returns_conflict() {
        let a = vec!["src_a".to_string()];
        let b = vec!["src_b".to_string()];
        let c = detect_provider_metadata_conflicts(&a, &b, "license", "MIT", "Apache-2.0");
        let c = c.unwrap();
        assert_eq!(c.conflict_class, ConflictClass::DifferentProviderMetadata);
        assert_eq!(c.resolution, ConflictResolution::PreferAuthoritativeSource);
        assert!(c.directly_comparable);
    }

    #[test]
    fn detect_provider_metadata_conflicts_no_conflict_when_equal() {
        let a = vec!["src_a".to_string()];
        let b = vec!["src_b".to_string()];
        assert!(detect_provider_metadata_conflicts(&a, &b, "license", "MIT", "MIT").is_none());
    }

    #[test]
    fn detect_mutable_vs_pinned_returns_conflict() {
        let m = vec!["src_branch".to_string()];
        let p = vec!["src_commit".to_string()];
        let c = detect_mutable_vs_pinned(&m, &p).unwrap();
        assert_eq!(
            c.conflict_class,
            ConflictClass::MutableVsCommitPinnedContent
        );
        assert_eq!(c.severity, ConflictSeverity::Critical);
        assert_eq!(c.resolution, ConflictResolution::PreferCommitPinned);
        assert!(!c.directly_comparable);
    }

    #[test]
    fn detect_mutable_vs_pinned_no_conflict_when_either_empty() {
        assert!(detect_mutable_vs_pinned(&[], &["src_a".to_string()]).is_none());
        assert!(detect_mutable_vs_pinned(&["src_a".to_string()], &[]).is_none());
    }

    #[test]
    fn detect_benchmark_conflicts_returns_conflict() {
        let a = vec!["src_a".to_string()];
        let b = vec!["src_b".to_string()];
        let c = detect_benchmark_conflicts(&a, &b, "throughput", "1500 ops/s", "2200 ops/s");
        let c = c.unwrap();
        assert_eq!(c.conflict_class, ConflictClass::DivergentBenchmarkNumbers);
        assert_eq!(c.severity, ConflictSeverity::High);
        assert_eq!(c.resolution, ConflictResolution::ManualReviewRequired);
        assert!(c.directly_comparable);
    }

    #[test]
    fn detect_benchmark_conflicts_no_conflict_when_equal() {
        let a = vec!["src_a".to_string()];
        let b = vec!["src_b".to_string()];
        assert!(detect_benchmark_conflicts(&a, &b, "throughput", "1500", "1500").is_none());
    }

    #[test]
    fn deterministic_id_generation() {
        let a = vec!["src_x".to_string()];
        let b = vec!["src_y".to_string()];
        let c1 = detect_version_range_conflicts(&a, &b, "field", "v1", "v2").unwrap();
        let c2 = detect_version_range_conflicts(&a, &b, "field", "v1", "v2").unwrap();
        assert_eq!(c1.id, c2.id);
    }

    #[test]
    fn different_inputs_produce_different_ids() {
        let a = vec!["src_x".to_string()];
        let b = vec!["src_y".to_string()];
        let c1 = detect_version_range_conflicts(&a, &b, "field_a", "v1", "v2").unwrap();
        let c2 = detect_version_range_conflicts(&a, &b, "field_b", "v1", "v2").unwrap();
        assert_ne!(c1.id, c2.id);
    }

    #[test]
    fn serde_roundtrip() {
        let a = vec!["src_a".to_string()];
        let b = vec!["src_b".to_string()];
        let conflict = detect_benchmark_conflicts(&a, &b, "latency", "10ms", "50ms").unwrap();
        let json = serde_json::to_string(&conflict).unwrap();
        let deserialized: EvidenceConflict = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, conflict.id);
        assert_eq!(deserialized.conflict_class, conflict.conflict_class);
        assert_eq!(deserialized.severity, conflict.severity);
        assert_eq!(deserialized.resolution, conflict.resolution);
    }

    #[test]
    fn conflict_detector_methods() {
        let mut det = ConflictDetector::new();
        assert!(det.is_empty());
        assert_eq!(det.len(), 0);
        assert!(det.conflicts().is_empty());

        let a = vec!["src_a".to_string()];
        let b = vec!["src_b".to_string()];
        let c = detect_version_range_conflicts(&a, &b, "f", "1", "2").unwrap();
        det.add_conflict(c);
        assert!(!det.is_empty());
        assert_eq!(det.len(), 1);
        assert_eq!(det.conflicts().len(), 1);

        let c2 = detect_benchmark_conflicts(&a, &b, "b", "1", "2").unwrap();
        det.add_conflict(c2);
        assert_eq!(det.len(), 2);

        let all = det.into_conflicts();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn source_ids_deduped_in_conflict() {
        let a = vec!["src_shared".to_string(), "src_a".to_string()];
        let b = vec!["src_shared".to_string(), "src_b".to_string()];
        let c = detect_version_range_conflicts(&a, &b, "f", "1", "2").unwrap();
        assert!(c.source_ids.contains(&"src_shared".to_string()));
        assert!(c.source_ids.contains(&"src_a".to_string()));
        assert!(c.source_ids.contains(&"src_b".to_string()));
        assert_eq!(c.source_ids.len(), 3);
    }
}
