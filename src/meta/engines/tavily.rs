use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::error::EngineError;
use super::models::SearchResult;
use super::request::EngineSearchRequest;
use crate::core::query::{Freshness, SafeSearch, SearchIntent};

const ENGINE: &str = "tavily";
const DEFAULT_URL: &str = "https://api.tavily.com/search";
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const TAVILY_MAX_RESULTS: usize = 20;

#[derive(Debug, Serialize)]
struct TavilySearchRequest {
    query: String,
    search_depth: String,
    max_results: usize,
    chunks_per_source: usize,
    topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_domains_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter_by_language: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safe_search: Option<bool>,
    include_answer: bool,
    include_raw_content: bool,
    include_images: bool,
    auto_parameters: bool,
}

#[derive(Debug, Deserialize)]
struct TavilySearchResponse {
    #[serde(default)]
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize, Clone)]
struct TavilyResult {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    content: Option<String>,
}

fn resolve_url(base_url: Option<&str>) -> String {
    match base_url {
        Some(u) if !u.trim().is_empty() => u.trim().to_string(),
        _ => DEFAULT_URL.to_string(),
    }
}

fn clamp_max_results(max_results: usize) -> usize {
    max_results.clamp(1, TAVILY_MAX_RESULTS)
}

fn resolve_chunks(excerpt_count: usize) -> usize {
    if excerpt_count == 0 {
        1
    } else {
        excerpt_count.clamp(1, 3)
    }
}

fn map_topic(intent: SearchIntent) -> String {
    match intent {
        SearchIntent::News => "news".to_string(),
        _ => "general".to_string(),
    }
}

fn map_time_range(freshness: Freshness, has_exact_range: bool) -> Option<String> {
    if has_exact_range {
        return None;
    }
    match freshness {
        Freshness::Any => None,
        Freshness::Day => Some("day".to_string()),
        Freshness::Week => Some("week".to_string()),
        Freshness::Month => Some("month".to_string()),
        Freshness::Year => Some("year".to_string()),
    }
}

fn map_start_end(
    date_range: Option<&crate::core::query::SearchDateRange>,
) -> (Option<String>, Option<String>) {
    let Some(range) = date_range else {
        return (None, None);
    };
    let start = range.start.trim();
    let end = range.end.trim();
    if start.is_empty() || end.is_empty() {
        return (None, None);
    }
    (Some(start.to_string()), Some(end.to_string()))
}

fn map_safe_search(value: Option<SafeSearch>) -> Option<bool> {
    match value {
        None => None,
        Some(SafeSearch::Off) => Some(false),
        Some(SafeSearch::Moderate) | Some(SafeSearch::Strict) => Some(true),
    }
}

fn map_language(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    let normalized = raw.replace('_', "-");
    let parts: Vec<&str> = normalized.split('-').collect();
    match parts.as_slice() {
        [primary]
            if (2..=3).contains(&primary.len())
                && primary.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            Some(primary.to_ascii_lowercase())
        }
        [primary, region]
            if (2..=3).contains(&primary.len())
                && region.len() == 2
                && primary.chars().all(|c| c.is_ascii_alphabetic())
                && region.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            Some(format!(
                "{}-{}",
                primary.to_ascii_lowercase(),
                region.to_ascii_lowercase()
            ))
        }
        _ => None,
    }
}

