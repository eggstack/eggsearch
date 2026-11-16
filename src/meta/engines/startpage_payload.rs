use scraper::Html;
use serde_json::Value;

use super::is_http_url;
use super::models::SearchResult;
use crate::core::sanitize::{
    normalize_whitespace, strip_control_chars, truncate_at_word, SNIPPET_MAX_CHARS,
};

const ENGINE: &str = "startpage";
const PAYLOAD_PREFIX: &str = "React.createElement(UIStartpage.AppSerp";
const WEB_REGION_PREFIX: &str = "web";

pub(super) fn parse_payload(html: &str, max_results: usize) -> Option<Vec<SearchResult>> {
    let object = payload_object(html)?;
    Some(payload_results(object, max_results))
}

fn payload_object(html: &str) -> Option<&str> {
    let start = html.find(PAYLOAD_PREFIX)?;
    let rest = &html[start + PAYLOAD_PREFIX.len()..];
    let token_len = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if token_len == 0 {
        return None;
    }
    let after_token = &rest[token_len..];
    let open = after_token.find('{')?;
    if after_token[..open].trim() != "," {
        return None;
    }
    object_end(&after_token[open..])
}

fn object_end(object: &str) -> Option<&str> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in object.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&object[..index + ch.len_utf8()]);
                }
            }
            _ => {}
        }
    }
    None
}

