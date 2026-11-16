use crate::fetch::limits::FetchLimits;

/// The extraction budget (how much of a page is addressable) and the output
/// window (how much text one call renders) for a single fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FetchBudgets {
    document_chars: usize,
    window_chars: usize,
}

impl FetchBudgets {
    /// Build a budget pair, keeping the window inside the document budget.
    pub fn new(document_chars: usize, window_chars: usize) -> Self {
        let document_chars = document_chars.max(1);
        Self {
            document_chars,
            window_chars: window_chars.clamp(1, document_chars),
        }
    }

    /// Resolve both budgets from the configured limits and a caller request.
    pub fn resolve(limits: &FetchLimits, requested: Option<usize>) -> Self {
        Self::new(
            limits.max_chars_cap,
            requested.unwrap_or(limits.max_chars_default),
        )
    }

    /// The number of characters extraction keeps addressable.
    pub fn document_chars(&self) -> usize {
        self.document_chars
    }

    /// The number of characters one call renders.
    pub fn window_chars(&self) -> usize {
        self.window_chars
    }
}

#[cfg(test)]
mod tests {
    use super::FetchBudgets;
    use crate::fetch::limits::FetchLimits;

    #[test]
    fn default_configuration_uses_the_configured_defaults() {
        let limits = FetchLimits::default();
        let budgets = FetchBudgets::resolve(&limits, None);
        assert_eq!(budgets.document_chars(), limits.max_chars_cap);
        assert_eq!(budgets.window_chars(), limits.max_chars_default);
        assert_eq!(budgets.document_chars(), 50_000);
        assert_eq!(budgets.window_chars(), 12_000);
    }

    #[test]
    fn a_requested_window_is_clamped_to_the_document_budget() {
        let limits = FetchLimits::default();
        let budgets = FetchBudgets::resolve(&limits, Some(500_000));
        assert_eq!(budgets.window_chars(), limits.max_chars_cap);
        assert_eq!(budgets.document_chars(), limits.max_chars_cap);
    }

    #[test]
    fn a_window_equal_to_the_document_leaves_one_page() {
        let budgets = FetchBudgets::new(12_000, 12_000);
        assert_eq!(budgets.document_chars(), budgets.window_chars());
    }

    #[test]
    fn a_one_character_document_bounds_the_window_to_one() {
        let budgets = FetchBudgets::new(1, 12_000);
        assert_eq!(budgets.document_chars(), 1);
        assert_eq!(budgets.window_chars(), 1);
    }

    #[test]
    fn a_zero_document_budget_is_lifted_to_one() {
        let budgets = FetchBudgets::new(0, 0);
        assert_eq!(budgets.document_chars(), 1);
        assert_eq!(budgets.window_chars(), 1);
    }

    #[test]
    fn a_narrower_window_leaves_the_document_budget_untouched() {
        let budgets = FetchBudgets::new(50_000, 2_000);
        assert_eq!(budgets.document_chars(), 50_000);
        assert_eq!(budgets.window_chars(), 2_000);
    }
}
