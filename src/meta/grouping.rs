//! Shared helpers for deterministic `SourceCard` result grouping.

use std::collections::HashMap;
use std::hash::Hash;

use crate::core::quality::{compute_group_quality, GroupQualitySummary};
use crate::core::SourceCard;

/// A fully materialized result group before conversion to a public response type.
pub struct BuiltGroup<K> {
    pub kind: K,
    pub label: String,
    pub results: Vec<SourceCard>,
    pub truncated: bool,
    pub quality_summary: GroupQualitySummary,
}

/// Bucket cards by a classifier, optionally rerank buckets, then emit non-empty
/// groups in caller-provided canonical order.
pub fn build_card_groups<K, Classify, Label, Rerank>(
    cards: Vec<SourceCard>,
    classify: Classify,
    canonical_order: &[K],
    label_for: Label,
    max_per_group: usize,
    max_groups: Option<usize>,
    mut rerank: Rerank,
) -> Vec<BuiltGroup<K>>
where
    K: Copy + Eq + Hash,
    Classify: Fn(&SourceCard) -> K,
    Label: Fn(K) -> String,
    Rerank: FnMut(K, &mut [SourceCard]),
{
    let mut buckets: HashMap<K, Vec<SourceCard>> = HashMap::new();
    for card in cards {
        buckets.entry(classify(&card)).or_default().push(card);
    }

    for (kind, bucket) in buckets.iter_mut() {
        rerank(*kind, bucket);
    }

    let mut groups = Vec::new();
    for &kind in canonical_order {
        if max_groups.is_some_and(|cap| groups.len() >= cap) {
            break;
        }

        if let Some(mut results) = buckets.remove(&kind) {
            let full_count = results.len();
            results.truncate(max_per_group);
            let truncated = full_count > max_per_group;
            let quality_summary = compute_group_quality(&results);
            groups.push(BuiltGroup {
                kind,
                label: label_for(kind),
                results,
                truncated,
                quality_summary,
            });
        }
    }

    groups
}
