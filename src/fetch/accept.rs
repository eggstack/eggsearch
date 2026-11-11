//! Content-negotiation policy for fetch targets.
//!
//! Every fetch sends one fixed `Accept` policy. HTML-ish targets ask for
//! Markdown first (`md1`); targets whose representation is known ahead of
//! time keep the plain HTML policy (`html1`). The token is part of the raw
//! cache key so a Markdown body and an HTML body can never share an entry.

use crate::core::code_host_fetch::resolve_code_host_fetch_target;
use crate::core::fetch::ExtractMode;

/// `Accept` value that prefers a Markdown representation.
pub const MARKDOWN_ACCEPT_POLICY: &str = "text/markdown;q=1.0, text/html;q=0.9, */*;q=0.1";

/// `Accept` value that asks for the HTML representation.
pub const HTML_ACCEPT_POLICY: &str = "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8";

/// The negotiated representation a fetch asked the origin for.
///
/// The variant names are the cache-key tokens (`md1`, `html1`); `html1` is
/// also the historical no-negotiation behaviour.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum AcceptPolicy {
    /// Plain HTML representation (`html1`).
    #[default]
    Html,
    /// Markdown-first representation (`md1`).
    Markdown,
}

impl AcceptPolicy {
    /// Short versioned cache-key token for this policy.
    pub fn token(self) -> &'static str {
        match self {
            Self::Html => "html1",
            Self::Markdown => "md1",
        }
    }

    /// The `Accept` header value this policy sends.
    pub fn header_value(self) -> &'static str {
        match self {
            Self::Html => HTML_ACCEPT_POLICY,
            Self::Markdown => MARKDOWN_ACCEPT_POLICY,
        }
    }
}

/// Extensions whose target representation is known to be non-HTML, so the
/// Markdown preference would only add a cost or a fallback request.
const NON_HTML_DOCUMENT_EXTENSIONS: &[&str] = &[
    "pdf", "json", "jsonl", "ndjson", "geojson", "xml", "rss", "atom", "yaml", "yml", "toml",
    "csv", "tsv", "ipynb", "diff", "patch", "txt", "md", "mdx", "markdown", "mkd", "rst", "adoc",
    "asciidoc",
];
const NON_HTML_SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "py", "pyi", "js", "mjs", "cjs", "ts", "tsx", "jsx", "go", "c", "h", "cpp", "cc", "cxx",
    "hpp", "java", "kt", "kts", "scala", "sh", "bash", "zsh", "fish", "sql", "lua", "rb", "swift",
    "dart", "ex", "exs", "erl", "hs", "ml", "fs", "clj", "vim", "el", "lisp", "css", "scss",
    "less", "ini", "cfg", "conf", "lock", "log",
];
const NON_HTML_BINARY_EXTENSIONS: &[&str] = &[
    "zip", "tar", "gz", "tgz", "bz2", "xz", "7z", "rar", "dmg", "exe", "msi", "deb", "rpm", "apk",
    "jar", "war", "wasm", "png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "bmp", "tif", "tiff",
    "mp3", "wav", "flac", "ogg", "m4a", "mp4", "mov", "avi", "mkv", "webm", "woff", "woff2", "ttf",
    "otf", "eot", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp",
];

fn is_non_html_extension(extension: &str) -> bool {
    NON_HTML_DOCUMENT_EXTENSIONS.contains(&extension)
        || NON_HTML_SOURCE_EXTENSIONS.contains(&extension)
        || NON_HTML_BINARY_EXTENSIONS.contains(&extension)
}

/// Select the `Accept` policy for one fetch target.
///
/// Unknown or extension-less targets are HTML-ish and get the Markdown
/// policy: that is the happy path where an origin can negotiate. PDF,
/// structured data, source, and media extensions keep the plain HTML
/// policy. Code-host source URLs keep it too, because the fetch is
/// rewritten to a raw file. `metadata_only` keeps it because the page
/// metadata lives in the HTML head.
pub fn select_accept_policy(url: &str, extract_mode: ExtractMode) -> AcceptPolicy {
    if extract_mode == ExtractMode::MetadataOnly {
        return AcceptPolicy::Html;
    }
    if let Some(target) = resolve_code_host_fetch_target(url) {
        if target.raw_url.is_some() {
            return AcceptPolicy::Html;
        }
    }
    match url_target_extension(url) {
        Some(extension) if is_non_html_extension(&extension) => AcceptPolicy::Html,
        _ => AcceptPolicy::Markdown,
    }
}

fn url_target_extension(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let path = parsed.path();
    let last_segment = path.rsplit('/').next()?;
    let (stem, extension) = last_segment.rsplit_once('.')?;
    if stem.is_empty() || extension.is_empty() || extension.len() > 10 {
        return None;
    }
    if !extension.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(extension.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_tokens_and_headers_are_distinct() {
        assert_eq!(AcceptPolicy::Markdown.token(), "md1");
        assert_eq!(AcceptPolicy::Html.token(), "html1");
        assert_eq!(
            AcceptPolicy::Markdown.header_value(),
            "text/markdown;q=1.0, text/html;q=0.9, */*;q=0.1"
        );
        assert_eq!(
            AcceptPolicy::Html.header_value(),
            "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8"
        );
        assert_ne!(
            AcceptPolicy::Markdown.header_value(),
            AcceptPolicy::Html.header_value()
        );
    }

    #[test]
    fn extensionless_and_html_targets_prefer_markdown() {
        for url in [
            "https://example.com/docs/getting-started",
            "https://example.com/",
            "https://example.com/index.html",
            "https://example.com/page.htm",
            "https://example.com/view.php",
            "https://example.com/v1.2/reference",
            "https://example.com/search?q=rust",
        ] {
            assert_eq!(
                select_accept_policy(url, ExtractMode::Text),
                AcceptPolicy::Markdown,
                "{url} should be HTML-ish"
            );
        }
    }

    #[test]
    fn known_non_html_targets_keep_the_html_policy() {
        for url in [
            "https://example.com/paper.pdf",
            "https://example.com/api.json",
            "https://example.com/feed.xml",
            "https://example.com/notes.md",
            "https://example.com/data.csv",
            "https://example.com/src/lib.rs",
            "https://example.com/readme.txt",
            "https://example.com/archive.tar.gz",
        ] {
            assert_eq!(
                select_accept_policy(url, ExtractMode::Text),
                AcceptPolicy::Html,
                "{url} has a known non-HTML representation"
            );
        }
    }

    #[test]
    fn metadata_only_never_negotiates() {
        assert_eq!(
            select_accept_policy("https://example.com/docs/page", ExtractMode::MetadataOnly),
            AcceptPolicy::Html
        );
    }

    #[test]
    fn code_host_source_urls_keep_the_html_policy() {
        let url = "https://github.com/tokio-rs/axum/blob/main/src/lib.rs";
        assert_eq!(
            select_accept_policy(url, ExtractMode::Text),
            AcceptPolicy::Html
        );
    }

    #[test]
    fn unknown_extension_with_dot_prefers_markdown() {
        assert_eq!(
            select_accept_policy("https://example.com/release-1.2", ExtractMode::Text),
            AcceptPolicy::Markdown
        );
    }
}