fn iso_to_country(code: &str) -> Option<&'static str> {
    match code {
        "AF" => Some("afghanistan"),
        "AL" => Some("albania"),
        "DZ" => Some("algeria"),
        "AD" => Some("andorra"),
        "AO" => Some("angola"),
        "AR" => Some("argentina"),
        "AM" => Some("armenia"),
        "AU" => Some("australia"),
        "AT" => Some("austria"),
        "AZ" => Some("azerbaijan"),
        "BS" => Some("bahamas"),
        "BH" => Some("bahrain"),
        "BD" => Some("bangladesh"),
        "BB" => Some("barbados"),
        "BY" => Some("belarus"),
        "BE" => Some("belgium"),
        "BZ" => Some("belize"),
        "BJ" => Some("benin"),
        "BT" => Some("bhutan"),
        "BO" => Some("bolivia"),
        "BA" => Some("bosnia and herzegovina"),
        "BW" => Some("botswana"),
        "BR" => Some("brazil"),
        "BN" => Some("brunei"),
        "BG" => Some("bulgaria"),
        "BF" => Some("burkina faso"),
        "BI" => Some("burundi"),
        "KH" => Some("cambodia"),
        "CM" => Some("cameroon"),
        "CA" => Some("canada"),
        "CV" => Some("cape verde"),
        "CF" => Some("central african republic"),
        "TD" => Some("chad"),
        "CL" => Some("chile"),
        "CN" => Some("china"),
        "CO" => Some("colombia"),
        "KM" => Some("comoros"),
        "CG" => Some("congo"),
        "CR" => Some("costa rica"),
        "HR" => Some("croatia"),
        "CU" => Some("cuba"),
        "CY" => Some("cyprus"),
        "CZ" => Some("czech republic"),
        "DK" => Some("denmark"),
        "DJ" => Some("djibouti"),
        "DO" => Some("dominican republic"),
        "EC" => Some("ecuador"),
        "EG" => Some("egypt"),
        "SV" => Some("el salvador"),
        "GQ" => Some("equatorial guinea"),
        "ER" => Some("eritrea"),
        "EE" => Some("estonia"),
        "ET" => Some("ethiopia"),
        "FJ" => Some("fiji"),
        "FI" => Some("finland"),
        "FR" => Some("france"),
        "GA" => Some("gabon"),
        "GM" => Some("gambia"),
        "GE" => Some("georgia"),
        "DE" => Some("germany"),
        "GH" => Some("ghana"),
        "GR" => Some("greece"),
        "GT" => Some("guatemala"),
        "GN" => Some("guinea"),
        "HT" => Some("haiti"),
        "HN" => Some("honduras"),
        "HU" => Some("hungary"),
        "IS" => Some("iceland"),
        "IN" => Some("india"),
        "ID" => Some("indonesia"),
        "IR" => Some("iran"),
        "IQ" => Some("iraq"),
        "IE" => Some("ireland"),
        "IL" => Some("israel"),
        "IT" => Some("italy"),
        "JM" => Some("jamaica"),
        "JP" => Some("japan"),
        "JO" => Some("jordan"),
        "KZ" => Some("kazakhstan"),
        "KE" => Some("kenya"),
        "KW" => Some("kuwait"),
        "KG" => Some("kyrgyzstan"),
        "LV" => Some("latvia"),
        "LB" => Some("lebanon"),
        "LS" => Some("lesotho"),
        "LR" => Some("liberia"),
        "LY" => Some("libya"),
        "LI" => Some("liechtenstein"),
        "LT" => Some("lithuania"),
        "LU" => Some("luxembourg"),
        "MG" => Some("madagascar"),
        "MW" => Some("malawi"),
        "MY" => Some("malaysia"),
        "MV" => Some("maldives"),
        "ML" => Some("mali"),
        "MT" => Some("malta"),
        "MR" => Some("mauritania"),
        "MU" => Some("mauritius"),
        "MX" => Some("mexico"),
        "MD" => Some("moldova"),
        "MC" => Some("monaco"),
        "MN" => Some("mongolia"),
        "ME" => Some("montenegro"),
        "MA" => Some("morocco"),
        "MZ" => Some("mozambique"),
        "MM" => Some("myanmar"),
        "NA" => Some("namibia"),
        "NP" => Some("nepal"),
        "NL" => Some("netherlands"),
        "NZ" => Some("new zealand"),
        "NI" => Some("nicaragua"),
        "NE" => Some("niger"),
        "NG" => Some("nigeria"),
        "KP" => Some("north korea"),
        "MK" => Some("north macedonia"),
        "NO" => Some("norway"),
        "OM" => Some("oman"),
        "PK" => Some("pakistan"),
        "PA" => Some("panama"),
        "PG" => Some("papua new guinea"),
        "PY" => Some("paraguay"),
        "PE" => Some("peru"),
        "PH" => Some("philippines"),
        "PL" => Some("poland"),
        "PT" => Some("portugal"),
        "QA" => Some("qatar"),
        "RO" => Some("romania"),
        "RU" => Some("russia"),
        "RW" => Some("rwanda"),
        "SA" => Some("saudi arabia"),
        "SN" => Some("senegal"),
        "RS" => Some("serbia"),
        "SG" => Some("singapore"),
        "SK" => Some("slovakia"),
        "SI" => Some("slovenia"),
        "SO" => Some("somalia"),
        "ZA" => Some("south africa"),
        "KR" => Some("south korea"),
        "SS" => Some("south sudan"),
        "ES" => Some("spain"),
        "LK" => Some("sri lanka"),
        "SD" => Some("sudan"),
        "SE" => Some("sweden"),
        "CH" => Some("switzerland"),
        "SY" => Some("syria"),
        "TW" => Some("taiwan"),
        "TJ" => Some("tajikistan"),
        "TZ" => Some("tanzania"),
        "TH" => Some("thailand"),
        "TG" => Some("togo"),
        "TT" => Some("trinidad and tobago"),
        "TN" => Some("tunisia"),
        "TR" => Some("turkey"),
        "TM" => Some("turkmenistan"),
        "UG" => Some("uganda"),
        "UA" => Some("ukraine"),
        "AE" => Some("united arab emirates"),
        "GB" => Some("united kingdom"),
        "UK" => Some("united kingdom"),
        "US" => Some("united states"),
        "UY" => Some("uruguay"),
        "UZ" => Some("uzbekistan"),
        "VE" => Some("venezuela"),
        "VN" => Some("vietnam"),
        "YE" => Some("yemen"),
        "ZM" => Some("zambia"),
        "ZW" => Some("zimbabwe"),
        _ => None,
    }
}

