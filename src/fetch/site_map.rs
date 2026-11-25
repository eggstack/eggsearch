use std::collections::HashSet;
use std::time::Duration;

use quick_xml::events::Event;
use serde::Serialize;
use url::Url;

use crate::core::batch_fetch::SiteQueryNormalization;
use crate::fetch::client::FetchClient;
use crate::fetch::types::FetchError;

pub(crate) const DEFAULT_SITE_MAP_LIMIT: usize = 200;
pub(crate) const MAX_SITE_MAP_LIMIT: usize = 2_000;
pub(crate) const MAX_SITE_MAP_BYTES: usize = 64 * 1024;
pub(crate) const MAX_SITE_MAP_READS: usize = 8;
pub(crate) const MAX_SITE_MAP_XML_DEPTH: usize = 32;
pub(crate) const SITE_MAP_DISCOVERY_TIMEOUT_MS: u64 = 5_000;
const MAX_SITE_MAP_URL_CHARS: usize = 8_192;
const MAX_LASTMOD_CHARS: usize = 200;
pub(crate) struct SiteMapRequest {
    pub url: String,
    pub limit: Option<usize>,
    pub include_subdomains: bool,
    pub link_discovery: bool,
    pub normalize_query: Option<SiteQueryNormalization>,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SiteMapResponse {
    pub origin: String,
    pub urls: Vec<SiteMapUrl>,
    pub counts: SiteMapCounts,
    pub sitemap_index_used: bool,
    pub robots_sitemaps: usize,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SiteMapUrl {
    pub url: String,
    pub source: SiteMapSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastmod: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SiteMapSource {
    Sitemap,
    Link,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SiteMapCounts {
    pub sitemap: usize,
    pub link: usize,
    pub cross_origin_skipped: usize,
    pub duplicates: usize,
    pub total: usize,
}

#[derive(Debug)]
pub(crate) struct SiteMapError {
    pub code: &'static str,
    pub detail: String,
}

impl std::fmt::Display for SiteMapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

#[derive(Clone)]
struct Candidate {
    url: Url,
    raw_url: String,
    source: SiteMapSource,
    lastmod: Option<String>,
}

struct Inventory {
    limit: usize,
    seen: HashSet<String>,
    candidates: Vec<Candidate>,
    sitemap_count: usize,
    link_count: usize,
    cross_origin_skipped: usize,
    duplicates: usize,
    sitemap_index_used: bool,
    robots_sitemaps: usize,
    truncated: bool,
}

#[derive(Debug)]
struct SitemapDocument {
    pages: Vec<(String, Option<String>)>,
    sitemaps: Vec<String>,
    is_index: bool,
    truncated: bool,
}

enum CandidateResult {
    Allowed(Url, String),
    CrossOrigin,
    Invalid,
}

enum EntryKind {
    Page,
    Sitemap,
}

pub(crate) async fn discover(
    client: &FetchClient,
    request: SiteMapRequest,
) -> Result<SiteMapResponse, SiteMapError> {
    let site_url = Url::parse(&request.url).map_err(|_| {
        site_map_error(
            "site_map_invalid_url",
            "entry URL is not a valid HTTP(S) URL",
        )
    })?;
    if !matches!(site_url.scheme(), "http" | "https")
        || site_url.host_str().is_none()
        || !site_url.username().is_empty()
        || site_url.password().is_some()
    {
        return Err(site_map_error(
            "site_map_invalid_url",
            "entry URL is not a valid HTTP(S) URL",
        ));
    }
    let limit = request.limit.unwrap_or(DEFAULT_SITE_MAP_LIMIT);
    if limit == 0 || limit > MAX_SITE_MAP_LIMIT {
        return Err(site_map_error(
            "site_map_invalid_limit",
            "limit must be between 1 and 2000",
        ));
    }
    let origin_url = site_origin(&site_url);
    let origin = origin_url.origin().ascii_serialization();
    let mut inventory = Inventory {
        limit,
        seen: HashSet::new(),
        candidates: Vec::new(),
        sitemap_count: 0,
        link_count: 0,
        cross_origin_skipped: 0,
        duplicates: 0,
        sitemap_index_used: false,
        robots_sitemaps: 0,
        truncated: false,
    };
    let timeout_ms = request.timeout_ms.clamp(1, SITE_MAP_DISCOVERY_TIMEOUT_MS);
    match tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        discover_bounded(client, &request, &site_url, &origin_url, &mut inventory),
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => inventory.truncated = true,
    }

    inventory.candidates.sort_by(|left, right| {
        left.url
            .host_str()
            .cmp(&right.url.host_str())
            .then_with(|| left.url.path().cmp(right.url.path()))
            .then_with(|| left.url.query().cmp(&right.url.query()))
            .then_with(|| left.raw_url.cmp(&right.raw_url))
    });
    let urls = inventory
        .candidates
        .into_iter()
        .filter_map(|candidate| {
            let url = sanitize_text(candidate.url.as_str(), MAX_SITE_MAP_URL_CHARS)?;
            Some(SiteMapUrl {
                url,
                source: candidate.source,
                lastmod: candidate
                    .lastmod
                    .and_then(|value| sanitize_text(&value, MAX_LASTMOD_CHARS)),
            })
        })
        .collect::<Vec<_>>();
    let total = urls.len();
    Ok(SiteMapResponse {
        origin,
        counts: SiteMapCounts {
            sitemap: inventory.sitemap_count,
            link: inventory.link_count,
            cross_origin_skipped: inventory.cross_origin_skipped,
            duplicates: inventory.duplicates,
            total,
        },
        urls,
        sitemap_index_used: inventory.sitemap_index_used,
        robots_sitemaps: inventory.robots_sitemaps,
        truncated: inventory.truncated,
    })
}

async fn discover_bounded(
    client: &FetchClient,
    request: &SiteMapRequest,
    site_url: &Url,
    origin_url: &Url,
    inventory: &mut Inventory,
) -> Result<(), SiteMapError> {
    client
        .validate_site_resource_target(&request.url)
        .await
        .map_err(site_map_fetch_error)?;
    let robots_url = origin_url
        .join("/robots.txt")
        .map_err(|_| site_map_error("site_map_invalid_url", "robots URL is invalid"))?;
    let robots = client
        .fetch_bounded_site_resource(robots_url.as_str(), MAX_SITE_MAP_BYTES, origin_url)
        .await
        .map_err(site_map_fetch_error)?;
    inventory.truncated |= robots.truncated;
    let mut sitemap_urls = Vec::new();
    let mut sitemap_seen = HashSet::new();
    if (200..300).contains(&robots.status) {
        if is_compressed(&robots.content_encoding) {
            return Err(site_map_error(
                "site_map_gzip_unsupported",
                "compressed robots and sitemap responses are not supported",
            ));
        }
        let body = sanitize_body(&robots.body);
        for line in body.lines() {
            let line = line.trim();
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            if !name.trim().eq_ignore_ascii_case("sitemap") || value.trim().is_empty() {
                continue;
            }
            inventory.robots_sitemaps = inventory.robots_sitemaps.saturating_add(1);
            match normalize_candidate(
                value.trim(),
                origin_url,
                site_url,
                request.include_subdomains,
                query_policy(request.normalize_query),
            ) {
                CandidateResult::Allowed(url, _) => {
                    let key = url.to_string();
                    if sitemap_seen.insert(key) {
                        sitemap_urls.push((url, 0usize));
                    }
                }
                CandidateResult::CrossOrigin => {
                    inventory.cross_origin_skipped =
                        inventory.cross_origin_skipped.saturating_add(1);
                }
                CandidateResult::Invalid => {}
            }
        }
    } else if robots.status != 404 && robots.status != 410 {
        return Err(site_map_status_error(robots.status));
    }

    if inventory.robots_sitemaps == 0 {
        for path in ["/sitemap.xml", "/sitemap_index.xml"] {
            let url = origin_url.join(path).map_err(|_| {
                site_map_error("site_map_invalid_url", "fallback sitemap URL is invalid")
            })?;
            let key = url.to_string();
            if sitemap_seen.insert(key) {
                sitemap_urls.push((url, 0usize));
            }
        }
    }

    let mut sitemap_reads = 0usize;
    let mut child_sitemaps = Vec::new();
    let mut processed_sitemaps = HashSet::new();
    for (sitemap_url, depth) in sitemap_urls {
        if sitemap_reads >= MAX_SITE_MAP_READS {
            inventory.truncated = true;
            break;
        }
        if inventory.candidates.len() >= inventory.limit {
            inventory.truncated = true;
            break;
        }
        sitemap_reads += 1;
        let key = sitemap_url.to_string();
        if !processed_sitemaps.insert(key) {
            continue;
        }
        let Some(document) = read_sitemap(client, &sitemap_url, origin_url, inventory).await?
        else {
            continue;
        };
        inventory.sitemap_index_used |= document.is_index;
        inventory.truncated |= document.truncated;
        for (raw_url, lastmod) in document.pages {
            match normalize_candidate(
                &raw_url,
                &sitemap_url,
                site_url,
                request.include_subdomains,
                query_policy(request.normalize_query),
            ) {
                CandidateResult::Allowed(url, raw) => {
                    add_candidate(inventory, url, raw, SiteMapSource::Sitemap, lastmod);
                }
                CandidateResult::CrossOrigin => {
                    inventory.cross_origin_skipped =
                        inventory.cross_origin_skipped.saturating_add(1);
                }
                CandidateResult::Invalid => {}
            }
        }
        for raw_sitemap in document.sitemaps {
            match normalize_candidate(
                &raw_sitemap,
                &sitemap_url,
                site_url,
                request.include_subdomains,
                query_policy(request.normalize_query),
            ) {
                CandidateResult::Allowed(url, _) => {
                    let key = url.to_string();
                    if sitemap_seen.insert(key) {
                        if depth == 0 {
                            child_sitemaps.push(url);
                        } else {
                            inventory.truncated = true;
                        }
                    }
                }
                CandidateResult::CrossOrigin => {
                    inventory.cross_origin_skipped =
                        inventory.cross_origin_skipped.saturating_add(1);
                }
                CandidateResult::Invalid => {}
            }
        }
    }

    for sitemap_url in child_sitemaps {
        if sitemap_reads >= MAX_SITE_MAP_READS {
            inventory.truncated = true;
            break;
        }
        if inventory.candidates.len() >= inventory.limit {
            inventory.truncated = true;
            break;
        }
        sitemap_reads += 1;
        let key = sitemap_url.to_string();
        if !processed_sitemaps.insert(key) {
            continue;
        }
        let Some(document) = read_sitemap(client, &sitemap_url, origin_url, inventory).await?
        else {
            continue;
        };
        inventory.sitemap_index_used |= document.is_index;
        inventory.truncated |= document.truncated;
        for (raw_url, lastmod) in document.pages {
            match normalize_candidate(
                &raw_url,
                &sitemap_url,
                site_url,
                request.include_subdomains,
                query_policy(request.normalize_query),
            ) {
                CandidateResult::Allowed(url, raw) => {
                    add_candidate(inventory, url, raw, SiteMapSource::Sitemap, lastmod);
                }
                CandidateResult::CrossOrigin => {
                    inventory.cross_origin_skipped =
                        inventory.cross_origin_skipped.saturating_add(1);
                }
                CandidateResult::Invalid => {}
            }
        }
        if !document.sitemaps.is_empty() {
            inventory.truncated = true;
        }
    }

    if request.link_discovery {
        if inventory.candidates.len() >= inventory.limit {
            inventory.truncated = true;
        } else {
            let page = client
                .fetch_bounded_site_resource(&request.url, MAX_SITE_MAP_BYTES, origin_url)
                .await
                .map_err(site_map_fetch_error)?;
            inventory.truncated |= page.truncated;
            if !(200..300).contains(&page.status) {
                return Err(site_map_status_error(page.status));
            }
            if is_html(&page.content_type, &page.body) {
                let extracted =
                    crate::fetch::extract::extract_links_from_html(&page.body, &page.final_url);
                inventory.truncated |= extracted.truncated;
                for link in extracted.links {
                    match normalize_candidate(
                        &link.url,
                        &page.final_url.parse().unwrap_or_else(|_| site_url.clone()),
                        site_url,
                        request.include_subdomains,
                        query_policy(request.normalize_query),
                    ) {
                        CandidateResult::Allowed(url, raw) => {
                            add_candidate(inventory, url, raw, SiteMapSource::Link, None);
                        }
                        CandidateResult::CrossOrigin => {
                            inventory.cross_origin_skipped =
                                inventory.cross_origin_skipped.saturating_add(1);
                        }
                        CandidateResult::Invalid => {}
                    }
                }
            }
        }
    }
    Ok(())
}

async fn read_sitemap(
    client: &FetchClient,
    url: &Url,
    origin_url: &Url,
    inventory: &mut Inventory,
) -> Result<Option<SitemapDocument>, SiteMapError> {
    if url.path().to_ascii_lowercase().ends_with(".gz") {
        return Err(site_map_error(
            "site_map_gzip_unsupported",
            "compressed sitemap files are not supported",
        ));
    }
    let response = client
        .fetch_bounded_site_resource(url.as_str(), MAX_SITE_MAP_BYTES, origin_url)
        .await
        .map_err(site_map_fetch_error)?;
    inventory.truncated |= response.truncated;
    if response.status == 404 || response.status == 410 {
        return Ok(None);
    }
    if !(200..300).contains(&response.status) {
        return Err(site_map_status_error(response.status));
    }
    if is_compressed(&response.content_encoding) {
        return Err(site_map_error(
            "site_map_gzip_unsupported",
            "compressed sitemap files are not supported",
        ));
    }
    parse_sitemap(&response.body, response.truncated).map(Some)
}

fn parse_sitemap(body: &[u8], body_truncated: bool) -> Result<SitemapDocument, SiteMapError> {
    let mut reader = quick_xml::Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut root: Option<String> = None;
    let mut entry: Option<(EntryKind, Option<String>, Option<String>)> = None;
    let mut field: Option<(String, String)> = None;
    let mut document = SitemapDocument {
        pages: Vec::new(),
        sitemaps: Vec::new(),
        is_index: false,
        truncated: false,
    };
    loop {
        let event = match reader.read_event_into(&mut buffer) {
            Ok(event) => event,
            Err(_) if body_truncated => {
                document.truncated = true;
                break;
            }
            Err(_) => {
                return Err(site_map_error(
                    "site_map_malformed",
                    "sitemap XML is malformed",
                ))
            }
        };
        match event {
            Event::Start(element) => {
                let name = xml_local_name(element.local_name().as_ref())?;
                if stack.len() >= MAX_SITE_MAP_XML_DEPTH {
                    document.truncated = true;
                    break;
                }
                if stack.is_empty() {
                    root = Some(name.clone());
                    if name == "sitemapindex" {
                        document.is_index = true;
                    } else if name != "urlset" {
                        return Err(site_map_error(
                            "site_map_malformed",
                            "sitemap XML has an unsupported root element",
                        ));
                    }
                }
                stack.push(name.clone());
                match (root.as_deref(), name.as_str()) {
                    (Some("urlset"), "url") => entry = Some((EntryKind::Page, None, None)),
                    (Some("sitemapindex"), "sitemap") => {
                        entry = Some((EntryKind::Sitemap, None, None))
                    }
                    _ => {}
                }
                if entry.is_some() && (name == "loc" || name == "lastmod") {
                    field = Some((name, String::new()));
                }
            }
            Event::Empty(element) => {
                if stack.is_empty() {
                    let name = xml_local_name(element.local_name().as_ref())?;
                    root = Some(name.clone());
                    if name == "sitemapindex" {
                        document.is_index = true;
                    } else if name != "urlset" {
                        return Err(site_map_error(
                            "site_map_malformed",
                            "sitemap XML has an unsupported root element",
                        ));
                    }
                }
            }
            Event::Text(text) => {
                if let Some((_, value)) = field.as_mut() {
                    let decoded = text.decode().map_err(|_| {
                        site_map_error("site_map_malformed", "sitemap XML is malformed")
                    })?;
                    let unescaped = quick_xml::escape::unescape(&decoded).map_err(|_| {
                        site_map_error("site_map_malformed", "sitemap XML is malformed")
                    })?;
                    value.push_str(&unescaped);
                }
            }
            Event::CData(text) => {
                if let Some((_, value)) = field.as_mut() {
                    value.push_str(&text.decode().map_err(|_| {
                        site_map_error("site_map_malformed", "sitemap XML is malformed")
                    })?);
                }
            }
            Event::GeneralRef(reference) => {
                if let Some((_, value)) = field.as_mut() {
                    if let Some(character) = reference.resolve_char_ref().map_err(|_| {
                        site_map_error("site_map_malformed", "sitemap XML is malformed")
                    })? {
                        value.push(character);
                    } else {
                        let name = reference.decode().map_err(|_| {
                            site_map_error("site_map_malformed", "sitemap XML is malformed")
                        })?;
                        let character = match name.as_ref() {
                            "amp" => '&',
                            "lt" => '<',
                            "gt" => '>',
                            "quot" => '"',
                            "apos" => '\'',
                            _ => {
                                return Err(site_map_error(
                                    "site_map_malformed",
                                    "sitemap XML contains an unsupported entity",
                                ));
                            }
                        };
                        value.push(character);
                    }
                }
            }
            Event::End(element) => {
                let name = xml_local_name(element.local_name().as_ref())?;
                if let Some((field_name, value)) = field.take() {
                    if field_name == name {
                        let value = value.trim().to_string();
                        if let Some((kind, loc, lastmod)) = entry.as_mut() {
                            if field_name == "loc" && loc.is_none() {
                                *loc = Some(value);
                            } else if field_name == "lastmod"
                                && matches!(kind, EntryKind::Page)
                                && lastmod.is_none()
                            {
                                *lastmod = Some(value);
                            }
                        }
                    } else {
                        field = Some((field_name, value));
                    }
                }
                if let Some((kind, loc, lastmod)) = entry.as_ref() {
                    let is_entry_end = matches!(
                        (kind, name.as_str()),
                        (EntryKind::Page, "url") | (EntryKind::Sitemap, "sitemap")
                    );
                    if is_entry_end {
                        if let Some(loc) = loc.as_ref().filter(|loc| !loc.is_empty()) {
                            match kind {
                                EntryKind::Page => document.pages.push((
                                    loc.clone(),
                                    lastmod.clone().filter(|value| !value.is_empty()),
                                )),
                                EntryKind::Sitemap => document.sitemaps.push(loc.clone()),
                            }
                        }
                        entry = None;
                    }
                }
                if stack.pop().as_deref() != Some(name.as_str()) {
                    return Err(site_map_error(
                        "site_map_malformed",
                        "sitemap XML is malformed",
                    ));
                }
            }
            Event::DocType(_) => {
                return Err(site_map_error(
                    "site_map_malformed",
                    "DTD and entity declarations are not supported",
                ));
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if root.is_none() {
        if body_truncated {
            document.truncated = true;
            return Ok(document);
        }
        return Err(site_map_error("site_map_malformed", "sitemap XML is empty"));
    }
    if !stack.is_empty() && !body_truncated && !document.truncated {
        return Err(site_map_error(
            "site_map_malformed",
            "sitemap XML is incomplete",
        ));
    }
    document.truncated |= body_truncated;
    Ok(document)
}

fn add_candidate(
    inventory: &mut Inventory,
    url: Url,
    raw_url: String,
    source: SiteMapSource,
    lastmod: Option<String>,
) {
    let key = url.to_string();
    if !inventory.seen.insert(key) {
        inventory.duplicates = inventory.duplicates.saturating_add(1);
        return;
    }
    if inventory.candidates.len() >= inventory.limit {
        inventory.truncated = true;
        return;
    }
    match source {
        SiteMapSource::Sitemap => {
            inventory.sitemap_count = inventory.sitemap_count.saturating_add(1)
        }
        SiteMapSource::Link => inventory.link_count = inventory.link_count.saturating_add(1),
    }
    inventory.candidates.push(Candidate {
        url,
        raw_url,
        source,
        lastmod,
    });
}

fn normalize_candidate(
    raw: &str,
    base: &Url,
    site_url: &Url,
    include_subdomains: bool,
    normalize_query: SiteQueryNormalization,
) -> CandidateResult {
    let Some(raw) = sanitize_text(raw.trim(), MAX_SITE_MAP_URL_CHARS) else {
        return CandidateResult::Invalid;
    };
    let mut url = match base.join(&raw) {
        Ok(url) => url,
        Err(_) => return CandidateResult::Invalid,
    };
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return CandidateResult::Invalid;
    }
    if !same_site(
        url.host_str().unwrap_or_default(),
        site_url.host_str().unwrap_or_default(),
        include_subdomains,
    ) || url.scheme() != site_url.scheme()
        || url.port_or_known_default() != site_url.port_or_known_default()
    {
        return CandidateResult::CrossOrigin;
    }
    let raw_url = url.to_string();
    let normalized_path = normalize_percent_encoding(url.path());
    url.set_path(&normalized_path);
    url.set_fragment(None);
    if normalize_query == SiteQueryNormalization::Drop {
        url.set_query(None);
    }
    if let Some(port) = url.port() {
        let default_port = match url.scheme() {
            "http" => Some(80),
            "https" => Some(443),
            _ => None,
        };
        if Some(port) == default_port {
            let _ = url.set_port(None);
        }
    }
    if url.as_str().chars().count() > MAX_SITE_MAP_URL_CHARS {
        return CandidateResult::Invalid;
    }
    CandidateResult::Allowed(url, raw_url)
}

fn same_site(candidate: &str, origin: &str, include_subdomains: bool) -> bool {
    if candidate.eq_ignore_ascii_case(origin) {
        return true;
    }
    if !include_subdomains {
        return false;
    }
    if origin.is_empty()
        || origin.starts_with('[')
        || candidate.starts_with('[')
        || origin.parse::<std::net::IpAddr>().is_ok()
        || candidate.parse::<std::net::IpAddr>().is_ok()
    {
        return false;
    }
    let candidate = candidate.to_ascii_lowercase();
    let suffix = format!(".{}", origin.to_ascii_lowercase());
    candidate.len() > suffix.len() && candidate.ends_with(&suffix)
}

fn normalize_percent_encoding(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut output = String::with_capacity(path.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                let value = high * 16 + low;
                if is_unreserved(value) {
                    output.push(char::from(value));
                } else {
                    output.push('%');
                    output.push(char::from(b"0123456789ABCDEF"[(value >> 4) as usize]));
                    output.push(char::from(b"0123456789ABCDEF"[(value & 15) as usize]));
                }
                index += 3;
                continue;
            }
        }
        output.push(char::from(bytes[index]));
        index += 1;
    }
    output
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn is_unreserved(value: u8) -> bool {
    value.is_ascii_alphanumeric() || matches!(value, b'-' | b'.' | b'_' | b'~')
}

fn query_policy(value: Option<SiteQueryNormalization>) -> SiteQueryNormalization {
    value.unwrap_or_default()
}

fn site_origin(url: &Url) -> Url {
    let mut origin = url.clone();
    origin.set_path("/");
    origin.set_query(None);
    origin.set_fragment(None);
    origin
}

fn is_compressed(value: &Option<String>) -> bool {
    value.as_deref().is_some_and(|encoding| {
        encoding
            .split(',')
            .any(|part| !matches!(part.trim().to_ascii_lowercase().as_str(), "" | "identity"))
    })
}

fn is_html(content_type: &Option<String>, body: &[u8]) -> bool {
    if content_type.as_deref().is_some_and(|value| {
        let media_type = value.split(';').next().unwrap_or_default().trim();
        media_type.eq_ignore_ascii_case("text/html")
            || media_type.eq_ignore_ascii_case("application/xhtml+xml")
    }) {
        return true;
    }
    let prefix = String::from_utf8_lossy(&body[..body.len().min(512)]).to_ascii_lowercase();
    prefix.contains("<html") || prefix.contains("<!doctype html")
}

fn sanitize_body(body: &[u8]) -> String {
    let decoded = String::from_utf8_lossy(body);
    let (stripped, _) = crate::core::sanitize::strip_control_chars(&decoded);
    let (bounded, _) = crate::core::sanitize::bound_text(&stripped, MAX_SITE_MAP_BYTES);
    bounded
}

fn sanitize_text(value: &str, max_chars: usize) -> Option<String> {
    let (stripped, _) = crate::core::sanitize::strip_control_chars(value);
    let (bounded, truncated) = crate::core::sanitize::bound_text(&stripped, max_chars);
    if truncated || bounded.trim().is_empty() {
        None
    } else {
        Some(bounded.trim().to_string())
    }
}

fn xml_local_name(value: &[u8]) -> Result<String, SiteMapError> {
    std::str::from_utf8(value)
        .map(|value| value.to_ascii_lowercase())
        .map_err(|_| site_map_error("site_map_malformed", "sitemap XML is malformed"))
}

fn site_map_fetch_error(error: FetchError) -> SiteMapError {
    if matches!(&error, FetchError::NetworkError(message) if message == "site_map_cross_origin_redirect")
    {
        return site_map_error(
            "site_map_cross_origin_redirect",
            "cross-origin redirects are not followed",
        );
    }
    site_map_error("site_map_fetch_failed", error.error_code())
}

fn site_map_status_error(status: u16) -> SiteMapError {
    site_map_error("site_map_http_status", &format!("HTTP status {status}"))
}

fn site_map_error(code: &'static str, detail: &str) -> SiteMapError {
    let (detail, _) = crate::core::sanitize::strip_control_chars(detail);
    let (detail, _) = crate::core::sanitize::bound_text(&detail, 160);
    SiteMapError { code, detail }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sitemap_xml_extracts_urls_lastmod_and_index_children() {
        let urlset = br#"<urlset><url><loc>https://example.com/a</loc><lastmod>2026-01-02</lastmod></url></urlset>"#;
        let parsed = parse_sitemap(urlset, false).expect("valid urlset");
        assert_eq!(
            parsed.pages,
            vec![("https://example.com/a".into(), Some("2026-01-02".into()))]
        );
        assert!(!parsed.is_index);

        let index = br#"<sitemapindex><sitemap><loc>https://example.com/a.xml</loc></sitemap></sitemapindex>"#;
        let parsed = parse_sitemap(index, false).expect("valid sitemap index");
        assert_eq!(parsed.sitemaps, vec!["https://example.com/a.xml"]);
        assert!(parsed.is_index);
    }

    #[test]
    fn malformed_and_external_entity_xml_are_rejected() {
        assert_eq!(
            parse_sitemap(b"<urlset><url>", false).unwrap_err().code,
            "site_map_malformed"
        );
        assert_eq!(parse_sitemap(br#"<!DOCTYPE urlset [<!ENTITY x SYSTEM "file:///etc/passwd">]><urlset>&x;</urlset>"#, false).unwrap_err().code, "site_map_malformed");
    }

    #[test]
    fn candidate_normalization_drops_fragments_and_normalizes_paths() {
        let origin = Url::parse("https://Example.com:443/docs/").unwrap();
        let candidate = normalize_candidate(
            "../%7edocs/a%2fb?x=1#section",
            &origin,
            &origin,
            false,
            SiteQueryNormalization::Keep,
        );
        let CandidateResult::Allowed(url, _) = candidate else {
            panic!("candidate should be allowed")
        };
        assert_eq!(url.as_str(), "https://example.com/~docs/a%2Fb?x=1");
    }

    #[test]
    fn include_subdomains_is_the_requested_host_suffix_only() {
        assert!(same_site("www.example.co.uk", "example.co.uk", true));
        assert!(same_site(
            "a.shop.example.co.uk",
            "shop.example.co.uk",
            true
        ));
        assert!(!same_site("www.example.co.uk", "docs.example.co.uk", true));
        assert!(!same_site("other.co.uk", "docs.example.co.uk", true));
        assert!(!same_site("evil.co.zz", "shop.example.co.zz", true));
        assert!(same_site(
            "a.shop.example.co.zz",
            "shop.example.co.zz",
            true
        ));
        assert!(!same_site("a.example.co.uk", "example.co.uk", false));
        let url = Url::parse("https://BÜCHER.example/Path").unwrap();
        assert_eq!(url.host_str(), Some("xn--bcher-kva.example"));
        assert!(!same_site("[2001:db8::1]", "[2001:db8::2]", true));
        assert!(same_site("127.0.0.1", "127.0.0.1", true));
        assert!(!same_site("127.0.0.2", "127.0.0.1", true));
    }
}
