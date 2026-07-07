use std::time::Duration;

use reqwest::Client;

use super::error::EngineError;
use super::kev::KevClient;
use super::models::{ResultMetadata, SearchResult};
use crate::core::security::{VulnerabilityMetadata, VulnerabilityReference, VulnerabilitySource};

const ENGINE: &str = "cisa_kev";

pub struct CisaKevEngine {
    pub client: Client,
    pub kev_client: KevClient,
}

impl CisaKevEngine {
    pub fn new(client: Client) -> Self {
        let kev_client = KevClient::new(client.clone());
        Self { client, kev_client }
    }
}

impl super::SearchEngine for CisaKevEngine {
    fn name(&self) -> &'static str {
        ENGINE
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        max_results: usize,
        timeout: Duration,
    ) -> super::BoxFuture<'a, Result<Vec<SearchResult>, EngineError>> {
        Box::pin(async move {
            if max_results == 0 {
                return Ok(Vec::new());
            }
            search_kev_catalog(&self.kev_client, query, max_results, timeout).await
        })
    }

    fn lookup_advisory<'a>(
        &'a self,
        vuln_id: &'a str,
        _timeout: Duration,
    ) -> super::BoxFuture<
        'a,
        Result<Option<crate::core::security::VulnerabilityMetadata>, EngineError>,
    > {
        Box::pin(async move {
            let upper = vuln_id.to_uppercase();
            if !upper.starts_with("CVE-") {
                return Ok(None);
            }
            match self
                .kev_client
                .lookup(&upper)
                .await
                .map_err(|e| EngineError::NetworkError {
                    engine: ENGINE,
                    reason: e.to_string(),
                })? {
                Some(kev_meta) => {
                    let metadata = VulnerabilityMetadata {
                        cve_ids: vec![upper.clone()],
                        ghsa_ids: Vec::new(),
                        osv_ids: Vec::new(),
                        rustsec_ids: Vec::new(),
                        ecosystem: None,
                        package: kev_meta.product.clone(),
                        affected_ranges: Vec::new(),
                        patched_ranges: Vec::new(),
                        vulnerable_versions: Vec::new(),
                        patched_versions: Vec::new(),
                        severity: None,
                        cvss_score: None,
                        cvss_vector: None,
                        epss_score: None,
                        kev: Some(kev_meta.clone()),
                        published_at: None,
                        modified_at: None,
                        withdrawn_at: None,
                        references: vec![VulnerabilityReference {
                            url: "https://www.cisa.gov/known-exploited-vulnerabilities-catalog"
                                .to_string(),
                            kind: Some("KEV Catalog".to_string()),
                        }],
                        source: VulnerabilitySource::CisaKev,
                    };
                    Ok(Some(metadata))
                }
                None => Ok(None),
            }
        })
    }
}

async fn search_kev_catalog(
    kev_client: &KevClient,
    query: &str,
    max_results: usize,
    _timeout: Duration,
) -> Result<Vec<SearchResult>, EngineError> {
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();

    // Try to fetch the full catalog and filter by keyword
    let catalog = kev_client
        .lookup(&query_uppercase(&query_lower))
        .await
        .map_err(|e| EngineError::NetworkError {
            engine: ENGINE,
            reason: e.to_string(),
        })?;

    // If the query matched a specific CVE, return it
    if let Some(kev_meta) = catalog {
        let cve_id = query_uppercase(&query_lower);
        let title = format!(
            "{cve_id}: {} (KEV Catalog)",
            kev_meta
                .product
                .as_deref()
                .unwrap_or("known exploited vulnerability")
        );
        let url = "https://www.cisa.gov/known-exploited-vulnerabilities-catalog".to_string();
        let snippet = kev_meta
            .required_action
            .as_deref()
            .map(|a| format!("Required action: {a}"));

        let metadata = VulnerabilityMetadata {
            cve_ids: vec![cve_id],
            ghsa_ids: Vec::new(),
            osv_ids: Vec::new(),
            rustsec_ids: Vec::new(),
            ecosystem: None,
            package: kev_meta.product.clone(),
            affected_ranges: Vec::new(),
            patched_ranges: Vec::new(),
            vulnerable_versions: Vec::new(),
            patched_versions: Vec::new(),
            severity: None,
            cvss_score: None,
            cvss_vector: None,
            epss_score: None,
            kev: Some(kev_meta),
            published_at: None,
            modified_at: None,
            withdrawn_at: None,
            references: vec![VulnerabilityReference {
                url: "https://www.cisa.gov/known-exploited-vulnerabilities-catalog".to_string(),
                kind: Some("KEV Catalog".to_string()),
            }],
            source: VulnerabilitySource::CisaKev,
        };

        results.push(SearchResult {
            title,
            url,
            snippet,
            source_engine: ENGINE.to_string(),
            metadata: ResultMetadata::Advisory(Box::new(metadata)),
        });
    }

    // Keyword-based search is limited since KEV is a flat catalog.
    // The lookup function handles exact CVE matches. For broader
    // keyword search we return what we found (either the CVE match or empty).
    Ok(results.into_iter().take(max_results).collect())
}

fn query_uppercase(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.len() >= 4 && trimmed[..4].eq_ignore_ascii_case("CVE-") {
        trimmed.to_uppercase()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::engines::SearchEngine;

    #[test]
    fn test_provider_descriptor() {
        use crate::core::provider::built_in_provider_descriptor;

        let desc =
            built_in_provider_descriptor("cisa_kev", true, false, true, false, None, None).unwrap();
        assert_eq!(desc.id, "cisa_kev");
        assert!(desc.capabilities.supports_exploit_kev_status);
        assert!(desc.capabilities.supports_advisory_lookup_by_id);
        assert!(!desc.capabilities.supports_security_search);
    }

    #[test]
    fn test_query_uppercase_cve() {
        assert_eq!(query_uppercase("cve-2024-12345"), "CVE-2024-12345");
        assert_eq!(query_uppercase("CVE-2024-12345"), "CVE-2024-12345");
    }

    #[test]
    fn test_query_uppercase_non_cve() {
        assert_eq!(query_uppercase("apache"), "apache");
    }

    #[tokio::test]
    async fn test_kev_client_creation() {
        let client = Client::new();
        let engine = CisaKevEngine::new(client);
        assert_eq!(engine.name(), "cisa_kev");
    }
}