fn is_known_country_name(name: &str) -> bool {
    matches!(
        name,
        "afghanistan"
            | "albania"
            | "algeria"
            | "andorra"
            | "angola"
            | "argentina"
            | "armenia"
            | "australia"
            | "austria"
            | "azerbaijan"
            | "bahamas"
            | "bahrain"
            | "bangladesh"
            | "barbados"
            | "belarus"
            | "belgium"
            | "belize"
            | "benin"
            | "bhutan"
            | "bolivia"
            | "bosnia and herzegovina"
            | "botswana"
            | "brazil"
            | "brunei"
            | "bulgaria"
            | "burkina faso"
            | "burundi"
            | "cambodia"
            | "cameroon"
            | "canada"
            | "cape verde"
            | "central african republic"
            | "chad"
            | "chile"
            | "china"
            | "colombia"
            | "comoros"
            | "congo"
            | "costa rica"
            | "croatia"
            | "cuba"
            | "cyprus"
            | "czech republic"
            | "denmark"
            | "djibouti"
            | "dominican republic"
            | "ecuador"
            | "egypt"
            | "el salvador"
            | "equatorial guinea"
            | "eritrea"
            | "estonia"
            | "ethiopia"
            | "fiji"
            | "finland"
            | "france"
            | "gabon"
            | "gambia"
            | "georgia"
            | "germany"
            | "ghana"
            | "greece"
            | "guatemala"
            | "guinea"
            | "haiti"
            | "honduras"
            | "hungary"
            | "iceland"
            | "india"
            | "indonesia"
            | "iran"
            | "iraq"
            | "ireland"
            | "israel"
            | "italy"
            | "jamaica"
            | "japan"
            | "jordan"
            | "kazakhstan"
            | "kenya"
            | "kuwait"
            | "kyrgyzstan"
            | "latvia"
            | "lebanon"
            | "lesotho"
            | "liberia"
            | "libya"
            | "liechtenstein"
            | "lithuania"
            | "luxembourg"
            | "madagascar"
            | "malawi"
            | "malaysia"
            | "maldives"
            | "mali"
            | "malta"
            | "mauritania"
            | "mauritius"
            | "mexico"
            | "moldova"
            | "monaco"
            | "mongolia"
            | "montenegro"
            | "morocco"
            | "mozambique"
            | "myanmar"
            | "namibia"
            | "nepal"
            | "netherlands"
            | "new zealand"
            | "nicaragua"
            | "niger"
            | "nigeria"
            | "north korea"
            | "north macedonia"
            | "norway"
            | "oman"
            | "pakistan"
            | "panama"
            | "papua new guinea"
            | "paraguay"
            | "peru"
            | "philippines"
            | "poland"
            | "portugal"
            | "qatar"
            | "romania"
            | "russia"
            | "rwanda"
            | "saudi arabia"
            | "senegal"
            | "serbia"
            | "singapore"
            | "slovakia"
            | "slovenia"
            | "somalia"
            | "south africa"
            | "south korea"
            | "south sudan"
            | "spain"
            | "sri lanka"
            | "sudan"
            | "sweden"
            | "switzerland"
            | "syria"
            | "taiwan"
            | "tajikistan"
            | "tanzania"
            | "thailand"
            | "togo"
            | "trinidad and tobago"
            | "tunisia"
            | "turkey"
            | "turkmenistan"
            | "uganda"
            | "ukraine"
            | "united arab emirates"
            | "united kingdom"
            | "united states"
            | "uruguay"
            | "uzbekistan"
            | "venezuela"
            | "vietnam"
            | "yemen"
            | "zambia"
            | "zimbabwe"
    )
}

