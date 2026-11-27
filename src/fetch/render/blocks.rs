use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::core::document::{BlockKind, DocumentOutlineEntry, RenderedBlock};

/// Result of rendering HTML into structured blocks.
pub struct RenderedBlocks {
    /// The rendered content blocks.
    pub blocks: Vec<RenderedBlock>,
    /// Document outline (table of contents) built from headings.
    pub outline: Vec<DocumentOutlineEntry>,
    /// Whether the total text exceeded `max_chars` and was truncated.
    pub text_truncated: bool,
    /// Whether the block list was truncated (exceeded max_chars).
    pub block_truncated: bool,
}

/// Elements whose content is always skipped.
const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "svg", "nav", "footer", "header", "form", "aside", "template",
    "xmp",
];
const MAX_DOM_TEXT_CHARS: usize = 100_000;
const SPARSE_EXTRACTION_THRESHOLD: f64 = 0.05;

/// Maximum recursion depth for HTML tree walks. Adversarial markup
/// with extreme nesting must not overflow the stack.
/// Language class mappings for code blocks.
fn normalize_language(lang: &str) -> String {
    match lang {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "sh" | "shell" => "bash",
        "md" => "markdown",
        other => other,
    }
    .to_string()
}

/// Render HTML bytes into structured blocks.
///
/// Returns `(title, description, rendered_blocks, warnings, non_utf8)`.
pub fn render_blocks(
    html: &[u8],
    base_url: &str,
    max_chars: usize,
    markdown: bool,
) -> (
    Option<String>,
    Option<String>,
    RenderedBlocks,
    Vec<String>,
    bool,
) {
    let (title, description, rendered, warnings, non_utf8, _) =
        render_blocks_inner(html, base_url, max_chars, markdown, None);
    (title, description, rendered, warnings, non_utf8)
}

pub(crate) fn render_blocks_scoped(
    html: &[u8],
    base_url: &str,
    max_chars: usize,
    markdown: bool,
    scope: &crate::fetch::scope::SelectorScope,
) -> (
    Option<String>,
    Option<String>,
    RenderedBlocks,
    Vec<String>,
    bool,
    Option<usize>,
) {
    render_blocks_inner(html, base_url, max_chars, markdown, Some(scope))
}

fn render_blocks_inner(
    html: &[u8],
    base_url: &str,
    max_chars: usize,
    markdown: bool,
    scope: Option<&crate::fetch::scope::SelectorScope>,
) -> (
    Option<String>,
    Option<String>,
    RenderedBlocks,
    Vec<String>,
    bool,
    Option<usize>,
) {
    let (html_str, mut warnings, non_utf8) = decode_html(html);
    let document = Html::parse_document(&html_str);

    let title = extract_title(&document);
    let description = extract_description(&document);

    let mut blocks = Vec::new();
    let mut outline = Vec::new();

    let matches = scope
        .filter(|scope| scope.has_selectors())
        .map(|scope| crate::fetch::scope::match_document(&document, scope));
    let matched_roots = scope
        .filter(|scope| scope.has_includes())
        .map(|_| matches.as_ref().map_or(0, |matches| matches.roots.len()));
    if scope.is_some_and(|scope| scope.has_includes()) {
        let matched = matches.as_ref().expect("include selectors were matched");
        if matched.roots.is_empty() {
            warnings.push("selector_no_match".to_string());
        }
        let all = Selector::parse("*").expect("universal CSS selector is valid");
        for element in document.select(&all) {
            if !matched.roots.contains(&element.id())
                || has_matched_ancestor(element, &matched.roots)
            {
                continue;
            }
            walk_node(
                element,
                &mut blocks,
                &mut outline,
                base_url,
                &mut warnings,
                markdown,
                WalkOptions {
                    depth: 0,
                    matches: matches.as_ref(),
                },
            );
        }
    } else {
        let root = select_content_root(&document);
        walk_element(
            root,
            &mut blocks,
            &mut outline,
            base_url,
            &mut warnings,
            markdown,
            WalkOptions {
                depth: 0,
                matches: matches.as_ref(),
            },
        );
    }

    let rendered_text_chars: usize = blocks.iter().map(|block| block.text.chars().count()).sum();
    let selector_scoped = scope.is_some_and(|scope| scope.has_includes());
    let dom_text_chars = if selector_scoped {
        0
    } else {
        bounded_dom_text_chars(select_content_root(&document), matches.as_ref())
    };
    let block_truncated = truncate_blocks_at_budget(&mut blocks, max_chars);
    let selector_no_text =
        selector_scoped && matched_roots.is_some_and(|roots| roots > 0) && rendered_text_chars == 0;
    if selector_no_text {
        warnings.push(format!(
            "selector_no_blocks: matched_roots={}",
            matched_roots.unwrap_or_default()
        ));
    }
    if !selector_scoped && !block_truncated && dom_text_chars > 0 {
        let extraction_ratio = rendered_text_chars as f64 / dom_text_chars as f64;
        if extraction_ratio < SPARSE_EXTRACTION_THRESHOLD {
            warnings.push(format!(
                "extraction_sparse: text_chars={rendered_text_chars} dom_text_chars={dom_text_chars} ratio={extraction_ratio:.3}"
            ));
        }
    }

    // After truncation, prune outline entries whose `block_index`
    // points beyond the retained block list. Heading blocks that
    // were dropped by the budget would leave stale index references
    // otherwise. Title-derived fallback entries (block_index = None)
    // are emitted later in `FetchClient` and are unaffected here.
    prune_outline_to_blocks(&mut outline, blocks.len());

    // If we truncated, emit a warning.
    if block_truncated {
        warnings.push("content truncated at block boundary".to_string());
    }

    let text_truncated = block_truncated;

    (
        title,
        description,
        RenderedBlocks {
            blocks,
            outline,
            text_truncated,
            block_truncated,
        },
        warnings,
        non_utf8,
        matched_roots,
    )
}

