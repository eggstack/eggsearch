use crate::core::identity::{write_entity_prefix, write_str, write_usize, FnvHasher};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanTool {
    WebFetch,
    BatchFetch,
    ResearchSearch,
}

impl PlanTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WebFetch => "web_fetch",
            Self::BatchFetch => "batch_fetch",
            Self::ResearchSearch => "research_search",
        }
    }
}

pub struct PlanKey(FnvHasher);

impl PlanKey {
    pub fn new(tool: PlanTool) -> Self {
        let mut hasher = FnvHasher::new();
        write_entity_prefix(&mut hasher, "request_plan");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_key_is_content_derived_and_sensitive() {
        let base = PlanKey::new(PlanTool::WebFetch)
            .str("https://example.com/a")
            .finish();
        assert_eq!(base.len(), 16);
        assert!(base
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert_eq!(
            base,
            PlanKey::new(PlanTool::WebFetch)
                .str("https://example.com/a")
                .finish()
        );
        assert_ne!(
            base,
            PlanKey::new(PlanTool::WebFetch)
                .str("https://example.com/b")
                .finish()
        );
        assert_ne!(
            base,
            PlanKey::new(PlanTool::BatchFetch)
                .str("https://example.com/a")
                .finish()
        );
    }

    #[test]
    fn plan_key_canonicalizes_urls_before_hashing() {
        let canonical = PlanKey::new(PlanTool::WebFetch)
            .canonical_url("https://Example.com/a/")
            .finish();
        assert_eq!(
            canonical,
            PlanKey::new(PlanTool::WebFetch)
                .canonical_url("https://www.example.com/a")
                .finish()
        );
    }

    #[test]
    fn optional_fields_hash_distinctly_from_absent_values() {
        let absent = PlanKey::new(PlanTool::WebFetch).opt_str(None).finish();
        let empty = PlanKey::new(PlanTool::WebFetch).opt_str(Some("")).finish();
        let present = PlanKey::new(PlanTool::WebFetch)
            .opt_str(Some("focus"))
            .finish();
        assert_eq!(absent, empty);
        assert_ne!(absent, present);
    }
}
