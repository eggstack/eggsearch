use scraper::Html;

use crate::core::fetch::{FetchAttributeExtraction, FetchAttributeValue};
use crate::core::sanitize::{bound_text, strip_control_chars};
use crate::fetch::scope::{SelectorScope, MAX_ATTRIBUTE_VALUES, MAX_ATTRIBUTE_VALUE_CHARS};

pub(crate) fn extract_attributes(
    html: &[u8],
    scope: &SelectorScope,
) -> Vec<FetchAttributeExtraction> {
    if scope.attributes.is_empty() {
        return Vec::new();
    }
    let decoded = String::from_utf8_lossy(html);
    let document = Html::parse_document(&decoded);
    scope
        .attributes
        .iter()
        .map(|request| {
            let lookup = request.attribute.to_ascii_lowercase();
            let values = document
                .select(&request.selector)
                .filter_map(|element| element.value().attr(&lookup))
                .take(MAX_ATTRIBUTE_VALUES)
                .enumerate()
                .map(|(ordinal, value)| {
                    let (stripped, _) = strip_control_chars(value);
                    let (value, _) = bound_text(&stripped, MAX_ATTRIBUTE_VALUE_CHARS);
                    FetchAttributeValue { value, ordinal }
                })
                .collect();
            FetchAttributeExtraction {
                selector: request.selector_text.clone(),
                attribute: request.attribute.clone(),
                values,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::fetch::FetchAttributeRequest;
    use crate::fetch::scope::validate_selector_request;

    #[test]
    fn attribute_values_are_ordered_sanitized_and_capped() {
        let requests = vec![FetchAttributeRequest {
            selector: "a".into(),
            attribute: "title".into(),
        }];
        let scope = validate_selector_request(None, None, Some(&requests)).unwrap();
        let mut html = String::from("<html><body>");
        for index in 0..70 {
            let value = if index == 0 {
                "x".repeat(MAX_ATTRIBUTE_VALUE_CHARS + 20)
            } else {
                format!("value{index}\u{1}")
            };
            html.push_str(&format!("<a title='{value}'>x</a>"));
        }
        html.push_str("</body></html>");
        let extracted = extract_attributes(html.as_bytes(), &scope);
        assert_eq!(extracted[0].values.len(), MAX_ATTRIBUTE_VALUES);
        assert_eq!(extracted[0].values[0].ordinal, 0);
        assert_eq!(
            extracted[0].values[0].value.chars().count(),
            MAX_ATTRIBUTE_VALUE_CHARS
        );
        assert_eq!(extracted[0].values[63].ordinal, 63);
    }
}