fn map_country(value: Option<&str>, news_topic: bool) -> Option<String> {
    if news_topic {
        return None;
    }
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.len() == 2 && raw.chars().all(|c| c.is_ascii_alphabetic()) {
        let code = raw.to_ascii_uppercase();
        return iso_to_country(code.as_str()).map(|s| s.to_string());
    }
    let lowered = raw.to_ascii_lowercase().replace(['_', '-'], " ");
    let collapsed = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    if is_known_country_name(collapsed.as_str()) {
        Some(collapsed)
    } else {
        None
    }
}

fn split_content_chunks(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in content.split("[...]") {
        let text = crate::core::sanitize::normalize_whitespace(part)
            .trim()
            .to_string();
        if text.is_empty() {
            continue;
        }
        out.push(text);
    }
    out
}

fn build_request_body(request: &EngineSearchRequest) -> TavilySearchRequest {
    let topic = map_topic(request.intent);
    let news_topic = topic == "news";
    let (start_date, end_date) = map_start_end(request.date_range.as_ref());
    let has_exact = start_date.is_some() && end_date.is_some();
    let time_range = map_time_range(request.freshness, has_exact);
    let include_domains = if request.include_domains.is_empty() {
        None
    } else {
        Some(request.include_domains.clone())
    };
    let exclude_domains = if request.exclude_domains.is_empty() {
        None
    } else {
        Some(request.exclude_domains.clone())
    };
    let include_domains_mode = if include_domains.is_some() {
        Some("filter".to_string())
    } else {
        None
    };
    let country = map_country(request.region.as_deref(), news_topic);
    let language = map_language(request.language.as_deref());
    let filter_by_language = if language.is_some() { Some(true) } else { None };
    TavilySearchRequest {
        query: request.query.clone(),
        search_depth: "basic".to_string(),
        max_results: clamp_max_results(request.max_results),
        chunks_per_source: resolve_chunks(request.excerpt_count),
        topic,
        time_range,
        start_date,
        end_date,
        include_domains,
        exclude_domains,
        include_domains_mode,
        country,
        language,
        filter_by_language,
        safe_search: map_safe_search(request.safe_search),
        include_answer: false,
        include_raw_content: false,
        include_images: false,
        auto_parameters: false,
    }
}

pub async fn search(
    client: &Client,
    api_key: &str,
    base_url: Option<&str>,
    request: &EngineSearchRequest,
) -> Result<Vec<SearchResult>, EngineError> {
    if request.max_results == 0 {
        return Ok(Vec::new());
    }
    let url = resolve_url(base_url);
    let body = build_request_body(request);
    let timeout = request.timeout;
    let max_results = request.max_results;
    let excerpt_count = request
        .excerpt_count
        .min(crate::core::source_card::MAX_EXCERPT_REQUEST_COUNT);
    let bytes = tokio::time::timeout(timeout, async {
        let resp = client
            .post(url)
            .json(&body)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|e| EngineError::Http {
                engine: ENGINE,
                source: e,
            })?;
        let status = resp.status();
        if !status.is_success() {
            return Err(EngineError::BadStatus {
                engine: ENGINE,
                status: status.as_u16(),
            });
        }
        super::read_bounded_body(resp, ENGINE, MAX_BODY_BYTES).await
    })
    .await
    .map_err(|_| EngineError::Timeout { engine: ENGINE })??;

    let parsed: TavilySearchResponse =
        serde_json::from_slice(&bytes).map_err(|e| EngineError::ParseFailed {
            engine: ENGINE,
            reason: format!("invalid JSON: {e}"),
        })?;
    Ok(convert(parsed.results, max_results, excerpt_count))
}

