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
    let mut bucket_order = Vec::new();
    for card in cards {
        let kind = classify(&card);
        if !buckets.contains_key(&kind) {
            bucket_order.push(kind);
        }
        buckets.entry(kind).or_default().push(card);
    }

    for (kind, bucket) in buckets.iter_mut() {
        rerank(*kind, bucket);
    }

    let mut groups = Vec::new();
    let mut ordered_kinds = canonical_order.to_vec();
    ordered_kinds.extend(
        bucket_order
            .into_iter()
            .filter(|kind| !canonical_order.contains(kind)),
    );

    for kind in ordered_kinds {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::result::TrustLevel;

    fn card() -> SourceCard {
        SourceCard::new(
            "title",
            "https://example.com",
            vec!["test".into()],
            None,
            TrustLevel::ExternalUntrusted,
        )
    }

    #[test]
    fn emits_buckets_missing_from_canonical_order() {
        let groups = build_card_groups(
            vec![card()],
            |_| 2u8,
            &[1u8],
            |kind| format!("group-{kind}"),
            10,
            None,
            |_, _| {},
        );

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].kind, 2);
    }

    #[test]
    fn max_groups_zero_emits_no_groups() {
        let groups = build_card_groups(
            vec![card()],
            |_| 1u8,
            &[1u8],
            |kind| format!("group-{kind}"),
            10,
            Some(0),
            |_, _| {},
        );

        assert!(groups.is_empty());
    }
}