pub(crate) fn truncate_blocks_at_budget(blocks: &mut Vec<RenderedBlock>, budget: usize) -> bool {
    let mut char_budget = budget;
    let mut block_truncated = false;
    let mut last_valid = blocks.len();
    for (i, block) in blocks.iter().enumerate() {
        let block_chars = block.text.chars().count();
        if block_chars <= char_budget {
            char_budget -= block_chars;
        } else {
            // Budget exhausted. If the block is a code block, try to
            // snap to the nearest newline boundary within the budget.
            if block.kind == BlockKind::Code && char_budget > 0 {
                let truncated: String = block.text.chars().take(char_budget).collect();
                if let Some(last_nl) = truncated.rfind('\n') {
                    // Snap to line boundary if we have at least a few
                    // chars of the line; otherwise keep what we have.
                    if last_nl > char_budget / 2 {
                        blocks[i].text = truncated[..last_nl].to_string();
                        last_valid = i + 1;
                        block_truncated = true;
                        break;
                    }
                }
                blocks[i].text = truncated;
                last_valid = i + 1;
                block_truncated = true;
                break;
            }
            last_valid = i;
            block_truncated = true;
            break;
        }
    }
    blocks.truncate(last_valid);
    block_truncated
}

fn decode_html(html: &[u8]) -> (String, Vec<String>, bool) {
    match std::str::from_utf8(html) {
        Ok(s) => (s.to_string(), Vec::new(), false),
        Err(_) => {
            let decoded = String::from_utf8_lossy(html).into_owned();
            (
                decoded,
                vec!["body is not valid UTF-8; extraction may be incomplete".to_string()],
                true,
            )
        }
    }
}

fn extract_title(document: &Html) -> Option<String> {
    Selector::parse("title")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.text().next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn extract_description(document: &Html) -> Option<String> {
    Selector::parse(r#"meta[name="description"]"#)
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Minimum number of rendered blocks required for a content root to
/// be considered useful.  A root that produces zero blocks is always
/// treated as sparse.
const MIN_BLOCKS: usize = 1;

/// Minimum total character count across all rendered blocks for a
/// content root to be considered useful.  This catches the case
/// where a `<main>` element exists but only contains whitespace or
/// a tiny placeholder.
const MIN_CHARS: usize = 50;

/// Probe a candidate content root by rendering it into a temporary
/// buffer and returning `(block_count, total_chars)`.  The
/// `warnings` and `outline` accumulators are supplied so that
/// `walk_element` can push into them without extra plumbing — but
/// callers should discard these after probing.
fn probe_root<'a>(
    element: ElementRef<'a>,
    blocks: &mut Vec<RenderedBlock>,
    outline: &mut Vec<DocumentOutlineEntry>,
    base_url: &str,
    warnings: &mut Vec<String>,
    markdown: bool,
) -> (usize, usize) {
    blocks.clear();
    outline.clear();
    walk_element(
        element,
        blocks,
        outline,
        base_url,
        warnings,
        markdown,
        WalkOptions {
            depth: 0,
            matches: None,
        },
    );
    let total_chars: usize = blocks.iter().map(|b| b.text.chars().count()).sum();
    (blocks.len(), total_chars)
}

/// Select the content root for the page.
///
/// Candidates are tried in priority order: `main`, `article`,
/// `[role=main]`, `body`.  The first candidate that produces at
/// least [`MIN_BLOCKS`] blocks **and** at least [`MIN_CHARS`] chars
/// of useful text is returned.  If every explicit candidate is
/// sparse, `body` is returned as the ultimate fallback (it is
/// always non-empty in a valid HTML document).
fn select_content_root<'a>(document: &'a Html) -> ElementRef<'a> {
    let selectors = ["main", "article", "[role=main]", "body"];

    // Collect candidate elements in priority order.
    let mut candidates: Vec<ElementRef<'a>> = Vec::new();
    for sel_str in &selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = document.select(&sel).next() {
                candidates.push(el);
            }
        }
    }

    // Probe each candidate; return the first one with enough content.
    let mut probe_blocks = Vec::new();
    let mut probe_outline = Vec::new();
    let mut probe_warnings = Vec::new();
    let mut last_candidate = document.root_element();

    for candidate in &candidates {
        last_candidate = *candidate;
        let (block_count, total_chars) = probe_root(
            *candidate,
            &mut probe_blocks,
            &mut probe_outline,
            "",
            &mut probe_warnings,
            false,
        );
        probe_warnings.clear();
        if block_count >= MIN_BLOCKS && total_chars >= MIN_CHARS {
            return *candidate;
        }
    }

    // All candidates were sparse — fall back to the last one (body
    // or root_element).
    last_candidate
}

fn should_skip(elem: &ElementRef) -> bool {
    let tag = elem.value().name();
    if SKIP_TAGS.contains(&tag) {
        return true;
    }
    if elem.value().attr("hidden").is_some() {
        return true;
    }
    if elem.value().attr("aria-hidden") == Some("true") {
        return true;
    }
    false
}