fn convert(raw: Vec<TavilyResult>, max_results: usize, excerpt_count: usize) -> Vec<SearchResult> {
    let mut out = Vec::with_capacity(max_results.min(raw.len()));
    for r in raw {
        if out.len() >= max_results {
            break;
        }
        let Some(url) = r.url else { continue };
        if !super::is_http_url(&url) {
            continue;
        }
        let title = r
            .title
            .map(|t| crate::core::sanitize::normalize_whitespace(&t))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let Some(title) = title else { continue };
        let chunks = r
            .content
            .as_deref()
            .map(split_content_chunks)
            .unwrap_or_default();
        let snippet = chunks.first().cloned();
        let mut excerpts = Vec::new();
        if excerpt_count > 0 {
            for text in chunks.iter().take(excerpt_count) {
                if text.trim().is_empty() {
                    continue;
                }
                excerpts.push(crate::core::source_card::SourceExcerpt {
                    text: text.clone(),
                    score: None,
                    provenance: crate::core::source_card::ExcerptProvenance::ProviderSnippet,
                });
                if excerpts.len() >= excerpt_count {
                    break;
                }
            }
        }
        out.push(SearchResult {
            title,
            url,
            snippet,
            source_engine: ENGINE.to_string(),
            excerpts,
            published_at: None,
            metadata: Default::default(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn simple_req(query: &str, max_results: usize) -> EngineSearchRequest {
        EngineSearchRequest::simple(query, max_results, Duration::from_secs(5))
    }

    #[test]
    fn clamp_max_results_bounds() {
        assert_eq!(clamp_max_results(0), 1);
        assert_eq!(clamp_max_results(5), 5);
        assert_eq!(clamp_max_results(500), TAVILY_MAX_RESULTS);
    }

    #[test]
    fn resolve_chunks_bounds() {
        assert_eq!(resolve_chunks(0), 1);
        assert_eq!(resolve_chunks(1), 1);
        assert_eq!(resolve_chunks(2), 2);
        assert_eq!(resolve_chunks(3), 3);
        assert_eq!(resolve_chunks(10), 3);
    }

    #[test]
    fn resolve_url_defaults_and_trims() {
        assert_eq!(resolve_url(None), DEFAULT_URL);
        assert_eq!(resolve_url(Some("")), DEFAULT_URL);
        assert_eq!(resolve_url(Some("  ")), DEFAULT_URL);
        assert_eq!(
            resolve_url(Some("  https://proxy.example/search  ")),
            "https://proxy.example/search"
        );
    }

    #[test]
    fn topic_routes_news_intent() {
        assert_eq!(map_topic(SearchIntent::News), "news");
        assert_eq!(map_topic(SearchIntent::Web), "general");
        assert_eq!(map_topic(SearchIntent::Code), "general");
    }

    #[test]
    fn exact_range_takes_precedence_over_freshness() {
        let mut req = simple_req("q", 5);
        req.freshness = Freshness::Week;
        req.date_range = Some(crate::core::query::SearchDateRange::new(
            "2024-01-01",
            "2024-01-31",
        ));
        let body = build_request_body(&req);
        assert_eq!(body.start_date.as_deref(), Some("2024-01-01"));
        assert_eq!(body.end_date.as_deref(), Some("2024-01-31"));
        assert!(body.time_range.is_none());
    }

    #[test]
    fn relative_freshness_maps_to_time_range() {
        for (freshness, expected) in [
            (Freshness::Day, "day"),
            (Freshness::Week, "week"),
            (Freshness::Month, "month"),
            (Freshness::Year, "year"),
        ] {
            let mut req = simple_req("q", 5);
            req.freshness = freshness;
            let body = build_request_body(&req);
            assert_eq!(body.time_range.as_deref(), Some(expected));
            assert!(body.start_date.is_none());
            assert!(body.end_date.is_none());
        }
        let plain = simple_req("q", 5);
        let body = build_request_body(&plain);
        assert!(body.time_range.is_none());
    }

    #[test]
    fn safe_search_collapses_moderate_strict_to_true() {
        assert_eq!(map_safe_search(None), None);
        assert_eq!(map_safe_search(Some(SafeSearch::Off)), Some(false));
        assert_eq!(map_safe_search(Some(SafeSearch::Moderate)), Some(true));
        assert_eq!(map_safe_search(Some(SafeSearch::Strict)), Some(true));
    }

    #[test]
    fn language_maps_only_when_representable() {
        assert_eq!(map_language(Some("en")).as_deref(), Some("en"));
        assert_eq!(map_language(Some("en-US")).as_deref(), Some("en-us"));
        assert_eq!(map_language(Some("en_US")).as_deref(), Some("en-us"));
        assert_eq!(map_language(Some("zh-CN")).as_deref(), Some("zh-cn"));
        assert_eq!(map_language(Some("not-a-locale!!!")), None);
        assert_eq!(map_language(None), None);
    }

    #[test]
    fn country_maps_iso_and_names_but_not_news() {
        assert_eq!(
            map_country(Some("US"), false).as_deref(),
            Some("united states")
        );
        assert_eq!(
            map_country(Some("us"), false).as_deref(),
            Some("united states")
        );
        assert_eq!(
            map_country(Some("GB"), false).as_deref(),
            Some("united kingdom")
        );
        assert_eq!(
            map_country(Some("UK"), false).as_deref(),
            Some("united kingdom")
        );
        assert_eq!(
            map_country(Some("france"), false).as_deref(),
            Some("france")
        );
        assert_eq!(map_country(Some("USA"), false), None);
        assert_eq!(map_country(Some("u1"), false), None);
        assert_eq!(map_country(None, false), None);
        assert_eq!(map_country(Some("US"), true), None);
    }

    #[test]
    fn split_content_chunks_handles_delimiter() {
        let chunks = split_content_chunks("first chunk [...] second chunk [...] third");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "first chunk");
        let single = split_content_chunks("only one chunk");
        assert_eq!(single, vec!["only one chunk".to_string()]);
        let empty = split_content_chunks("   [...]   ");
        assert!(empty.is_empty());
    }

    #[test]
    fn convert_uses_first_chunk_as_snippet_and_bounded_excerpts() {
        let raw = vec![TavilyResult {
            title: Some("Example".to_string()),
            url: Some("https://example.com/a".to_string()),
            content: Some("first [...] second [...] third [...] fourth".to_string()),
        }];
        let without = convert(raw.clone(), 10, 0);
        assert_eq!(without.len(), 1);
        assert_eq!(without[0].snippet.as_deref(), Some("first"));
        assert!(without[0].excerpts.is_empty());
        assert!(without[0].published_at.is_none());
        let with = convert(raw, 10, 2);
        assert_eq!(with[0].excerpts.len(), 2);
        assert_eq!(with[0].excerpts[0].text, "first");
        assert!(matches!(
            with[0].excerpts[0].provenance,
            crate::core::source_card::ExcerptProvenance::ProviderSnippet
        ));
    }

    #[test]
    fn convert_skips_missing_title_and_non_http() {
        let raw = vec![
            TavilyResult {
                title: None,
                url: Some("https://example.com/a".to_string()),
                content: Some("c".to_string()),
            },
            TavilyResult {
                title: Some("T".to_string()),
                url: Some("/relative".to_string()),
                content: Some("c".to_string()),
            },
            TavilyResult {
                title: Some("Valid".to_string()),
                url: Some("https://valid.example".to_string()),
                content: None,
            },
        ];
        let out = convert(raw, 10, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url, "https://valid.example");
        assert!(out[0].snippet.is_none());
    }

    #[test]
    fn descriptor_flags_are_conservative() {
        let desc = crate::core::provider::built_in_provider_descriptor(
            "tavily", true, false, true, false, None, None,
        )
        .expect("descriptor");
        assert_eq!(desc.id, "tavily");
        assert_eq!(desc.kind, crate::core::provider::ProviderKind::ApiKey);
        assert!(desc.requires_api_key);
        assert!(desc.capabilities.supports_safe_search);
        assert!(desc.capabilities.supports_freshness);
        assert!(desc.capabilities.supports_language);
        assert!(desc.capabilities.supports_region);
        assert!(desc.capabilities.supports_domain_filters);
        assert!(desc.capabilities.supports_news);
        assert!(!desc.capabilities.supports_result_timestamps);
        assert!(!desc.capabilities.supports_code_search);
        assert!(!desc.capabilities.supports_issue_search);
        assert!(!desc.capabilities.supports_release_search);
    }
}
