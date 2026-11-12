use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::core::identity::{write_entity_prefix, write_str, write_usize, FnvHasher};

pub const CURSOR_PREFIX: &str = "e1.";
pub const MAX_CURSOR_BYTES: usize = 512;
pub const CURSOR_SCHEMA_VERSION: u8 = 1;
pub const MAX_CURSOR_ID_CHARS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorTool {
    WebFetch,
    BatchFetch,
    ResearchSearch,
}

impl CursorTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WebFetch => "web_fetch",
            Self::BatchFetch => "batch_fetch",
            Self::ResearchSearch => "research_search",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CursorPosition {
    pub ord: u32,
    pub id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CursorBudget {
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievalCursor {
    pub v: u8,
    pub t: CursorTool,
    pub k: String,
    pub p: CursorPosition,
    pub d: CursorBudget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorError {
    Invalid,
    Mismatch,
}

impl CursorError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Invalid => "budget_invalid",
            Self::Mismatch => "cursor_mismatch",
        }
    }
}

impl RetrievalCursor {
    pub fn new(
        tool: CursorTool,
        plan_key: &str,
        delivered: usize,
        last_id: &str,
        timeout_ms: u64,
    ) -> Self {
        Self {
            v: CURSOR_SCHEMA_VERSION,
            t: tool,
            k: plan_key.to_string(),
            p: CursorPosition {
                ord: u32::try_from(delivered).unwrap_or(u32::MAX),
                id: last_id.to_string(),
            },
            d: CursorBudget { timeout_ms },
        }
    }

    pub fn encode(&self) -> Result<String, CursorError> {
        let body = serde_json::to_string(self).map_err(|_| CursorError::Invalid)?;
        let encoded = format!("{CURSOR_PREFIX}{}", URL_SAFE_NO_PAD.encode(body.as_bytes()));
        if encoded.len() > MAX_CURSOR_BYTES {
            return Err(CursorError::Invalid);
        }
        Ok(encoded)
    }

    pub fn decode(raw: &str) -> Result<Self, CursorError> {
        if raw.len() > MAX_CURSOR_BYTES || raw.len() <= CURSOR_PREFIX.len() {
            return Err(CursorError::Invalid);
        }
        let body = raw
            .strip_prefix(CURSOR_PREFIX)
            .ok_or(CursorError::Invalid)?;
        if body.contains('=') {
            return Err(CursorError::Invalid);
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| CursorError::Invalid)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| CursorError::Invalid)?;
        let cursor: Self = serde_json::from_str(text).map_err(|_| CursorError::Invalid)?;
        let canonical = serde_json::to_string(&cursor).map_err(|_| CursorError::Invalid)?;
        if canonical != text {
            return Err(CursorError::Invalid);
        }
        cursor.validate()?;
        Ok(cursor)
    }

    pub fn validate(&self) -> Result<(), CursorError> {
        if self.v != CURSOR_SCHEMA_VERSION {
            return Err(CursorError::Invalid);
        }
        if self.k.len() != 16
            || !self
                .k
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(CursorError::Invalid);
        }
        if self.p.id.is_empty() || self.p.id.chars().count() > MAX_CURSOR_ID_CHARS {
            return Err(CursorError::Invalid);
        }
        if !self.p.id.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(CursorError::Invalid);
        }
        if self.d.timeout_ms == 0 {
            return Err(CursorError::Invalid);
        }
        Ok(())
    }

    pub fn require_tool(&self, tool: CursorTool) -> Result<(), CursorError> {
        if self.t == tool {
            Ok(())
        } else {
            Err(CursorError::Mismatch)
        }
    }

    pub fn require_plan_key(&self, plan_key: &str) -> Result<(), CursorError> {
        if self.k == plan_key {
            Ok(())
        } else {
            Err(CursorError::Mismatch)
        }
    }

    pub fn timeout_ms(&self) -> u64 {
        self.d.timeout_ms
    }
}

pub struct PlanKey(FnvHasher);

impl PlanKey {
    pub fn new(tool: CursorTool) -> Self {
        let mut hasher = FnvHasher::new();
        write_entity_prefix(&mut hasher, "cursor_plan");
        write_str(&mut hasher, tool.as_str());
        Self(hasher)
    }

    pub fn str(mut self, value: &str) -> Self {
        write_str(&mut self.0, value);
        self
    }

    pub fn opt_str(self, value: Option<&str>) -> Self {
        self.str(value.unwrap_or(""))
    }

    pub fn usize(mut self, value: usize) -> Self {
        write_usize(&mut self.0, value);
        self
    }

    pub fn opt_usize(self, value: Option<usize>) -> Self {
        self.usize(value.unwrap_or(usize::MAX))
    }

