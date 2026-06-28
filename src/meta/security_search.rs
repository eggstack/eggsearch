//! Security search orchestration for `security_search`.
//!
//! This module contains the core orchestration logic for the
//! `security_search` tool, extracted from `src/mcp/tools.rs`. It
//! coordinates web search, native advisory lookups, KEV enrichment,
//! result grouping, and suggested fetch generation.

use std::collections::HashSet;

use crate::core::query::SearchIntent;
use crate::core::security::{
    SecurityIdentifiers, SecuritySearchRequest, SecuritySearchResponse, VulnerabilityMetadata,
};
use crate::core::SearchWarning;
use crate::core::WebSearchRequest;
use crate::meta::engines::kev::KevClient;
use crate::meta::security_grouping::group_security_results;
use crate::meta::security_suggested_fetches::generate_security_suggested_fetches;
use crate::meta::MetadataSearchAdapter;

/// Orchestrate a security search: parse identifiers, run web search
/// with security intent, perform native advisory lookups, enrich with
/// KEV data, group results, and generate suggested fetches.
///
/// `effective_max` is the caller-computed max results (after config
/// cap). `max_results_cap` is the configured server cap used to bound
/// the candidate pool.
pub async fn run_security_search_plan(
    adapter: &MetadataSearchAdapter,
    kev_client: &KevClient,
    req: &SecuritySearchRequest,
    effective_max: usize,
    max_results_cap: usize,
) -> SecuritySearchResponse {
    // 1. Parse identifiers from request fields and free-text query
    let resolved_ids = SecurityIdentifiers::parse(
        &req.query,
        req.cve_id.as_deref(),
        req.ghsa_id.as_deref(),
        req.osv_id.as_deref(),
        req.rustsec_id.as_deref(),
        req.package.as_deref(),
        req.ecosystem.as_deref(),
        req.version.as_deref(),
    );

    // 2. Build web_search request with security intent for generic fallback
    let effective_providers = if req.providers.is_empty() {
        adapter.provider_ids().to_vec()
    } else {
        req.providers.clone()
    };

    let mut web_req = WebSearchRequest::new(req.query.clone());
    web_req.intent = SearchIntent::Security;
    web_req.freshness = req.freshness;
    web_req.max_results = Some(effective_max);
    web_req.timeout_ms = req.timeout_ms;
    web_req.providers = effective_providers.clone();

    let web_resp = adapter
        .web_search(&web_req, effective_max, max_results_cap)
        .await;

    // 3. Check if any native security provider (OSV) is available
    let has_native_advisory = effective_providers.iter().any(|id| id == "osv");

    let mut warnings: Vec<SearchWarning> = web_resp.warnings;

    if !has_native_advisory {
        warnings.push(SearchWarning::new(
            "_system",
            "native_advisory_search_unavailable: only generic web search was used; \
             enable the 'osv' provider for native advisory lookups",
        ));
    }

    // Generic context is external untrusted discussion, not advisory fact
    if !web_resp.results.is_empty() {
        warnings.push(SearchWarning::new(
            "_system",
            "generic_context_untrusted: generic web results are external untrusted \
             discussion, not authoritative advisory facts",
        ));
    }

    // Severity may be unavailable from generic search
    warnings.push(SearchWarning::new(
        "_system",
        "severity_unavailable: severity levels may not be available \
         from generic web search results; use native advisory providers for severity data",
    ));

    // 4. Native advisory ID lookups for identified CVE/GHSA/RustSec/OSV IDs
    let mut vulnerabilities: Vec<VulnerabilityMetadata> = Vec::new();
    let mut looked_up_ids: HashSet<String> = HashSet::new();

    for cve_id in &resolved_ids.cve_ids {
        if looked_up_ids.insert(cve_id.clone()) {
            if let Ok(Some(meta)) = adapter.lookup_advisory(cve_id).await {
                vulnerabilities.push(meta);
            }
        }
    }

    for ghsa_id in &resolved_ids.ghsa_ids {
        if looked_up_ids.insert(ghsa_id.clone()) {
            if let Ok(Some(meta)) = adapter.lookup_advisory(ghsa_id).await {
                vulnerabilities.push(meta);
            }
        }
    }

    for osv_id in &resolved_ids.osv_ids {
        if looked_up_ids.insert(osv_id.clone()) {
            if let Ok(Some(meta)) = adapter.lookup_advisory(osv_id).await {
                vulnerabilities.push(meta);
            }
        }
    }

    for rustsec_id in &resolved_ids.rustsec_ids {
        if looked_up_ids.insert(rustsec_id.clone()) {
            if let Ok(Some(meta)) = adapter.lookup_advisory(rustsec_id).await {
                vulnerabilities.push(meta);
            }
        }
    }

    // 5. Native OSV package query when both package and ecosystem are present
    if let (Some(ref package), Some(ref ecosystem)) =
        (&resolved_ids.package, &resolved_ids.ecosystem)
    {
        if has_native_advisory {
            let version = resolved_ids.version.as_deref();
            if let Ok(package_vulns) = adapter
                .query_advisories_by_package(ecosystem, package, version, effective_max)
                .await
            {
                for vuln in package_vulns {
                    if !vulnerabilities
                        .iter()
                        .any(|existing| ids_overlap(existing, &vuln))
                    {
                        vulnerabilities.push(vuln);
                    }
                }
            }
        }
    }

    // 6. Enrich vulnerabilities with KEV data if requested
    if req.include_kev == Some(true) {
        let cve_ids_for_kev: Vec<String> = vulnerabilities
            .iter()
            .flat_map(|v| v.cve_ids.iter().cloned())
            .collect();

        if cve_ids_for_kev.is_empty() {
            warnings.push(SearchWarning::new(
                "_system",
                "kev_lookup_skipped: KEV lookup requires CVE identifiers",
            ));
        } else {
            let mut kev_found_ids: Vec<String> = Vec::new();
            let mut kev_lookup_failed = false;

            for cve_id in &cve_ids_for_kev {
                match kev_client.lookup(cve_id).await {
                    Ok(Some(kev_meta)) => {
                        for vuln in &mut vulnerabilities {
                            if vuln.cve_ids.iter().any(|id| id == cve_id) {
                                vuln.kev = Some(kev_meta.clone());
                            }
                        }
                        kev_found_ids.push(cve_id.clone());
                    }
                    Ok(None) => {}
                    Err(_) => {
                        kev_lookup_failed = true;
                    }
                }
            }

            if kev_lookup_failed && kev_found_ids.is_empty() {
                warnings.push(SearchWarning::new(
                    "_system",
                    "kev_lookup_failed: KEV catalog lookup failed; KEV status could not be determined",
                ));
            } else if !kev_found_ids.is_empty() {
                warnings.push(SearchWarning::new(
                    "_system",
                    format!(
                        "kev_match: {} CVE(s) found in CISA KEV catalog",
                        kev_found_ids.len()
                    ),
                ));
            } else {
                warnings.push(SearchWarning::new(
                    "_system",
                    "kev_absent_not_proof: no CVE(s) found in CISA KEV catalog; \
                     absence does not prove no exploitation",
                ));
            }
        }
    }

    // 7. Warn about version matching limitations
    if req.version.is_some() {
        warnings.push(SearchWarning::new(
            "_system",
            "version_match_unavailable: version-specific matching is not yet implemented; \
             affected version ranges are returned as-is from advisory databases",
        ));
    }

    // 8. Group results and generate suggested fetches
    let groups = group_security_results(&web_resp.results, req.max_per_group);
    let suggested_fetches = generate_security_suggested_fetches(
        &groups,
        &resolved_ids,
        req.ecosystem.as_deref(),
        req.package.as_deref(),
    );

    SecuritySearchResponse {
        query: req.query.clone(),
        mode: "security_metasearch".to_string(),
        resolved_identifiers: resolved_ids,
        vulnerabilities,
        groups,
        suggested_fetches,
        providers_queried: web_resp.providers_queried,
        providers_failed: web_resp.providers_failed,
        warnings,
        trust_markers: web_resp.trust_markers,
    }
}

