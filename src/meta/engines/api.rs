use super::error::EngineError;
use super::{engine_timeout, map_request_error, read_bounded_body};

pub(crate) async fn get_json_bounded<T: serde::de::DeserializeOwned>(
    client: &eggfetch_core::Client,
    engine: &'static str,
    url: &str,
    query: &[(&str, &str)],
    headers: &[(&str, &str)],
    max_body: usize,
    timeout: std::time::Duration,
) -> Result<T, EngineError> {
    let mut request = client
        .get(url)
        .map_err(|error| map_request_error(engine, error))?;
    for &(name, value) in query {
        request = request.query(name, value);
    }
    for &(name, value) in headers {
        request = request.header(name, value);
    }
    let response = request
        .timeout(engine_timeout(timeout))
        .send()
        .await
        .map_err(|error| map_request_error(engine, error))?;
    let status = response.status();
    if !status.is_success() {
        return Err(EngineError::BadStatus {
            engine,
            status: status.as_u16(),
        });
    }
    let body = read_bounded_body(response, engine, max_body).await?;
    serde_json::from_slice(&body).map_err(|error| EngineError::ParseFailed {
        engine,
        reason: format!("invalid JSON: {error}"),
    })
}

pub(crate) fn truncate_body(body: &str, max_chars: usize) -> String {
    crate::core::sanitize::truncate_at_word(body, max_chars)
}

#[cfg(test)]
pub(crate) fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::truncate_body;

    #[test]
    fn truncate_body_short() {
        assert_eq!(truncate_body("hello", 100), "hello");
    }

    #[test]
    fn truncate_body_exact_limit() {
        assert_eq!(truncate_body("hello", 5), "hello");
    }

    #[test]
    fn truncate_body_at_word_boundary() {
        assert_eq!(truncate_body("hello world foo bar", 11), "hello");
    }

    #[test]
    fn truncate_body_without_spaces() {
        assert_eq!(truncate_body("abcdefghij", 5), "abcde");
    }

    #[test]
    fn truncate_body_handles_multibyte_utf8() {
        let body = "abc 🦀 rust 🧪 unicode";
        let out = truncate_body(body, 7);
        assert!(out.is_char_boundary(out.len()));
        assert!(out.len() <= body.len());
        assert!(out.chars().count() <= 7);
    }

    #[test]
    fn truncate_body_handles_cjk_text() {
        let out = truncate_body("修正修正修正修正", 5);
        assert!(out.is_char_boundary(out.len()));
        assert!(out.chars().count() <= 5);
    }

    #[test]
    fn truncate_body_handles_emoji_only_text() {
        let out = truncate_body("🦀🦀🦀🦀🦀", 3);
        assert!(out.is_char_boundary(out.len()));
        assert_eq!(out.chars().count(), 3);
        assert_eq!(out, "🦀🦀🦀");
    }

    #[test]
    fn truncate_body_at_word_boundary_with_emoji() {
        assert_eq!(truncate_body("hello 🦀 world", 7), "hello");
    }

    #[test]
    fn truncate_body_zero_limit() {
        assert_eq!(truncate_body("anything", 0), "");
    }
}
