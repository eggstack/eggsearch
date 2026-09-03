use serde::{Deserialize, Serialize};

use crate::core::security::VulnerabilityMetadata;
use crate::core::source_card::{IssueMetadata, ReleaseMetadata, SourceExcerpt};

/// Structured metadata from a code-search provider (e.g. GitHub Code Search).
///
/// Carries matched symbol and text fragment data that can be promoted
/// into `CodeEvidence` during source-card conversion.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CodeSearchMetadata {
    /// The matched text from the provider (e.g. function/struct name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_symbol: Option<String>,
    /// A snippet of the matching file content around the match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_fragment: Option<String>,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ResultMetadata {
    #[default]
    None,
    Issue(IssueMetadata),
    Release(ReleaseMetadata),
    Advisory(Box<VulnerabilityMetadata>),
    CodeSearch(CodeSearchMetadata),
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
            (ResultMetadata::CodeSearch(a), ResultMetadata::CodeSearch(b)) => {
                ResultMetadata::CodeSearch(CodeSearchMetadata {
                    matched_symbol: a.matched_symbol.or(b.matched_symbol),
                    text_fragment: a.text_fragment.or(b.text_fragment),
                })
            }
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excerpts: Vec<SourceExcerpt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excerpts: Vec<SourceExcerpt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
}

/// Provider-neutral scope-index status for a requested repository or
/// documentation scope.
///
/// Preserved at the response/retrieval layer, never on `SourceCard`.
/// Lets callers distinguish "scope not indexed" from "indexed but zero
/// matches".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeIndexStatus {
    /// The requested scope as echoed upstream (e.g. `owner/repo`).
    pub scope: String,
    /// Whether the upstream index holds this scope.
    pub indexed: bool,
}

/// Provider-neutral retrieval metadata attached to one engine batch.
///
/// Currently carries only scope-index evidence (e.g. Firecrawl Developer
/// `repos`/`sources` echo). Empty by default for providers without
/// scope semantics.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineRetrievalMetadata {
    /// Scope-index evidence for explicitly requested scopes.
    #[serde(default)]
    pub scope_index: Vec<ScopeIndexStatus>,
}

impl EngineRetrievalMetadata {
    /// Scopes reported as not indexed.
    pub fn unindexed_scopes(&self) -> Vec<&str> {
        self.scope_index
            .iter()
            .filter(|s| !s.indexed)
            .map(|s| s.scope.as_str())
            .collect()
    }

    /// Whether any requested scope was reported as not indexed.
    pub fn has_unindexed(&self) -> bool {
        self.scope_index.iter().any(|s| !s.indexed)
    }
}

/// One engine's results plus provider-neutral retrieval metadata.
#[derive(Clone, Debug, Default)]
pub struct EngineSearchBatch {
    /// Ranked results from the provider.
    pub results: Vec<SearchResult>,
    /// Retrieval-state evidence (scope-index, etc.).
    pub retrieval_metadata: EngineRetrievalMetadata,
}

impl EngineSearchBatch {
    /// Batch with results and empty retrieval metadata.
    pub fn from_results(results: Vec<SearchResult>) -> Self {
        Self {
            results,
            retrieval_metadata: EngineRetrievalMetadata::default(),
        }
    }
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
        let advisory = ResultMetadata::Advisory(Box::default());
        assert_eq!(advisory.clone().merge(ResultMetadata::None), advisory);
        assert_eq!(ResultMetadata::None.merge(advisory.clone()), advisory);
    }

    #[test]
    fn merge_code_search_with_code_search_merges_fields() {
        let a = ResultMetadata::CodeSearch(CodeSearchMetadata {
            matched_symbol: Some("router".to_string()),
            text_fragment: None,
        });
        let b = ResultMetadata::CodeSearch(CodeSearchMetadata {
            matched_symbol: None,
            text_fragment: Some("fn main() {}".to_string()),
        });
        let merged = a.merge(b);
        match merged {
            ResultMetadata::CodeSearch(m) => {
                assert_eq!(m.matched_symbol.as_deref(), Some("router"));
                assert_eq!(m.text_fragment.as_deref(), Some("fn main() {}"));
            }
            other => panic!("expected CodeSearch, got {other:?}"),
        }
    }

    #[test]
    fn merge_none_into_code_search_keeps_code_search() {
        let cs = ResultMetadata::CodeSearch(CodeSearchMetadata {
            matched_symbol: Some("foo".to_string()),
            text_fragment: None,
        });
        assert_eq!(cs.clone().merge(ResultMetadata::None), cs);
        assert_eq!(ResultMetadata::None.merge(cs.clone()), cs);
    }
}