#[derive(Clone, Copy)]
struct WalkOptions<'a> {
    depth: usize,
    matches: Option<&'a crate::fetch::scope::SelectorMatches>,
}

fn walk_element(
    element: ElementRef,
    blocks: &mut Vec<RenderedBlock>,
    outline: &mut Vec<DocumentOutlineEntry>,
    base_url: &str,
    warnings: &mut Vec<String>,
    markdown: bool,
    options: WalkOptions<'_>,
) {
    let depth = options.depth;
    let matches = options.matches;
    if depth >= super::super::MAX_TREE_WALK_DEPTH || is_excluded(&element, matches) {
        return;
    }
    let mut text_run = String::new();
    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            text_run.push_str(text);
            continue;
        }
        let Some(child_elem) = ElementRef::wrap(child) else {
            continue;
        };
        push_paragraph_text(&text_run, blocks);
        text_run.clear();
        walk_node(
            child_elem, blocks, outline, base_url, warnings, markdown, options,
        );
    }
    push_paragraph_text(&text_run, blocks);
}

fn push_paragraph_text(text: &str, blocks: &mut Vec<RenderedBlock>) {
    let text = crate::core::sanitize::normalize_whitespace(text)
        .trim()
        .to_string();
    if !text.is_empty() {
        blocks.push(RenderedBlock {
            kind: BlockKind::Paragraph,
            text,
            level: None,
            anchor: None,
            language: None,
            line_start: None,
            line_end: None,
            page: None,
        });
    }
}

fn bounded_dom_text_chars(
    root: ElementRef<'_>,
    matches: Option<&crate::fetch::scope::SelectorMatches>,
) -> usize {
    let mut text = String::new();
    let mut collected_chars = 0;
    collect_dom_text_bounded(*root, &mut text, &mut collected_chars, matches, 0);
    crate::core::sanitize::normalize_whitespace(&text)
        .chars()
        .count()
}

fn has_matched_ancestor(
    element: ElementRef<'_>,
    roots: &std::collections::HashSet<ego_tree::NodeId>,
) -> bool {
    let mut current = *element;
    while let Some(parent) = current.parent() {
        if roots.contains(&parent.id()) {
            return true;
        }
        current = parent;
    }
    false
}

fn collect_dom_text_bounded(
    node: NodeRef<'_, scraper::Node>,
    text: &mut String,
    collected_chars: &mut usize,
    matches: Option<&crate::fetch::scope::SelectorMatches>,
    depth: usize,
) {
    if depth >= super::super::MAX_TREE_WALK_DEPTH || *collected_chars >= MAX_DOM_TEXT_CHARS {
        return;
    }
    if let Some(value) = node.value().as_text() {
        for ch in value.chars() {
            if *collected_chars >= MAX_DOM_TEXT_CHARS {
                break;
            }
            text.push(ch);
            *collected_chars += 1;
        }
    } else if let Some(element) = ElementRef::wrap(node) {
        if should_skip(&element) || is_excluded(&element, matches) {
            return;
        }
        for child in node.children() {
            if *collected_chars >= MAX_DOM_TEXT_CHARS {
                break;
            }
            collect_dom_text_bounded(child, text, collected_chars, matches, depth + 1);
        }
    }
}

fn walk_node(
    element: ElementRef,
    blocks: &mut Vec<RenderedBlock>,
    outline: &mut Vec<DocumentOutlineEntry>,
    base_url: &str,
    warnings: &mut Vec<String>,
    markdown: bool,
    options: WalkOptions<'_>,
) {
    let depth = options.depth;
    let matches = options.matches;
    if depth >= super::super::MAX_TREE_WALK_DEPTH
        || should_skip(&element)
        || is_excluded(&element, matches)
    {
        return;
    }
    let node = *element;
    let tag = element.value().name();
    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag[1..].parse().unwrap_or(1);
            let text = collect_inline_text(node, markdown, base_url, matches);
            let anchor = element.value().id().map(|s| s.to_string()).or_else(|| {
                if text.is_empty() {
                    None
                } else {
                    Some(make_slug(&text))
                }
            });
            let block_index = blocks.len();
            blocks.push(RenderedBlock {
                kind: BlockKind::Heading,
                text: String::new(),
                level: Some(level),
                anchor: anchor.clone(),
                language: None,
                line_start: None,
                line_end: None,
                page: None,
            });
            blocks[block_index].text = text;
            outline.push(DocumentOutlineEntry {
                level,
                title: blocks[block_index].text.clone(),
                anchor,
                block_index: Some(block_index),
                page: None,
            });
        }
        "p" => {
            let text = collect_inline_text(node, markdown, base_url, matches);
            if !text.is_empty() {
                blocks.push(RenderedBlock {
                    kind: BlockKind::Paragraph,
                    text,
                    level: None,
                    anchor: None,
                    language: None,
                    line_start: None,
                    line_end: None,
                    page: None,
                });
            }
        }
        "pre" => {
            let text = collect_raw_text(node, matches);
            let language = detect_language_from_pre(&element);
            blocks.push(RenderedBlock {
                kind: BlockKind::Code,
                text,
                level: None,
                anchor: None,
                language,
                line_start: None,
                line_end: None,
                page: None,
            });
        }
        "table" => {
            let (text, irregular) = render_table_text(&element, matches);
            if irregular {
                warnings.push(
                    "table has irregular row lengths; rendering may be incomplete".to_string(),
                );
            }
            blocks.push(RenderedBlock {
                kind: BlockKind::Table,
                text,
                level: None,
                anchor: None,
                language: None,
                line_start: None,
                line_end: None,
                page: None,
            });
        }
        "blockquote" => {
            let text = collect_inline_text(node, markdown, base_url, matches);
            if !text.is_empty() {
                blocks.push(RenderedBlock {
                    kind: BlockKind::BlockQuote,
                    text,
                    level: None,
                    anchor: None,
                    language: None,
                    line_start: None,
                    line_end: None,
                    page: None,
                });
            }
        }
        "dl" => render_definition_list(&element, blocks, base_url, markdown, matches),
        "hr" => blocks.push(RenderedBlock {
            kind: BlockKind::HorizontalRule,
            text: String::new(),
            level: None,
            anchor: None,
            language: None,
            line_start: None,
            line_end: None,
            page: None,
        }),
        "ul" | "ol" => render_list(&element, blocks, base_url, markdown, 0, matches),
        _ => walk_element(
            element,
            blocks,
            outline,
            base_url,
            warnings,
            markdown,
            WalkOptions {
                depth: depth + 1,
                ..options
            },
        ),
    }
}

