use serde::{Deserialize, Serialize};

use crate::core::security::VulnerabilityMetadata;
use crate::core::source_card::{IssueMetadata, ReleaseMetadata};

#[allow(missing_docs)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ResultMetadata {
    #[default]
    None,
    Issue(IssueMetadata),
    Release(ReleaseMetadata),
    Advisory(Box<VulnerabilityMetadata>),
}

impl ResultMetadata {
    /// Merge two metadata values from deduplicated rows.
    ///
    /// The merge is **idempotent and order-independent**: the richer
    /// variant wins, and within the same variant the non-empty fields
    /// win (currently a no-op since `IssueMetadata` and
    /// `ReleaseMetadata` are fully populated by their native engines;
    /// the policy is fixed here for forward compatibility).
    ///
    /// Used by the RRF aggregator when the same URL is returned by
    /// multiple providers. A `github_issues` row that carries a real
    /// `IssueMetadata` payload must not be overwritten by a `None`
    /// from a generic HTML scraper that happened to scrape the same
    /// page later.
    pub fn merge(self, other: ResultMetadata) -> ResultMetadata {
        match (self, other) {
            // Same-variant merges keep `self` (the existing richer row).
            (ResultMetadata::Issue(a), ResultMetadata::Issue(b)) => {
                ResultMetadata::Issue(IssueMetadata::merge(a, b))
            }
            (ResultMetadata::Release(a), ResultMetadata::Release(b)) => {
                ResultMetadata::Release(ReleaseMetadata::merge(a, b))
            }
            (ResultMetadata::Advisory(a), ResultMetadata::Advisory(b)) => {
                ResultMetadata::Advisory(Box::new(VulnerabilityMetadata::merge(*a, *b)))
            }
            // Mixed / None: the structured variant always wins over
            // `None`. If both sides are structured but of different
            // kinds (theoretically possible if a release URL is
            // misclassified), prefer the first non-`None` to keep
            // the original kind stable.
            (ResultMetadata::None, other) => other,
            (this, ResultMetadata::None) => this,
            // Two different structured kinds: keep the left side. This
            // is intentionally a no-op rather than panicking because
            // the merged record represents the same URL; the original
            // classification is preserved.
            (this, _other_structured) => this,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
    pub source_engine: String,
    #[serde(default)]
    pub metadata: ResultMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResult {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
    pub engines: Vec<String>,
    pub score: f64,
    #[serde(default)]
    pub metadata: ResultMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_none_into_structured_keeps_structured() {
        let issue = ResultMetadata::Issue(IssueMetadata {
            number: Some(42),
            ..Default::default()
        });
        assert_eq!(issue.clone().merge(ResultMetadata::None), issue);
        assert_eq!(ResultMetadata::None.merge(issue.clone()), issue);
    }

    #[test]
    fn merge_none_into_none_is_none() {
        assert_eq!(
            ResultMetadata::None.merge(ResultMetadata::None),
            ResultMetadata::None
        );
    }

    #[test]
    fn merge_issue_with_issue_keeps_left_side() {
        let a = ResultMetadata::Issue(IssueMetadata {
            number: Some(42),
            ..Default::default()
        });
        let b = ResultMetadata::Issue(IssueMetadata {
            number: Some(99),
            ..Default::default()
        });
        let merged = a.clone().merge(b);
        match merged {
            ResultMetadata::Issue(m) => assert_eq!(m.number, Some(42)),
            other => panic!("expected Issue, got {other:?}"),
        }
    }

    #[test]
    fn merge_release_with_release_keeps_left_side() {
        let a = ResultMetadata::Release(ReleaseMetadata {
            tag: Some("v1.0.0".to_string()),
            ..Default::default()
        });
        let b = ResultMetadata::Release(ReleaseMetadata {
            tag: Some("v2.0.0".to_string()),
            ..Default::default()
        });
        let merged = a.clone().merge(b);
        match merged {
            ResultMetadata::Release(m) => assert_eq!(m.tag.as_deref(), Some("v1.0.0")),
            other => panic!("expected Release, got {other:?}"),
        }
    }

    #[test]
    fn merge_structured_kind_mismatch_keeps_left_side() {
        // Defensive: a release URL that arrives via two providers
        // and is misclassified by one should not flip the variant.
        let release = ResultMetadata::Release(ReleaseMetadata::default());
        let issue = ResultMetadata::Issue(IssueMetadata::default());
        assert!(matches!(
            release.clone().merge(issue),
            ResultMetadata::Release(_)
        ));
    }

    #[test]
    fn merge_advisory_with_advisory_keeps_left_side() {
        let a = ResultMetadata::Advisory(Box::new(VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            ..Default::default()
        }));
        let b = ResultMetadata::Advisory(Box::new(VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0002".to_string()],
            ..Default::default()
        }));
        let merged = a.clone().merge(b);
        match merged {
            ResultMetadata::Advisory(m) => {
                assert_eq!(m.cve_ids, vec!["CVE-2024-0001", "CVE-2024-0002"]);
            }
            other => panic!("expected Advisory, got {other:?}"),
        }
    }

    #[test]
    fn merge_none_into_advisory_keeps_advisory() {
        let advisory = ResultMetadata::Advisory(Box::new(VulnerabilityMetadata::default()));
        assert_eq!(advisory.clone().merge(ResultMetadata::None), advisory);
        assert_eq!(ResultMetadata::None.merge(advisory.clone()), advisory);
    }
}