fn payload_results(object: &str, max_results: usize) -> Vec<SearchResult> {
    let Ok(value) = serde_json::from_str::<Value>(object) else {
        return Vec::new();
    };
    let Some(regions) = value
        .get("render")
        .and_then(|render| render.get("presenter"))
        .and_then(|presenter| presenter.get("regions"))
        .and_then(|regions| regions.get("mainline"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    let mut results = Vec::new();
    for region in regions {
        if results.len() >= max_results {
            break;
        }
        let web_region = region
            .get("display_type")
            .and_then(Value::as_str)
            .is_some_and(|display_type| display_type.starts_with(WEB_REGION_PREFIX));
        if !web_region {
            continue;
        }
        let Some(entries) = region.get("results").and_then(Value::as_array) else {
            continue;
        };
        for entry in entries {
            if results.len() >= max_results {
                break;
            }
            if let Some(result) = entry_result(entry) {
                results.push(result);
            }
        }
    }
    results
}

fn entry_result(entry: &Value) -> Option<SearchResult> {
    let url = entry.get("clickUrl").and_then(Value::as_str)?.trim();
    if !is_http_url(url) {
        return None;
    }
    let title = html_to_text(entry.get("title").and_then(Value::as_str)?)?;
    let snippet = entry
        .get("description")
        .and_then(Value::as_str)
        .and_then(html_to_text)
        .map(|text| truncate_at_word(&text, SNIPPET_MAX_CHARS));
    Some(SearchResult {
        title,
        url: url.to_string(),
        snippet,
        source_engine: ENGINE.to_string(),
        excerpts: Vec::new(),
        published_at: None,
        metadata: Default::default(),
    })
}

fn html_to_text(raw: &str) -> Option<String> {
    let (stripped, _) = strip_control_chars(raw);
    if stripped.trim().is_empty() {
        return None;
    }
    let fragment = Html::parse_fragment(&stripped);
    let mut text = String::new();
    for node in fragment.root_element().descendants() {
        let Some(value) = node.value().as_text() else {
            continue;
        };
        if node.ancestors().any(|ancestor| {
            ancestor
                .value()
                .as_element()
                .is_some_and(|element| matches!(element.name(), "script" | "style"))
        }) {
            continue;
        }
        text.push_str(value);
        text.push(' ');
    }
    let normalized = normalize_whitespace(&text);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(payload: &str) -> String {
        format!(
            r#"<html><body><script>window.__DATA__ = React.createElement(UIStartpage.AppSerpWeb, {payload})</script></body></html>"#
        )
    }

    fn web_payload(entries: &str) -> String {
        format!(
            r#"{{"render":{{"presenter":{{"regions":{{"mainline":[{{"display_type":"web-google","results":[{entries}]}}]}}}}}}}}"#
        )
    }

    #[test]
    fn maps_url_title_and_snippet() {
        let html = page(&web_payload(
            r#"{"clickUrl":"https://example.com/a","title":"<b>Example</b> Title","description":"<b>Bold</b> text &amp; more"}"#,
        ));
        let rows = parse_payload(&html, 10).expect("payload present");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].url, "https://example.com/a");
        assert_eq!(rows[0].title, "Example Title");
        assert_eq!(rows[0].snippet.as_deref(), Some("Bold text & more"));
        assert_eq!(rows[0].source_engine, "startpage");
    }

    #[test]
    fn decodes_nbsp_in_description() {
        let html = page(&web_payload(
            r#"{"clickUrl":"https://example.com/a","title":"Title","description":"one&nbsp;two"}"#,
        ));
        let rows = parse_payload(&html, 10).expect("payload present");
        assert_eq!(rows[0].snippet.as_deref(), Some("one two"));
    }

    #[test]
    fn skips_non_web_regions() {
        let html = page(
            r#"{"render":{"presenter":{"regions":{"mainline":[{"display_type":"ads-google-top","results":[{"clickUrl":"https://ads.example.com","title":"Ad","description":"Sponsored"}]},{"display_type":"web-google","results":[{"clickUrl":"https://example.com/web","title":"Web","description":"Organic"}]}]}}}}"#,
        );
        let rows = parse_payload(&html, 10).expect("payload present");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].url, "https://example.com/web");
    }

    #[test]
    fn skips_partial_entries_without_dropping_the_page() {
        let html = page(&web_payload(
            r#"{"clickUrl":"https://example.com/no-title","description":"missing title"},{"clickUrl":"https://example.com/empty-title","title":"   ","description":"blank"},{"clickUrl":"/relative","title":"Relative","description":"not http"},{"clickUrl":"javascript:alert(1)","title":"Script","description":"not http"},{"title":"No URL","description":"absent clickUrl"},{"clickUrl":"https://example.com/good","title":"Good","description":"kept"}"#,
        ));
        let rows = parse_payload(&html, 10).expect("payload present");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].url, "https://example.com/good");
        assert_eq!(rows[0].title, "Good");
    }

    #[test]
    fn respects_max_results() {
        let html = page(&web_payload(
            r#"{"clickUrl":"https://example.com/1","title":"One"},{"clickUrl":"https://example.com/2","title":"Two"}"#,
        ));
        let rows = parse_payload(&html, 1).expect("payload present");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].url, "https://example.com/1");
    }

    #[test]
    fn absent_payload_returns_none() {
        let html = r#"<html><body><div class="result">no payload here</div></body></html>"#;
        assert!(parse_payload(html, 10).is_none());
    }

    #[test]
    fn other_category_token_is_located_but_yields_no_web_rows() {
        let html = r#"<html><body><script>React.createElement(UIStartpage.AppSerpImages, {"render":{}})</script></body></html>"#;
        let rows = parse_payload(html, 10).expect("payload located");
        assert!(rows.is_empty());
    }

    #[test]
    fn malformed_json_is_present_but_empty() {
        let html = page(r#"{"render":{"presenter":{"regions":undefined}}}"#);
        let rows = parse_payload(&html, 10).expect("payload located");
        assert!(rows.is_empty());
    }

    #[test]
    fn unclosed_payload_returns_none() {
        let html =
            page(r#"{"render":{"presenter":{"regions":{"mainline":[{"display_type":"web-google""#);
        assert!(parse_payload(&html, 10).is_none());
    }

    #[test]
    fn missing_mainline_is_present_but_empty() {
        let html = page(r#"{"render":{"presenter":{"regions":{}}}}"#);
        let rows = parse_payload(&html, 10).expect("payload located");
        assert!(rows.is_empty());
    }

    #[test]
    fn empty_web_region_yields_no_rows() {
        let html = page(&web_payload(""));
        let rows = parse_payload(&html, 10).expect("payload located");
        assert!(rows.is_empty());
    }

    #[test]
    fn braces_inside_strings_do_not_end_the_object() {
        let html = page(&web_payload(
            r#"{"clickUrl":"https://example.com/a","title":"A } title","description":"has } brace"}"#,
        ));
        let rows = parse_payload(&html, 10).expect("payload present");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "A } title");
    }
}