    pub fn flag(mut self, value: bool) -> Self {
        write_usize(&mut self.0, usize::from(value));
        self
    }

    pub fn canonical_url(self, url: &str) -> Self {
        self.str(&crate::core::identity::canonicalize_url(url))
    }

    pub fn finish(self) -> String {
        format!("{:016x}", self.0.finish())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageWindow {
    pub start: usize,
    pub end: usize,
}

impl PageWindow {
    pub fn is_empty(self) -> bool {
        self.end <= self.start
    }

    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

pub fn page_window(
    len: usize,
    start: usize,
    capacity: usize,
    unit_chars: impl Fn(usize) -> usize,
) -> PageWindow {
    let start = start.min(len);
    let mut end = start;
    let mut used = 0usize;
    while end < len {
        let next = used.saturating_add(unit_chars(end));
        if end > start && next > capacity {
            break;
        }
        used = next;
        end += 1;
    }
    PageWindow { start, end }
}

pub fn locate_start(ids: &[String], cursor: &RetrievalCursor) -> Option<usize> {
    ids.iter()
        .position(|id| id == &cursor.p.id)
        .map(|index| index + 1)
}

pub fn encode_next(
    tool: CursorTool,
    plan_key: &str,
    ids: &[String],
    window: PageWindow,
    timeout_ms: u64,
) -> Option<String> {
    if window.is_empty() || window.end >= ids.len() {
        return None;
    }
    let last_id = ids.get(window.end - 1)?;
    RetrievalCursor::new(tool, plan_key, window.end, last_id, timeout_ms)
        .encode()
        .ok()
}

pub fn cursor_args(cursor: Option<&str>) -> Result<Option<RetrievalCursor>, CursorError> {
    cursor.map(RetrievalCursor::decode).transpose()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedPage {
    pub window: PageWindow,
    pub stale: bool,
    pub resumed: bool,
}

pub fn resolve_page(
    len: usize,
    ids: &[String],
    cursor: Option<&RetrievalCursor>,
    capacity: usize,
    unit_chars: impl Fn(usize) -> usize,
) -> ResolvedPage {
    let Some(cursor) = cursor else {
        return ResolvedPage {
            window: page_window(len, 0, capacity, unit_chars),
            stale: false,
            resumed: false,
        };
    };
    match locate_start(ids, cursor) {
        Some(start) if start < len => ResolvedPage {
            window: page_window(len, start, capacity, unit_chars),
            stale: false,
            resumed: true,
        },
        _ => ResolvedPage {
            window: page_window(len, 0, capacity, unit_chars),
            stale: true,
            resumed: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor() -> RetrievalCursor {
        RetrievalCursor::new(
            CursorTool::WebFetch,
            &PlanKey::new(CursorTool::WebFetch)
                .str("https://example.com")
                .usize(12000)
                .finish(),
            3,
            "chunk_0123456789abcdef",
            12000,
        )
    }

    #[test]
    fn round_trip_preserves_every_field() {
        let original = cursor();
        let encoded = original.encode().expect("encodes");
        assert!(encoded.starts_with(CURSOR_PREFIX));
        assert!(!encoded.contains('='));
        let decoded = RetrievalCursor::decode(&encoded).expect("decodes");
        assert_eq!(decoded, original);
    }

    #[test]
    fn payload_is_canonical_and_bounded() {
        let encoded = cursor().encode().expect("encodes");
        assert!(encoded.len() <= MAX_CURSOR_BYTES);
        let body = encoded.strip_prefix(CURSOR_PREFIX).unwrap();
        let json = URL_SAFE_NO_PAD.decode(body).unwrap();
        let text = String::from_utf8(json).unwrap();
        let expected = format!(
            r#"{{"v":1,"t":"web_fetch","k":"{}","p":{{"ord":3,"id":"chunk_0123456789abcdef"}},"d":{{"timeout_ms":12000}}}}"#,
            cursor().k
        );
        assert_eq!(text, expected);
    }

    #[test]
    fn oversized_payload_rejected() {
        let oversized = format!("{CURSOR_PREFIX}{}", "A".repeat(MAX_CURSOR_BYTES));
        assert_eq!(
            RetrievalCursor::decode(&oversized).unwrap_err(),
            CursorError::Invalid
        );
    }

    #[test]
    fn invalid_base64_rejected() {
        assert_eq!(
            RetrievalCursor::decode("e1.!!!!").unwrap_err(),
            CursorError::Invalid
        );
    }

    #[test]
    fn missing_prefix_rejected() {
        assert_eq!(
            RetrievalCursor::decode("e2.eyJ2IjoxfQ").unwrap_err(),
            CursorError::Invalid
        );
    }

    #[test]
    fn padded_base64_rejected() {
        let encoded = cursor().encode().unwrap();
        let body = encoded.strip_prefix(CURSOR_PREFIX).unwrap();
        assert_eq!(
            RetrievalCursor::decode(&format!("{CURSOR_PREFIX}{body}==")).unwrap_err(),
            CursorError::Invalid
        );
    }

    #[test]
    fn non_canonical_json_rejected() {
        let json = r#"{ "v": 1, "t": "web_fetch", "k": "0123456789abcdef", "p": {"ord": 3, "id": "chunk_a"}, "d": {"timeout_ms": 12000} }"#;
        let encoded = format!("{CURSOR_PREFIX}{}", URL_SAFE_NO_PAD.encode(json));
        assert_eq!(
            RetrievalCursor::decode(&encoded).unwrap_err(),
            CursorError::Invalid
        );
    }

    #[test]
    fn unknown_fields_rejected() {
        let json = r#"{"v":1,"t":"web_fetch","k":"0123456789abcdef","p":{"ord":3,"id":"chunk_a"},"d":{"timeout_ms":12000},"x":1}"#;
        let encoded = format!("{CURSOR_PREFIX}{}", URL_SAFE_NO_PAD.encode(json));
        assert_eq!(
            RetrievalCursor::decode(&encoded).unwrap_err(),
            CursorError::Invalid
        );
    }

    #[test]
    fn version_mismatch_rejected() {
        let mut value = cursor();
        value.v = 2;
        let body = serde_json::to_string(&value).unwrap();
        let encoded = format!("{CURSOR_PREFIX}{}", URL_SAFE_NO_PAD.encode(body));
        assert_eq!(
            RetrievalCursor::decode(&encoded).unwrap_err(),
            CursorError::Invalid
        );
    }

    #[test]
    fn cross_tool_cursor_is_mismatch() {
        let decoded = RetrievalCursor::decode(&cursor().encode().unwrap()).unwrap();
        assert_eq!(
            decoded.require_tool(CursorTool::BatchFetch).unwrap_err(),
            CursorError::Mismatch
        );
        assert_eq!(
            decoded.require_plan_key("0000000000000000").unwrap_err(),
            CursorError::Mismatch
        );
    }

    #[test]
    fn plan_key_is_content_derived_and_sensitive() {
        let base = PlanKey::new(CursorTool::WebFetch)
            .str("https://example.com/a")
            .finish();
        assert_eq!(base.len(), 16);
        assert_eq!(
            base,
            PlanKey::new(CursorTool::WebFetch)
                .str("https://example.com/a")
                .finish()
        );
        assert_ne!(
            base,
            PlanKey::new(CursorTool::WebFetch)
                .str("https://example.com/b")
                .finish()
        );
        assert_ne!(
            base,
            PlanKey::new(CursorTool::BatchFetch)
                .str("https://example.com/a")
                .finish()
        );
    }

    #[test]
    fn page_window_splits_without_gap_or_overlap() {
        let units = [4usize, 4, 4, 4];
        let mut start = 0usize;
        let mut seen = Vec::new();
        loop {
            let window = page_window(units.len(), start, 8, |index| units[index]);
            if window.is_empty() {
                break;
            }
            seen.extend(window.start..window.end);
            start = window.end;
        }
        assert_eq!(seen, vec![0, 1, 2, 3]);
    }

    #[test]
    fn page_window_always_delivers_one_unit() {
        let window = page_window(3, 0, 1, |_| 1000);
        assert_eq!(window, PageWindow { start: 0, end: 1 });
    }

    #[test]
    fn locate_start_returns_after_cursor_id() {
        let ids = vec![
            "chunk_a".to_string(),
            "chunk_b".to_string(),
            "chunk_c".to_string(),
        ];
        let mut value = cursor();
        value.p.id = "chunk_b".to_string();
        assert_eq!(locate_start(&ids, &value), Some(2));
        value.p.id = "chunk_missing".to_string();
        assert_eq!(locate_start(&ids, &value), None);
    }

    #[test]
    fn encode_next_omits_cursor_on_last_page() {
        let ids: Vec<String> = (0..2).map(|i| format!("chunk_{i}")).collect();
        let key = PlanKey::new(CursorTool::WebFetch).finish();
        let last = page_window(ids.len(), 0, 100, |_| 1);
        assert_eq!(
            encode_next(CursorTool::WebFetch, &key, &ids, last, 1000),
            None
        );
        let first = page_window(ids.len(), 0, 1, |_| 1);
        assert!(encode_next(CursorTool::WebFetch, &key, &ids, first, 1000).is_some());
    }
}