/// Check if two `VulnerabilityMetadata` records share any advisory IDs.
fn ids_overlap(a: &VulnerabilityMetadata, b: &VulnerabilityMetadata) -> bool {
    for id in &a.cve_ids {
        if b.cve_ids.contains(id) {
            return true;
        }
    }
    for id in &a.ghsa_ids {
        if b.ghsa_ids.contains(id) {
            return true;
        }
    }
    for id in &a.osv_ids {
        if b.osv_ids.contains(id) {
            return true;
        }
    }
    for id in &a.rustsec_ids {
        if b.rustsec_ids.contains(id) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::security::VulnerabilitySource;

    fn make_vuln(cve_id: &str) -> VulnerabilityMetadata {
        VulnerabilityMetadata {
            cve_ids: vec![cve_id.to_string()],
            source: VulnerabilitySource::Osv,
            ..Default::default()
        }
    }

    #[test]
    fn ids_overlap_same_cve() {
        let a = make_vuln("CVE-2024-0001");
        let b = make_vuln("CVE-2024-0001");
        assert!(ids_overlap(&a, &b));
    }

    #[test]
    fn ids_overlap_different_cve() {
        let a = make_vuln("CVE-2024-0001");
        let b = make_vuln("CVE-2024-0002");
        assert!(!ids_overlap(&a, &b));
    }

    #[test]
    fn ids_overlap_ghsa_match() {
        let a = VulnerabilityMetadata {
            ghsa_ids: vec!["GHSA-test-1234-abcd".to_string()],
            source: VulnerabilitySource::Osv,
            ..Default::default()
        };
        let b = VulnerabilityMetadata {
            ghsa_ids: vec!["GHSA-test-1234-abcd".to_string()],
            source: VulnerabilitySource::Osv,
            ..Default::default()
        };
        assert!(ids_overlap(&a, &b));
    }

    #[test]
    fn ids_overlap_cross_type() {
        let a = VulnerabilityMetadata {
            cve_ids: vec!["CVE-2024-0001".to_string()],
            source: VulnerabilitySource::GithubAdvisory,
            ..Default::default()
        };
        let b = VulnerabilityMetadata {
            ghsa_ids: vec!["GHSA-test-1234-abcd".to_string()],
            source: VulnerabilitySource::GithubAdvisory,
            ..Default::default()
        };
        assert!(!ids_overlap(&a, &b));
    }

    #[test]
    fn warning_prefix_native_advisory_search_unavailable() {
        // Verify the warning message format uses the stable prefix
        let msg = "native_advisory_search_unavailable: only generic web search was used; \
                    enable the 'osv' provider for native advisory lookups";
        assert!(
            msg.starts_with("native_advisory_search_unavailable:"),
            "native advisory warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_kev_match() {
        let msg = "kev_match: 2 CVE(s) found in CISA KEV catalog";
        assert!(
            msg.starts_with("kev_match:"),
            "kev_match warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_kev_absent_not_proof() {
        let msg = "kev_absent_not_proof: no CVE(s) found in CISA KEV catalog; \
                   absence does not prove no exploitation";
        assert!(
            msg.starts_with("kev_absent_not_proof:"),
            "kev_absent_not_proof warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_kev_lookup_failed() {
        let msg =
            "kev_lookup_failed: KEV catalog lookup failed; KEV status could not be determined";
        assert!(
            msg.starts_with("kev_lookup_failed:"),
            "kev_lookup_failed warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_kev_lookup_skipped() {
        let msg = "kev_lookup_skipped: KEV lookup requires CVE identifiers";
        assert!(
            msg.starts_with("kev_lookup_skipped:"),
            "kev_lookup_skipped warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_version_match_unavailable() {
        let msg = "version_match_unavailable: version-specific matching is not yet implemented; \
                   affected version ranges are returned as-is from advisory databases";
        assert!(
            msg.starts_with("version_match_unavailable:"),
            "version_match_unavailable warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_generic_context_untrusted() {
        let msg = "generic_context_untrusted: generic web results are external untrusted \
                   discussion, not authoritative advisory facts";
        assert!(
            msg.starts_with("generic_context_untrusted:"),
            "generic_context_untrusted warning must use stable prefix: {msg}"
        );
    }
}
