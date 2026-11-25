use std::collections::HashSet;

use scraper::{ElementRef, Html, Selector};

use crate::core::fetch::FetchAttributeRequest;

pub(crate) const MAX_SELECTORS: usize = 16;
pub(crate) const MAX_SELECTOR_BYTES: usize = 256;
pub(crate) const MAX_ATTRIBUTES: usize = 16;
pub(crate) const MAX_ATTRIBUTE_NAME_BYTES: usize = 64;
pub(crate) const MAX_ATTRIBUTE_VALUES: usize = 64;
pub(crate) const MAX_ATTRIBUTE_VALUE_CHARS: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectorValidationError {
    pub group: &'static str,
    pub index: usize,
    pub reason: String,
}

impl std::fmt::Display for SelectorValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}[{}]: {}", self.group, self.index, self.reason)
    }
}

pub(crate) struct ParsedAttribute {
    pub selector_text: String,
    pub attribute: String,
    pub selector: Selector,
}

pub(crate) struct SelectorScope {
    pub include: Vec<(String, Selector)>,
    pub exclude: Vec<(String, Selector)>,
    pub attributes: Vec<ParsedAttribute>,
    pub fingerprint: Option<u64>,
}

impl SelectorScope {
    pub fn has_includes(&self) -> bool {
        !self.include.is_empty()
    }

    pub fn has_selectors(&self) -> bool {
        !self.include.is_empty() || !self.exclude.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        !self.has_selectors() && self.attributes.is_empty()
    }
}

pub(crate) struct SelectorMatches {
    pub roots: HashSet<ego_tree::NodeId>,
    pub excluded: HashSet<ego_tree::NodeId>,
}

fn parse_selectors(
    values: Option<&[String]>,
    group: &'static str,
) -> Result<Vec<(String, Selector)>, SelectorValidationError> {
    let values = values.unwrap_or_default();
    if values.len() > MAX_SELECTORS {
        return Err(SelectorValidationError {
            group,
            index: MAX_SELECTORS,
            reason: format!("maximum of {MAX_SELECTORS} selectors exceeded"),
        });
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if value.len() > MAX_SELECTOR_BYTES {
                return Err(SelectorValidationError {
                    group,
                    index,
                    reason: format!("selector exceeds {MAX_SELECTOR_BYTES} bytes"),
                });
            }
            let selector = Selector::parse(value).map_err(|error| SelectorValidationError {
                group,
                index,
                reason: format!("invalid CSS selector: {error}"),
            })?;
            Ok((value.clone(), selector))
        })
        .collect()
}