fn is_excluded(
    element: &ElementRef,
    matches: Option<&crate::fetch::scope::SelectorMatches>,
) -> bool {
    matches.is_some_and(|matches| matches.excluded.contains(&element.id()))
}

/// Collect inline text from a node, normalizing whitespace.
/// When `markdown` is true, `<code>` is wrapped in backticks and
/// `<a>` elements produce `[text](href)` Markdown links.
fn collect_inline_text<'a>(
    node: NodeRef<'a, scraper::Node>,
    markdown: bool,
    base_url: &str,
    matches: Option<&crate::fetch::scope::SelectorMatches>,
) -> String {
    let mut parts = Vec::new();
    collect_text_parts(node, &mut parts, markdown, base_url, matches, 0);
    let text = parts.join("");
    crate::core::sanitize::normalize_whitespace(&text)
        .trim()
        .to_string()
}
fn collect_text_parts<'a>(
    node: NodeRef<'a, scraper::Node>,
    parts: &mut Vec<String>,
    markdown: bool,
    base_url: &str,
    matches: Option<&crate::fetch::scope::SelectorMatches>,
    depth: usize,
) {
    if depth >= super::super::MAX_TREE_WALK_DEPTH {
        return;
    }
    if let Some(text) = node.value().as_text() {
        if !text.is_empty() {
            parts.push(text.to_string());
        }
    } else if let Some(elem) = ElementRef::wrap(node) {
        if should_skip(&elem) || is_excluded(&elem, matches) {
            return;
        }
        let tag = elem.value().name();
        if markdown && tag == "code" {
            let raw_inner = collect_raw_text(node, matches);
            let inner = raw_inner.trim();
            let rendered = (!inner.is_empty()).then(|| format!("\x60{inner}\x60"));
            push_inline_part_with_source_boundaries(parts, &raw_inner, rendered);
            return;
        }
        if markdown && tag == "a" {
            let href = elem.value().attr("href").unwrap_or("");
            let raw_link_text = collect_raw_text(node, matches);
            let link_text = raw_link_text.trim();
            let rendered = if link_text.is_empty() || href.is_empty() {
                None
            } else {
                let resolved = resolve_url(href, base_url);
                Some(format!("[{link_text}]({resolved})"))
            };
            push_inline_part_with_source_boundaries(parts, &raw_link_text, rendered);
            return;
        }
        for child in node.children() {
            collect_text_parts(child, parts, markdown, base_url, matches, depth + 1);
        }
    }
}
fn push_inline_part_with_source_boundaries(
    parts: &mut Vec<String>,
    source: &str,
    rendered: Option<String>,
) {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        if !source.is_empty() {
            parts.push(source.to_string());
        }
        return;
    }

    let leading_len = source.len() - source.trim_start().len();
    if leading_len > 0 {
        parts.push(source[..leading_len].to_string());
    }
    parts.push(rendered.unwrap_or_else(|| trimmed.to_string()));
    let trailing_start = source.trim_end().len();
    if trailing_start < source.len() {
        parts.push(source[trailing_start..].to_string());
    }
}

/// Collect raw text from a node, preserving whitespace (for code blocks).
fn collect_raw_text<'a>(
    node: NodeRef<'a, scraper::Node>,
    matches: Option<&crate::fetch::scope::SelectorMatches>,
) -> String {
    let mut text = String::new();
    collect_raw_text_inner(node, &mut text, 0, matches);
    text
}

fn collect_raw_text_inner<'a>(
    node: NodeRef<'a, scraper::Node>,
    text: &mut String,
    depth: usize,
    matches: Option<&crate::fetch::scope::SelectorMatches>,
) {
    if depth >= super::super::MAX_TREE_WALK_DEPTH {
        return;
    }
    if let Some(t) = node.value().as_text() {
        text.push_str(t);
    } else if let Some(elem) = ElementRef::wrap(node) {
        if should_skip(&elem) || is_excluded(&elem, matches) {
            return;
        }
        for child in node.children() {
            collect_raw_text_inner(child, text, depth + 1, matches);
        }
    }
}
fn detect_language_from_pre(pre: &ElementRef) -> Option<String> {
    if let Ok(code_sel) = Selector::parse("code") {
        if let Some(code_el) = pre.select(&code_sel).next() {
            if let Some(lang) = detect_language(code_el) {
                return Some(lang);
            }
        }
    }
    detect_language(*pre)
}

