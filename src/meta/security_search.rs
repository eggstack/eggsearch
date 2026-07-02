//! Security search orchestration for `security_search`.
//!
//! This module contains the core orchestration logic for the
//! `security_search` tool, extracted from `src/mcp/tools.rs`. It
//! coordinates web search, native advisory lookups, KEV enrichment,
//! result grouping, and suggested fetch generation.

use std::collections::HashSet;

use crate::core::code_evidence::EvidenceConfidence;
use crate::core::security::{
    self, AffectedPackageSummary, SecurityContext, SecurityIdentifiers, SecuritySearchRequest,
    SecuritySearchResponse, VulnerabilityMetadata, VulnerabilitySummary,
};
use crate::core::SearchWarning;
use crate::meta::engines::kev::KevClient;
use crate::meta::response::WebSearchResponse;
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

    // 2. Run security search via parallel dispatcher
    let effective_providers = if req.providers.is_empty() {
        adapter.provider_ids().to_vec()
    } else {
        req.providers.clone()
    };

    let (results, dispatch_warnings, trust_markers) = adapter
        .security_search_subqueries(
            &req.query,
            &effective_providers,
            effective_max,
            max_results_cap,
            req.timeout_ms,
        )
        .await;

    // Build a WebSearchResponse-shaped structure for downstream compatibility
    let web_resp = WebSearchResponse {
        query: req.query.clone(),
        mode: "security_metasearch",
        results,
        providers_queried: effective_providers.clone(),
        providers_failed: Vec::new(),
        warnings: dispatch_warnings,
        trust_markers,
    };

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

    // 7. Version matching status
    if req.version.is_some() && req.assess_applicability != Some(true) {
        warnings.push(SearchWarning::new(
            "_system",
            "version_match_unavailable: version-specific matching requires assess_applicability=true; \
             affected version ranges are returned as-is from advisory databases",
        ));

        // Warn when package was found but no vulnerability has affected ranges
        if resolved_ids.package.is_some()
            && resolved_ids.ecosystem.is_some()
            && vulnerabilities
                .iter()
                .all(|v| v.affected_ranges.is_empty() && v.vulnerable_versions.is_empty())
        {
            warnings.push(SearchWarning::new(
                "_system",
                "version_mismatch: package was found but no advisory has affected version \
                 ranges matching the supplied version; the package may not be affected or \
                 version-specific advisory data is unavailable",
            ));
        }
    }

    // Applicability analysis
    let mut applicability_assessments = Vec::new();
    let mut dependency_findings = Vec::new();

    if req.assess_applicability == Some(true) {
        use crate::core::security_applicability::{
            ApplicabilityAssessment, ApplicabilityConfidence, ApplicabilityStatus,
        };
        use crate::meta::advisory_range::{assess_version_applicability, extract_advisory_ranges};
        use crate::meta::dependency_parse::parse_dependency_file;

        // Track (advisory_id, package, version) to deduplicate assessments
        let mut seen_assessments: std::collections::HashSet<(String, String, String)> =
            std::collections::HashSet::new();

        for file_path in &req.dependency_files {
            if let Ok(content) = std::fs::read_to_string(file_path) {
                let findings = parse_dependency_file(file_path, &content);
                dependency_findings.extend(findings);
            } else {
                warnings.push(SearchWarning::new(
                    "_system",
                    format!("dependency_file_read_error: could not read {file_path}"),
                ));
            }
        }

        let target_version = resolved_ids.version.as_deref();
        let target_package = resolved_ids.package.as_deref();
        let target_ecosystem = resolved_ids.ecosystem.as_deref();

        for vuln in &vulnerabilities {
            let ranges = extract_advisory_ranges(vuln);

            if let (Some(pkg), Some(ver)) = (target_package, target_version) {
                let vuln_pkg = vuln.package.as_deref().unwrap_or("");
                let vuln_eco = vuln.ecosystem.as_deref().unwrap_or("");

                let pkg_matches = vuln_pkg.eq_ignore_ascii_case(pkg);
                let eco_matches = target_ecosystem
                    .map(|e| e.eq_ignore_ascii_case(vuln_eco))
                    .unwrap_or(true);

                if pkg_matches && eco_matches {
                    let advisory_id = vuln
                        .cve_ids
                        .first()
                        .or(vuln.ghsa_ids.first())
                        .or(vuln.osv_ids.first())
                        .or(vuln.rustsec_ids.first())
                        .cloned()
                        .unwrap_or_default();

                    let outcome = assess_version_applicability(
                        ver,
                        &ranges,
                        &ranges
                            .first()
                            .map(|r| r.ecosystem.clone())
                            .unwrap_or(crate::core::package::PackageEcosystem::CratesIo),
                    );
                    let status = outcome.status;
                    let confidence = if !ranges.is_empty() {
                        ApplicabilityConfidence::High
                    } else {
                        ApplicabilityConfidence::Low
                    };

                    let mut assessment_reasons = outcome.reasons;
                    match status {
                        ApplicabilityStatus::Affected => assessment_reasons.push(format!(
                            "version {ver} appears affected by advisory {advisory_id}"
                        )),
                        ApplicabilityStatus::NotAffected => assessment_reasons.push(format!(
                            "version {ver} does not appear affected by advisory {advisory_id}"
                        )),
                        ApplicabilityStatus::Unknown => assessment_reasons.push(format!(
                            "could not determine applicability of version {ver} for advisory {advisory_id}"
                        )),
                    }

                    let key = (advisory_id.clone(), pkg.to_string(), ver.to_string());
                    if seen_assessments.insert(key) {
                        applicability_assessments.push(ApplicabilityAssessment {
                            status,
                            confidence,
                            ecosystem: vuln
                                .ecosystem
                                .as_deref()
                                .and_then(crate::core::package::PackageEcosystem::parse)
                                .unwrap_or(crate::core::package::PackageEcosystem::CratesIo),
                            package: pkg.to_string(),
                            version: Some(ver.to_string()),
                            advisory_ids: vec![advisory_id],
                            matched_ranges: outcome.matched_ranges,
                            reasons: assessment_reasons,
                            evidence_urls: vuln.references.iter().map(|r| r.url.clone()).collect(),
                            warnings: Vec::new(),
                        });
                    }
                }
            }

            for finding in &dependency_findings {
                let vuln_pkg = vuln.package.as_deref().unwrap_or("");
                let vuln_eco = vuln.ecosystem.as_deref().unwrap_or("");

                if finding.package.eq_ignore_ascii_case(vuln_pkg)
                    && finding.ecosystem.as_str().eq_ignore_ascii_case(vuln_eco)
                {
                    if let Some(ref ver) = finding.version {
                        let advisory_id = vuln
                            .cve_ids
                            .first()
                            .or(vuln.ghsa_ids.first())
                            .or(vuln.osv_ids.first())
                            .or(vuln.rustsec_ids.first())
                            .cloned()
                            .unwrap_or_default();

                        let outcome =
                            assess_version_applicability(ver, &ranges, &finding.ecosystem);
                        let status = outcome.status;
                        let confidence = if !ranges.is_empty() {
                            ApplicabilityConfidence::High
                        } else {
                            ApplicabilityConfidence::Low
                        };

                        let mut reasons = outcome.reasons;
                        reasons.push(format!(
                            "dependency '{}' version '{}' found in {}",
                            finding.package,
                            ver,
                            finding.source_file.as_deref().unwrap_or("unknown")
                        ));

                        let key = (advisory_id.clone(), finding.package.clone(), ver.clone());
                        if seen_assessments.insert(key) {
                            applicability_assessments.push(ApplicabilityAssessment {
                                status,
                                confidence,
                                ecosystem: finding.ecosystem.clone(),
                                package: finding.package.clone(),
                                version: Some(ver.clone()),
                                advisory_ids: vec![advisory_id],
                                matched_ranges: outcome.matched_ranges,
                                reasons,
                                evidence_urls: vuln
                                    .references
                                    .iter()
                                    .map(|r| r.url.clone())
                                    .collect(),
                                warnings: Vec::new(),
                            });
                        }
                    }
                }
            }
        }

        if !applicability_assessments.is_empty() {
            warnings.push(SearchWarning::new(
                "_system",
                "applicability_not_exploitability: Advisory range matching does not determine \
                 runtime exploitability or reachability. Applicability assessments are based on \
                 advisory metadata and dependency file parsing, not runtime analysis.",
            ));
        }
    }

    // 8. Group results and generate suggested fetches
    let groups = group_security_results(&web_resp.results, req.max_per_group);
    let suggested_fetches = generate_security_suggested_fetches(
        &groups,
        &resolved_ids,
        req.ecosystem.as_deref(),
        req.package.as_deref(),
        &dependency_findings,
    );

    // 9. Build security context
    let query_kind = security::classify_query_kind(&resolved_ids);
    let identifiers = security::build_identifier_list(&resolved_ids);
    let mut source_quality = security::assess_source_quality(&web_resp.results);

    // Annotate when version hint is present and vulnerabilities have affected ranges
    if resolved_ids.version.is_some()
        && vulnerabilities
            .iter()
            .any(|v| !v.affected_ranges.is_empty())
    {
        source_quality.tier_reasons.push(
            "version_affected_match: query includes version hint and advisory has affected ranges"
                .to_string(),
        );
    }

    // Build affected package summaries from vulnerability metadata
    let affected_packages: Vec<AffectedPackageSummary> = {
        let mut seen = std::collections::HashSet::new();
        let mut packages = Vec::new();
        for vuln in &vulnerabilities {
            if let (Some(ref pkg), Some(ref eco)) = (&vuln.package, &vuln.ecosystem) {
                let key = format!("{eco}:{pkg}");
                if seen.insert(key) {
                    packages.push(AffectedPackageSummary {
                        package: pkg.clone(),
                        ecosystem: eco.clone(),
                        affected_ranges: vuln.affected_ranges.clone(),
                        patched_versions: vuln.patched_versions.clone(),
                    });
                }
            }
        }
        packages
    };

    // Build vulnerability summaries
    let vulnerability_summaries: Vec<VulnerabilitySummary> = vulnerabilities
        .iter()
        .map(|vuln| {
            let id = vuln
                .cve_ids
                .first()
                .or(vuln.ghsa_ids.first())
                .or(vuln.osv_ids.first())
                .or(vuln.rustsec_ids.first())
                .cloned()
                .unwrap_or_default();
            VulnerabilitySummary {
                id,
                severity: vuln.severity,
                description: None,
                source: vuln.source,
                kev: vuln.kev.is_some(),
            }
        })
        .collect();

    // Build defensive guidance from grouping results
    let mut defensive_guidance = Vec::new();
    for group in &groups {
        if group.kind == crate::core::security::SecurityResultGroupKind::DefensiveGuidance {
            for card in &group.results {
                defensive_guidance.push(security::DefensiveGuidance {
                    category: security::DefensiveGuidanceCategory::Unknown,
                    summary: card.title.clone(),
                    source_urls: vec![card.url.clone()],
                    confidence: EvidenceConfidence::Weak,
                });
            }
        }
    }

    // Build context warnings
    let mut context_warnings = Vec::new();
    if vulnerabilities.is_empty() && !resolved_ids.has_strong_identifier() {
        context_warnings.push(
            "no native vulnerability data found; results are generic web search only".to_string(),
        );
    }
    if source_quality.tier == security::SecuritySourceTier::Unknown
        || matches!(
            source_quality.tier,
            security::SecuritySourceTier::NewsOrBlog
                | security::SecuritySourceTier::CommunityDiscussion
        )
    {
        context_warnings.push(format!(
            "source quality is low ({}); advisory authority may be limited",
            source_quality.tier.as_str()
        ));
    }

    let security_context = SecurityContext {
        query_kind,
        identifiers,
        affected_packages,
        vulnerability_summaries,
        defensive_guidance,
        source_quality,
        warnings: context_warnings,
    };

    SecuritySearchResponse {
        query: req.query.clone(),
        mode: "security_metasearch".to_string(),
        resolved_identifiers: resolved_ids,
        vulnerabilities,
        security_context: Some(security_context),
        groups,
        suggested_fetches,
        providers_queried: web_resp.providers_queried,
        providers_failed: web_resp.providers_failed,
        warnings,
        trust_markers: web_resp.trust_markers,
        capability_enforcement: Some(
            crate::meta::provider_diagnostics::CapabilityEnforcementTelemetry::for_security_search(
                req,
                &effective_providers,
            ),
        ),
        routing_decision: None,
        applicability: applicability_assessments,
        dependency_findings,
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
        let msg = "version_match_unavailable: version-specific matching requires assess_applicability=true; \
                   affected version ranges are returned as-is from advisory databases";
        assert!(
            msg.starts_with("version_match_unavailable:"),
            "version_match_unavailable warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_version_mismatch() {
        let msg = "version_mismatch: package was found but no advisory has affected version \
                   ranges matching the supplied version; the package may not be affected or \
                   version-specific advisory data is unavailable";
        assert!(
            msg.starts_with("version_mismatch:"),
            "version_mismatch warning must use stable prefix: {msg}"
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

    #[test]
    fn warning_prefix_applicability_not_exploitability() {
        let msg = "applicability_not_exploitability: Advisory range matching does not determine \
                   runtime exploitability or reachability. Applicability assessments are based on \
                   advisory metadata and dependency file parsing, not runtime analysis.";
        assert!(
            msg.starts_with("applicability_not_exploitability:"),
            "applicability warning must use stable prefix: {msg}"
        );
    }

    #[test]
    fn warning_prefix_dependency_file_read_error() {
        let msg = "dependency_file_read_error: could not read /nonexistent/Cargo.lock";
        assert!(
            msg.starts_with("dependency_file_read_error:"),
            "dependency_file_read_error warning must use stable prefix: {msg}"
        );
    }
}
