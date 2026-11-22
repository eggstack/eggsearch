pub(crate) fn require_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

pub(crate) fn require_char_limit(field: &str, value: &str, max: usize) -> Result<(), String> {
    if value.chars().count() > max {
        Err(format!("{field} must be <= {max} characters"))
    } else {
        Ok(())
    }
}

pub(crate) fn require_positive(field: &str, value: Option<usize>) -> Result<(), String> {
    if value == Some(0) {
        Err(format!("{field} must be > 0"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_non_empty_rejects_empty() {
        assert_eq!(
            require_non_empty("query", " 	"),
            Err("query must not be empty".to_string())
        );
    }

    #[test]
    fn require_char_limit_counts_characters_and_accepts_boundary() {
        let value = "é🦀";
        assert_eq!(require_char_limit("query", value, 2), Ok(()));
        assert_eq!(
            require_char_limit("query", value, 1),
            Err("query must be <= 1 characters".to_string())
        );
    }

    #[test]
    fn require_positive_rejects_zero() {
        assert_eq!(
            require_positive("max_results", Some(0)),
            Err("max_results must be > 0".to_string())
        );
        assert_eq!(require_positive("max_results", Some(1)), Ok(()));
        assert_eq!(require_positive("max_results", None), Ok(()));
    }
}