fn detect_language(elem: ElementRef) -> Option<String> {
    let classes = elem.value().attr("class").unwrap_or("");
    for class in classes.split_whitespace() {
        let lang_opt = class
            .strip_prefix("language-")
            .or_else(|| class.strip_prefix("lang-"));
        if let Some(lang) = lang_opt {
            let lang = lang.trim();
            if !lang.is_empty() {
                return Some(normalize_language(lang));
            }
        }
    }
    None
}

fn render_list(
    list: &ElementRef,
    blocks: &mut Vec<RenderedBlock>,
    base_url: &str,
    markdown: bool,
    depth: usize,
    matches: Option<&crate::fetch::scope::SelectorMatches>,
) {
    if depth >= super::super::MAX_TREE_WALK_DEPTH {
        return;
    }
    for child in list.children() {
        if let Some(li) = ElementRef::wrap(child) {
            if li.value().name() == "li" && !is_excluded(&li, matches) {
                let text = collect_inline_text(child, markdown, base_url, matches);
                if !text.is_empty() {
                    blocks.push(RenderedBlock {
                        kind: BlockKind::ListItem,
                        text,
                        level: None,
                        anchor: None,
                        language: None,
                        line_start: None,
                        line_end: None,
                        page: None,
                    });
                }
                for nested_child in li.children() {
                    if let Some(nested_elem) = ElementRef::wrap(nested_child) {
                        let tag = nested_elem.value().name();
                        if (tag == "ul" || tag == "ol") && !is_excluded(&nested_elem, matches) {
                            render_list(
                                &nested_elem,
                                blocks,
                                base_url,
                                markdown,
                                depth + 1,
                                matches,
                            );
                        }
                    }
                }
            }
        }
    }
}
fn render_table_text(
    table: &ElementRef,
    matches: Option<&crate::fetch::scope::SelectorMatches>,
) -> (String, bool) {
    let mut rows = Vec::new();
    if let Ok(tr_sel) = Selector::parse("tr") {
        for tr in table.select(&tr_sel) {
            if is_excluded(&tr, matches) {
                continue;
            }
            let mut cells = Vec::new();
            for child in tr.children() {
                if let Some(cell) = ElementRef::wrap(child) {
                    let tag = cell.value().name();
                    if (tag == "td" || tag == "th") && !is_excluded(&cell, matches) {
                        cells.push(collect_inline_text(*cell, false, "", matches));
                    }
                }
            }
            if !cells.is_empty() {
                rows.push(cells);
            }
        }
    }
    if rows.is_empty() {
        return (collect_inline_text(**table, false, "", matches), false);
    }
    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let irregular = rows.iter().any(|r| r.len() != max_cols);
    let mut lines = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let mut line = String::from("|");
        for cell in row {
            line.push_str(&format!(" {} |", cell.replace('|', "\\|")));
        }
        for _ in row.len()..max_cols {
            line.push_str(" |");
        }
        lines.push(line);
        if i == 0 {
            let sep = " --- |".repeat(max_cols);
            lines.push(format!("|{sep}"));
        }
    }
    (lines.join("\n"), irregular)
}
fn render_definition_list(
    dl: &ElementRef,
    blocks: &mut Vec<RenderedBlock>,
    base_url: &str,
    markdown: bool,
    matches: Option<&crate::fetch::scope::SelectorMatches>,
) {
    let mut current_term = String::new();
    for child in dl.children() {
        if let Some(elem) = ElementRef::wrap(child) {
            if is_excluded(&elem, matches) {
                continue;
            }
            let tag = elem.value().name();
            match tag {
                "dt" => current_term = collect_inline_text(child, markdown, base_url, matches),
                "dd" => {
                    let definition = collect_inline_text(child, markdown, base_url, matches);
                    let text = if current_term.is_empty() {
                        definition
                    } else if definition.is_empty() {
                        current_term.clone()
                    } else {
                        format!("{current_term}: {definition}")
                    };
                    if !text.is_empty() {
                        blocks.push(RenderedBlock {
                            kind: BlockKind::Definition,
                            text,
                            level: None,
                            anchor: None,
                            language: None,
                            line_start: None,
                            line_end: None,
                            page: None,
                        });
                    }
                }
                _ => {}
            }
        }
    }
}
fn make_slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c)
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Prune outline entries whose `block_index` points beyond the
/// retained `blocks` list.
///
/// Called immediately after block-boundary truncation so headings
/// whose blocks were dropped by the budget do not leave stale index
/// references. Entries with `block_index = None` are retained as-is;
/// those are emitted later by `FetchClient` as title-derived
/// fallbacks and do not need block-relative validation.
fn prune_outline_to_blocks(outline: &mut Vec<DocumentOutlineEntry>, blocks_len: usize) {
    outline.retain(|entry| match entry.block_index {
        Some(i) => i < blocks_len,
        None => true,
    });
}

