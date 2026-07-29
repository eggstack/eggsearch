use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;

use crate::core::security::KevMetadata;

const KEV_CATALOG_URL: &str =
    "https://www.cisa.gov/sites/default/files/feeds/known_exploited_vulnerabilities.json";
const MAX_BODY_BYTES: usize = 100 * 1024 * 1024; // 100MB
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour
const KEV_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Deserialize)]
struct KeCatalog {
    #[serde(default)]
    vulnerabilities: Vec<KeVulnerability>,
    #[serde(rename = "dateReleased")]
    #[serde(default)]
    date_released: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct KeVulnerability {
    #[serde(rename = "cveID")]
    cve_id: String,
    #[serde(rename = "vendorProject")]
    vendor_project: Option<String>,
    #[serde(rename = "product")]
    product: Option<String>,
    #[serde(rename = "vulnerabilityName")]
    vulnerability_name: Option<String>,
    #[serde(rename = "dateAdded")]
    date_added: Option<String>,
    #[serde(rename = "shortDescription")]
    short_description: Option<String>,
    #[serde(rename = "requiredAction")]
    required_action: Option<String>,
    #[serde(rename = "dueDate")]
    due_date: Option<String>,
    #[serde(rename = "knownRansomwareCampaignUse")]
    known_ransomware_campaign_use: Option<String>,
    #[serde(rename = "notes")]
    notes: Option<String>,
}

pub struct KevClient {
    client: Client,
    cache: Arc<tokio::sync::RwLock<KevCache>>,
    cache_ttl: Duration,
}

struct KevCache {
    entries: HashMap<String, KevEntry>,
    catalog_date: Option<String>,
    fetched_at: Option<Instant>,
}

struct KevEntry {
    metadata: KevMetadata,
    #[allow(dead_code)]
    cve_id: String,
}

impl KevClient {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            cache: Arc::new(tokio::sync::RwLock::new(KevCache {
                entries: HashMap::new(),
                catalog_date: None,
                fetched_at: None,
            })),
            cache_ttl: DEFAULT_CACHE_TTL,
        }
    }

    pub fn with_cache_ttl(client: Client, ttl: Duration) -> Self {
        Self {
            client,
            cache: Arc::new(tokio::sync::RwLock::new(KevCache {
                entries: HashMap::new(),
                catalog_date: None,
                fetched_at: None,
            })),
            cache_ttl: ttl,
        }
    }

    /// Look up a CVE ID in the KEV catalog.
    /// Returns `Ok(Some(metadata))` if found, `Ok(None)` if not found.
    pub async fn lookup(&self, cve_id: &str) -> Result<Option<KevMetadata>, anyhow::Error> {
        let normalized = cve_id.to_uppercase();

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.entries.get(&normalized) {
                if cache
                    .fetched_at
                    .is_some_and(|t| t.elapsed() < self.cache_ttl)
                {
                    return Ok(Some(entry.metadata.clone()));
                }
            }
        }

        // Fetch fresh catalog if cache is stale
        self.fetch_catalog().await?;

        // Check cache again after fetch
        let cache = self.cache.read().await;
        Ok(cache
            .entries
            .get(&normalized)
            .map(|entry| entry.metadata.clone()))
    }

    /// Fetch the entire KEV catalog and populate the cache.
    async fn fetch_catalog(&self) -> Result<(), anyhow::Error> {
        let bytes = tokio::time::timeout(KEV_TIMEOUT, async {
            let resp = self.client.get(KEV_CATALOG_URL).send().await?;
            let status = resp.status();
            if !status.is_success() {
                return Err(anyhow::anyhow!(
                    "KEV catalog fetch failed with status: {status}"
                ));
            }
            if let Some(content_length) = resp.content_length() {
                if content_length as usize > MAX_BODY_BYTES {
                    return Err(anyhow::anyhow!(
                        "KEV catalog too large (Content-Length: {} bytes)",
                        content_length
                    ));
                }
            }
            let mut body = Vec::with_capacity(MAX_BODY_BYTES.min(64 * 1024));
            let mut stream = resp.bytes_stream();
            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result
                    .map_err(|e| anyhow::anyhow!("KEV catalog stream read error: {e}"))?;
                let remaining = MAX_BODY_BYTES.saturating_sub(body.len());
                if chunk.len() > remaining {
                    return Err(anyhow::anyhow!(
                        "KEV catalog too large: read {} bytes, limit is {MAX_BODY_BYTES} bytes",
                        body.len().max(chunk.len())
                    ));
                }
                body.extend_from_slice(&chunk);
            }
            Ok::<Vec<u8>, anyhow::Error>(body)
        })
        .await
        .map_err(|_| anyhow::anyhow!("KEV catalog fetch timed out"))??;

        let catalog: KeCatalog = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow::anyhow!("Failed to parse KEV catalog: {e}"))?;

        let mut entries = HashMap::new();
        for vuln in &catalog.vulnerabilities {
            let cve_id_upper = vuln.cve_id.to_uppercase();
            entries.insert(
                cve_id_upper.clone(),
                KevEntry {
                    cve_id: cve_id_upper,
                    metadata: KevMetadata {
                        vendor: vuln.vendor_project.clone(),
                        product: vuln.product.clone(),
                        required_action: vuln.required_action.clone(),
                        due_date: vuln.due_date.clone(),
                        known_ransomware_usage: vuln
                            .known_ransomware_campaign_use
                            .as_deref()
                            .map(|s| s.eq_ignore_ascii_case("Known"))
                            .unwrap_or(false),
                        catalog_date: vuln.date_added.clone(),
                    },
                },
            );
        }

        let mut cache = self.cache.write().await;
        cache.entries = entries;
        cache.catalog_date = catalog.date_released.clone();
        cache.fetched_at = Some(Instant::now());

        Ok(())
    }

    /// Check if the cache is populated and fresh.
    pub async fn is_cache_fresh(&self) -> bool {
        let cache = self.cache.read().await;
        cache
            .fetched_at
            .is_some_and(|t| t.elapsed() < self.cache_ttl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kev_metadata_fields() {
        let meta = KevMetadata {
            vendor: Some("TestVendor".to_string()),
            product: Some("TestProduct".to_string()),
            required_action: Some("Apply patch".to_string()),
            due_date: Some("2024-01-15".to_string()),
            known_ransomware_usage: true,
            catalog_date: Some("2024-01-01".to_string()),
        };
        assert_eq!(meta.vendor.as_deref(), Some("TestVendor"));
        assert_eq!(meta.product.as_deref(), Some("TestProduct"));
        assert!(meta.known_ransomware_usage);
    }

    #[test]
    fn kev_client_creation() {
        let client = Client::new();
        let kev = KevClient::new(client);
        assert_eq!(kev.cache_ttl, DEFAULT_CACHE_TTL);
    }

    #[test]
    fn kev_client_custom_ttl() {
        let client = Client::new();
        let kev = KevClient::with_cache_ttl(client, Duration::from_secs(300));
        assert_eq!(kev.cache_ttl, Duration::from_secs(300));
    }

    #[tokio::test]
    async fn kev_cache_starts_stale() {
        let client = Client::new();
        let kev = KevClient::new(client);
        assert!(!kev.is_cache_fresh().await);
    }
}