pub(crate) fn validate_selector_request(
    include: Option<&[String]>,
    exclude: Option<&[String]>,
    attributes: Option<&[FetchAttributeRequest]>,
) -> Result<SelectorScope, SelectorValidationError> {
    let include_parsed = parse_selectors(include, "include_selectors")?;
    let exclude_parsed = parse_selectors(exclude, "exclude_selectors")?;
    let attributes = attributes.unwrap_or_default();
    if attributes.len() > MAX_ATTRIBUTES {
        return Err(SelectorValidationError {
            group: "attributes",
            index: MAX_ATTRIBUTES,
            reason: format!("maximum of {MAX_ATTRIBUTES} attribute pairs exceeded"),
        });
    }
    let mut parsed_attributes = Vec::with_capacity(attributes.len());
    for (index, attribute) in attributes.iter().enumerate() {
        if attribute.selector.len() > MAX_SELECTOR_BYTES {
            return Err(SelectorValidationError {
                group: "attributes",
                index,
                reason: format!("selector exceeds {MAX_SELECTOR_BYTES} bytes"),
            });
        }
        if attribute.attribute.is_empty() || attribute.attribute.len() > MAX_ATTRIBUTE_NAME_BYTES {
            return Err(SelectorValidationError {
                group: "attributes",
                index,
                reason: format!(
                    "attribute name must contain 1 to {MAX_ATTRIBUTE_NAME_BYTES} bytes"
                ),
            });
        }
        let selector =
            Selector::parse(&attribute.selector).map_err(|error| SelectorValidationError {
                group: "attributes",
                index,
                reason: format!("invalid CSS selector: {error}"),
            })?;
        parsed_attributes.push(ParsedAttribute {
            selector_text: attribute.selector.clone(),
            attribute: attribute.attribute.clone(),
            selector,
        });
    }
    let include_text = include.unwrap_or_default().to_vec();
    let mut exclude_text = exclude.unwrap_or_default().to_vec();
    exclude_text.sort();
    let attribute_text = attributes
        .iter()
        .map(|pair| (pair.selector.clone(), pair.attribute.clone()))
        .collect::<Vec<_>>();
    let fingerprint =
        if include_text.is_empty() && exclude_text.is_empty() && attribute_text.is_empty() {
            None
        } else {
            let serialized = serde_json::to_vec(&(exclude_text, include_text, attribute_text))
                .unwrap_or_default();
            let mut hash = 0xcbf29ce484222325u64;
            for byte in serialized {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            Some(hash)
        };
    Ok(SelectorScope {
        include: include_parsed,
        exclude: exclude_parsed,
        attributes: parsed_attributes,
        fingerprint,
    })
}

pub(crate) fn match_document(document: &Html, scope: &SelectorScope) -> SelectorMatches {
    let mut excluded = HashSet::new();
    for (_, selector) in &scope.exclude {
        excluded.extend(document.select(selector).map(|element| element.id()));
    }
    let mut roots = HashSet::new();
    if scope.has_includes() {
        let mut selected = HashSet::new();
        for (_, selector) in &scope.include {
            selected.extend(document.select(selector).map(|element| element.id()));
        }
        let all = Selector::parse("*").expect("universal CSS selector is valid");
        for element in document.select(&all) {
            let id = element.id();
            if !selected.contains(&id) {
                continue;
            }
            let mut parent = element.parent();
            let mut blocked = false;
            let mut selected_ancestor = false;
            while let Some(node) = parent {
                let Some(parent_element) = ElementRef::wrap(node) else {
                    break;
                };
                let parent_id = parent_element.id();
                if excluded.contains(&parent_id) {
                    blocked = true;
                    break;
                }
                if selected.contains(&parent_id) {
                    selected_ancestor = true;
                    break;
                }
                parent = parent_element.parent();
            }
            if !blocked && !selected_ancestor && !excluded.contains(&id) {
                roots.insert(id);
            }
        }
    }
    SelectorMatches { roots, excluded }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_validation_bounds_and_fingerprint_are_deterministic() {
        let includes = vec!["main".to_string()];
        let exclusions = vec!["footer".to_string(), "nav".to_string()];
        let attrs = vec![FetchAttributeRequest {
            selector: "a".to_string(),
            attribute: "href".to_string(),
        }];
        let first = validate_selector_request(Some(&includes), Some(&exclusions), Some(&attrs))
            .expect("valid selectors");
        let second = validate_selector_request(
            Some(&includes),
            Some(&["nav".to_string(), "footer".to_string()]),
            Some(&attrs),
        )
        .expect("valid selectors");
        assert_eq!(first.fingerprint, second.fingerprint);
        assert!(validate_selector_request(Some(&["[".into()]), None, None).is_err());
        assert!(validate_selector_request(Some(&["x".repeat(257)]), None, None).is_err());
    }

    #[test]
    fn attribute_pair_count_is_bounded() {
        let attributes: Vec<FetchAttributeRequest> = (0..=MAX_ATTRIBUTES)
            .map(|index| FetchAttributeRequest {
                selector: format!("a{index}"),
                attribute: "href".to_string(),
            })
            .collect();
        let error = validate_selector_request(None, None, Some(&attributes))
            .err()
            .expect("the attribute pair bound must refuse one over the limit");
        assert_eq!(error.group, "attributes");
        assert_eq!(error.index, MAX_ATTRIBUTES);
        assert!(error
            .reason
            .contains("maximum of 16 attribute pairs exceeded"));
    }

    #[test]
    fn inclusion_roots_are_ordered_deduplicated_and_exclusions_use_source_tree() {
        let scope = validate_selector_request(
            Some(&[".target".into(), "#first".into()]),
            Some(&[".excluded".into()]),
            None,
        )
        .expect("valid selectors");
        let html = Html::parse_document(
            "<main><section id='first' class='target'>One<p>Two</p></section><section class='target excluded'>Hidden</section><section class='target'>Three</section></main>",
        );
        let matches = match_document(&html, &scope);
        assert_eq!(matches.roots.len(), 2);
        let all = Selector::parse("*").unwrap();
        let ordered = html
            .select(&all)
            .filter(|element| matches.roots.contains(&element.id()))
            .map(|element| element.value().attr("id").unwrap_or("last"))
            .collect::<Vec<_>>();
        assert_eq!(ordered, vec!["first", "last"]);
        assert!(matches.excluded.len() == 1);
    }

    #[test]
    fn no_inclusion_match_returns_no_roots() {
        let scope = validate_selector_request(Some(&[".missing".into()]), None, None)
            .expect("valid selector");
        let html = Html::parse_document("<html><body><p>content</p></body></html>");
        assert!(match_document(&html, &scope).roots.is_empty());
    }
}