/// Resolve a possibly-relative URL against a base URL.
fn resolve_url(href: &str, base_url: &str) -> String {
    // If the href is already absolute, return it directly.
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    if let Ok(base) = Url::parse(base_url) {
        if let Ok(resolved) = base.join(href) {
            return resolved.to_string();
        }
    }
    href.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_blocks_extracts_title() {
        let html = b"<!DOCTYPE html><html><head><title>My Page</title></head><body><p>content</p></body></html>";
        let (title, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        assert_eq!(title, Some("My Page".to_string()));
        assert!(!rendered.blocks.is_empty());
    }

    #[test]
    fn render_blocks_extracts_description() {
        let html = b"<!DOCTYPE html><html><head><meta name=\"description\" content=\"A test page\"></head><body></body></html>";
        let (_, desc, _, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        assert_eq!(desc, Some("A test page".to_string()));
    }

    #[test]
    fn render_blocks_creates_heading_blocks() {
        let html = b"<!DOCTYPE html><html><body><h1>Title</h1><p>text</p></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        assert!(rendered.blocks.len() >= 2);
        assert_eq!(rendered.blocks[0].kind, BlockKind::Heading);
        assert_eq!(rendered.blocks[0].text, "Title");
        assert_eq!(rendered.blocks[0].level, Some(1));
    }

    #[test]
    fn render_blocks_creates_paragraph_blocks() {
        let html = b"<!DOCTYPE html><html><body><p>hello</p><p>world</p></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        assert_eq!(rendered.blocks.len(), 2);
        assert!(rendered
            .blocks
            .iter()
            .all(|b| b.kind == BlockKind::Paragraph));
    }

    #[test]
    fn render_blocks_skips_script_and_style() {
        let html = b"<!DOCTYPE html><html><body><p>visible</p><script>alert('x')</script><style>body{}</style><p>also visible</p></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        let text: String = rendered
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("visible"));
        assert!(text.contains("also visible"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("body{}"));
    }

    #[test]
    fn render_blocks_skips_nav_footer_header_aside() {
        let html = b"<!DOCTYPE html><html><body><header>top</header><nav>links</nav><main><p>content</p></main><aside>side</aside><footer>bottom</footer></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        let text: String = rendered
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("content"));
        assert!(!text.contains("top"));
        assert!(!text.contains("links"));
        assert!(!text.contains("side"));
        assert!(!text.contains("bottom"));
    }

    #[test]
    fn render_blocks_populates_outline() {
        let html =
            b"<!DOCTYPE html><html><body><h1>Intro</h1><h2>Details</h2><p>text</p></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        assert_eq!(rendered.outline.len(), 2);
        assert_eq!(rendered.outline[0].level, 1);
        assert_eq!(rendered.outline[0].title, "Intro");
        assert_eq!(rendered.outline[1].level, 2);
        assert_eq!(rendered.outline[1].title, "Details");
    }

    #[test]
    fn render_blocks_code_block_detects_language() {
        let html = b"<!DOCTYPE html><html><body><pre><code class=\"language-rust\">fn main() {}</code></pre></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        assert_eq!(rendered.blocks[0].kind, BlockKind::Code);
        assert_eq!(rendered.blocks[0].language, Some("rust".to_string()));
    }

    #[test]
    fn render_blocks_text_truncation() {
        let html = b"<!DOCTYPE html><html><body><p>short</p></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 3, false);
        assert!(rendered.text_truncated);
    }

    #[test]
    fn render_blocks_no_truncation_within_limit() {
        let html = b"<!DOCTYPE html><html><body><p>short</p></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 1000, false);
        assert!(!rendered.text_truncated);
    }

    #[test]
    fn make_slug_basic() {
        assert_eq!(make_slug("Hello World"), "hello-world");
        assert_eq!(make_slug("  Lots  of   Spaces  "), "lots-of-spaces");
        assert_eq!(make_slug("Special!@#Chars"), "specialchars");
    }

    #[test]
    fn non_utf8_body_emits_warning() {
        let html: &[u8] = b"<html><body><p>before</p>\xff\xfe<p>after</p></body></html>";
        let (_, _, _, warnings, non_utf8) =
            render_blocks(html, "https://example.com/", 10000, false);
        assert!(non_utf8);
        assert!(!warnings.is_empty());
    }

    #[test]
    fn valid_utf8_no_warnings() {
        let html = b"<!DOCTYPE html><html><body><p>hello</p></body></html>";
        let (_, _, _, warnings, non_utf8) =
            render_blocks(html, "https://example.com/", 10000, false);
        assert!(!non_utf8);
        assert!(warnings.is_empty());
    }

    #[test]
    fn render_blocks_code_whitespace_preserved() {
        let html = b"<pre><code>line1\n  line2\n    line3</code></pre>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        assert_eq!(rendered.blocks.len(), 1);
        assert_eq!(rendered.blocks[0].kind, BlockKind::Code);
        assert_eq!(rendered.blocks[0].text, "line1\n  line2\n    line3");
    }

    #[test]
    fn render_blocks_regular_table() {
        let html = b"<table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>";
        let (_, _, rendered, warnings, _) =
            render_blocks(html, "https://example.com/", 10000, false);
        assert_eq!(rendered.blocks.len(), 1);
        assert_eq!(rendered.blocks[0].kind, BlockKind::Table);
        assert!(rendered.blocks[0].text.contains("| A | B |"));
        assert!(rendered.blocks[0].text.contains("| 1 | 2 |"));
        assert!(rendered.blocks[0].text.contains("---"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn render_blocks_escapes_table_cell_pipes() {
        let html = b"<table><tr><th>A|B</th></tr><tr><td>C|D</td></tr></table>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        assert!(rendered.blocks[0].text.contains("| A\\|B |"));
        assert!(rendered.blocks[0].text.contains("| C\\|D |"));
    }

    #[test]
    fn render_blocks_irregular_table_warns() {
        let html =
            b"<table><tr><th>A</th><th>B</th><th>C</th></tr><tr><td>1</td><td>2</td></tr></table>";
        let (_, _, rendered, warnings, _) =
            render_blocks(html, "https://example.com/", 10000, false);
        assert_eq!(rendered.blocks.len(), 1);
        assert_eq!(rendered.blocks[0].kind, BlockKind::Table);
        assert!(warnings.iter().any(|w| w.contains("irregular row lengths")));
    }

    #[test]
    fn render_blocks_blockquote() {
        let html = b"<blockquote><p>quoted text</p></blockquote>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        assert_eq!(rendered.blocks.len(), 1);
        assert_eq!(rendered.blocks[0].kind, BlockKind::BlockQuote);
        assert!(rendered.blocks[0].text.contains("quoted text"));
    }

    #[test]
    fn render_blocks_inline_code_markdown() {
        let html = b"<p>Use <code>fn main()</code> to start.</p>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, true);
        assert_eq!(rendered.blocks.len(), 1);
        assert!(rendered.blocks[0].text.contains("`fn main()`"));
        assert!(rendered.blocks[0].text.contains("Use"));
        assert!(rendered.blocks[0].text.contains("to start"));
    }

    #[test]
    fn render_blocks_inline_code_text_mode_no_backticks() {
        let html = b"<p>Use <code>fn main()</code> to start.</p>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        assert_eq!(rendered.blocks.len(), 1);
        assert!(rendered.blocks[0].text.contains("fn main()"));
        assert!(!rendered.blocks[0].text.contains("`"));
    }

    #[test]
    fn render_blocks_inline_link_markdown() {
        let html = b"<p>See <a href=\"/docs/start\">the docs</a> for more.</p>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/page", 10000, true);
        assert_eq!(rendered.blocks.len(), 1);
        assert!(rendered.blocks[0]
            .text
            .contains("[the docs](https://example.com/docs/start)"));
    }

    #[test]
    fn render_blocks_inline_link_absolute_url() {
        let html = "<p>Visit <a href=\"https://other.com/x\">other site</a>.</p>".as_bytes();
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, true);
        assert!(rendered.blocks[0]
            .text
            .contains("[other site](https://other.com/x)"));
    }

    #[test]
    fn inline_whitespace_preserves_source_boundaries_exactly() {
        let cases = [
            (
                "<p>Before   <a href=\"/docs\"><em>linked</em> text</a> \n then <code>sample</code> after.</p>",
                "Before linked text then sample after.",
                "Before [linked text](https://example.com/docs) then `sample` after.",
            ),
            (
                "<p>See<a href=\"/docs\"><strong>this</strong></a><code>x</code>,next!</p>",
                "Seethisx,next!",
                "See[this](https://example.com/docs)`x`,next!",
            ),
            (
                "<p>Before<a href=\"/docs\"> linked </a>after</p>",
                "Before linked after",
                "Before [linked](https://example.com/docs) after",
            ),
            (
                "<p>Before<code> sample </code>after</p>",
                "Before sample after",
                "Before `sample` after",
            ),
        ];

        for (html, text_expected, markdown_expected) in cases {
            let (_, _, text_rendered, _, _) =
                render_blocks(html.as_bytes(), "https://example.com/page", 10000, false);
            let (_, _, markdown_rendered, _, _) =
                render_blocks(html.as_bytes(), "https://example.com/page", 10000, true);

            assert_eq!(text_rendered.blocks[0].text, text_expected);
            assert_eq!(markdown_rendered.blocks[0].text, markdown_expected);
        }
    }

    #[test]
    fn empty_main_falls_back_to_body() {
        let html = b"<!DOCTYPE html><html><head><title>Sparse Main</title></head><body><main></main><p>Body content that should be visible and is long enough to pass the threshold.</p></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        let text: String = rendered
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            text.contains("Body content"),
            "expected body content, got: {text}"
        );
        assert!(!rendered.blocks.is_empty());
    }

    #[test]
    fn non_empty_main_preferred_over_noisy_body() {
        let html = b"<!DOCTYPE html><html><body><main><h1>Article Title</h1><p>Main article content that is substantive and should be preferred.</p></main><p>Footer noise that should be ignored when main is selected.</p></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        let text: String = rendered
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("Article Title"), "should prefer main: {text}");
        assert!(
            text.contains("Main article content"),
            "should include main body: {text}"
        );
        assert!(
            !text.contains("Footer noise"),
            "should not include body noise when main is rich: {text}"
        );
    }

    #[test]
    fn tiny_main_falls_back_to_body() {
        let html = b"<!DOCTYPE html><html><body><main>.</main><p>Substantial body content that provides real useful information and is well beyond the fifty character minimum threshold.</p></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        let text: String = rendered
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            text.contains("Substantial body content"),
            "expected body fallback, got: {text}"
        );
    }

    #[test]
    fn body_only_page_still_works() {
        let html = b"<!DOCTYPE html><html><body><h1>Page Title</h1><p>Paragraph one.</p><p>Paragraph two.</p></body></html>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10000, false);
        let text: String = rendered
            .blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("Page Title"));
        assert!(text.contains("Paragraph one."));
        assert!(text.contains("Paragraph two."));
    }

    #[test]
    fn render_blocks_block_boundary_truncation() {
        // Two paragraphs: "aaa" (3 chars) + "bbb" (3 chars). Truncate at 4 chars.
        // Should keep first block (3 chars) and drop second (would exceed budget).
        let html = b"<p>aaa</p><p>bbb</p>";
        let (_, _, rendered, warnings, _) = render_blocks(html, "https://example.com/", 4, false);
        assert!(rendered.block_truncated);
        assert!(rendered.text_truncated);
        assert!(warnings
            .iter()
            .any(|w| w.contains("truncated at block boundary")));
        assert_eq!(rendered.blocks.len(), 1);
        assert_eq!(rendered.blocks[0].text, "aaa");
    }

    #[test]
    fn render_blocks_keeps_partial_code_block_when_snap_is_early() {
        let html = b"<pre><code>a\nlong code line</code></pre>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 8, false);

        assert!(rendered.block_truncated);
        assert_eq!(rendered.blocks.len(), 1);
        assert_eq!(rendered.blocks[0].text, "a\nlong c");
    }

    // --- Outline pruning after block-boundary truncation ---

    #[test]
    fn render_blocks_outline_indexes_in_range_when_no_truncation() {
        // Test A from the final micro-closure plan: valid retained
        // outline. With a generous budget, all blocks and outline
        // entries survive; every block_index is in bounds.
        let html = b"<h1>One</h1><p>first</p><h2>Two</h2><p>second</p>";
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 10_000, false);
        assert!(!rendered.block_truncated);
        assert!(!rendered.outline.is_empty());
        for entry in &rendered.outline {
            if let Some(idx) = entry.block_index {
                assert!(
                    idx < rendered.blocks.len(),
                    "outline entry {:?} has block_index {} >= blocks.len() {}",
                    entry.title,
                    idx,
                    rendered.blocks.len()
                );
            }
        }
    }

    #[test]
    fn render_blocks_outline_pruned_after_truncation() {
        // Test B from the final micro-closure plan: low max_chars keeps
        // the first heading but drops later heading blocks. The dropped
        // heading's outline entry must be removed so it cannot point
        // beyond the truncated block list.
        let html = b"<h1>Keep</h1><p>some text</p><h2>Drop</h2><p>more text here</p>";
        // Budget ~ 11 chars: heading text "Keep" + paragraph "some text"
        // (10 chars body) -> enough for first heading + first paragraph
        // but not the second heading block.
        let (_, _, rendered, _, _) = render_blocks(html, "https://example.com/", 12, false);

        assert!(
            rendered.block_truncated || rendered.text_truncated,
            "expected truncation flags, got blocks={:?} outline={:?}",
            rendered.blocks,
            rendered.outline
        );

        // Every retained outline entry must have a valid block_index.
        for entry in &rendered.outline {
            if let Some(idx) = entry.block_index {
                assert!(
                    idx < rendered.blocks.len(),
                    "outline entry {:?} has stale block_index {} (blocks.len() = {})",
                    entry.title,
                    idx,
                    rendered.blocks.len()
                );
            }
        }

        // The removed heading's title must not be present.
        let titles: Vec<&str> = rendered.outline.iter().map(|e| e.title.as_str()).collect();
        assert!(
            !titles.contains(&"Drop"),
            "dropped heading should not appear in outline, got: {titles:?}"
        );
        assert!(
            titles.contains(&"Keep"),
            "retained heading should appear in outline, got: {titles:?}"
        );
    }

    #[test]
    fn render_blocks_outline_pruning_helper_directly() {
        // Direct unit test for the pruning helper covering all branches.
        let mut outline = vec![
            DocumentOutlineEntry {
                level: 1,
                title: "in-range".to_string(),
                anchor: None,
                block_index: Some(0),
                page: None,
            },
            DocumentOutlineEntry {
                level: 2,
                title: "boundary".to_string(),
                anchor: None,
                block_index: Some(1),
                page: None,
            },
            DocumentOutlineEntry {
                level: 2,
                title: "out-of-range".to_string(),
                anchor: None,
                block_index: Some(5),
                page: None,
            },
            DocumentOutlineEntry {
                level: 1,
                title: "fallback".to_string(),
                anchor: None,
                block_index: None,
                page: None,
            },
        ];
        prune_outline_to_blocks(&mut outline, 2);
        let titles: Vec<&str> = outline.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["in-range", "boundary", "fallback"],
            "only entries with block_index < blocks.len() should survive"
        );
    }

    #[test]
    fn render_blocks_deeply_nested_html_does_not_overflow() {
        let depth = 5_000;
        let mut html = String::from("<!DOCTYPE html><html><body>");
        for _ in 0..depth {
            html.push_str("<div>");
        }
        html.push_str("<p>deep content</p>");
        for _ in 0..depth {
            html.push_str("</div>");
        }
        html.push_str("</body></html>");

        let (_, _, rendered, _, _) =
            render_blocks(html.as_bytes(), "https://example.com/", 10_000, false);
        assert!(
            rendered.blocks.is_empty(),
            "content beyond the depth cap must not be walked"
        );
    }

    #[test]
    fn render_blocks_deeply_nested_inline_text_does_not_overflow() {
        let depth = 5_000;
        let mut html = String::from("<!DOCTYPE html><html><body><p>");
        for _ in 0..depth {
            html.push_str("<span>");
        }
        html.push_str("deep inline");
        for _ in 0..depth {
            html.push_str("</span>");
        }
        html.push_str("</p></body></html>");

        let (_, _, rendered, _, _) =
            render_blocks(html.as_bytes(), "https://example.com/", 10_000, false);
        assert!(
            rendered.blocks.is_empty(),
            "inline text beyond the depth cap must not be collected"
        );
    }
}
