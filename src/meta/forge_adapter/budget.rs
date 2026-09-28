use super::*;

/// Error type for forge bounded-read operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForgeReadError {
    /// A single response exceeded the per-response byte limit.
    PerResponseLimitExceeded,
    /// The aggregate operation budget was exhausted.
    AggregateBudgetExhausted,
    /// The declared Content-Length exceeded the effective byte cap.
    ContentLengthTooLarge,
    /// Reading a response stream chunk failed.
    StreamReadFailure,
}

impl std::fmt::Display for ForgeReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PerResponseLimitExceeded => write!(f, "response_too_large"),
            Self::AggregateBudgetExhausted => write!(f, "aggregate_budget_exhausted"),
            Self::ContentLengthTooLarge => write!(f, "content_length_too_large"),
            Self::StreamReadFailure => write!(f, "stream_read_failure"),
        }
    }
}

impl ForgeReadError {
    /// Return a stable static string representation of the error variant.
    pub fn as_static_str(&self) -> &'static str {
        match self {
            Self::PerResponseLimitExceeded => "response_too_large",
            Self::AggregateBudgetExhausted => "aggregate_budget_exhausted",
            Self::ContentLengthTooLarge => "content_length_too_large",
            Self::StreamReadFailure => "stream_read_failure",
        }
    }
}

/// Telemetry snapshot for the forge read budget.
pub struct ForgeReadBudgetTelemetry {
    /// Configured aggregate byte limit for the operation.
    pub aggregate_limit: usize,
    /// Total bytes observed across all counted responses.
    pub aggregate_observed: usize,
    /// Remaining budget before exhaustion.
    pub remaining: usize,
    /// Number of requests counted against the budget.
    pub request_count: usize,
    /// Which request kind exhausted the budget, if any.
    pub exhausted_by: Option<ForgeRequestKind>,
    /// Number of per-response cap hits.
    pub per_response_cap_hits: usize,
}

/// Aggregate byte budget for forge tree operations.
pub struct ForgeReadBudget {
    /// Maximum bytes allowed for a single response.
    pub per_response_limit: usize,
    /// Maximum aggregate bytes for the whole operation.
    pub aggregate_limit: usize,
    /// Bytes observed so far.
    pub aggregate_observed: usize,
    /// Whether the budget has been exhausted.
    pub exhausted: bool,
    /// Which request kind caused exhaustion.
    pub exhausted_by: Option<ForgeRequestKind>,
    /// Number of requests observed.
    pub request_count: usize,
    /// Count of per-response cap hits.
    pub per_response_cap_hits: usize,
}

impl ForgeReadBudget {
    /// Create a new budget with the given aggregate limit.
    pub fn new(aggregate_limit: usize) -> Self {
        Self {
            per_response_limit: DEFAULT_MAX_RESPONSE_BYTES,
            aggregate_limit,
            aggregate_observed: 0,
            exhausted: false,
            exhausted_by: None,
            request_count: 0,
            per_response_cap_hits: 0,
        }
    }

    /// Remaining budget before exhaustion.
    pub fn remaining(&self) -> usize {
        self.aggregate_limit.saturating_sub(self.aggregate_observed)
    }

    /// Consume bytes against the budget and mark exhaustion if reached.
    pub fn consume(&mut self, bytes: usize, kind: ForgeRequestKind) {
        self.aggregate_observed = self.aggregate_observed.saturating_add(bytes);
        self.request_count += 1;
        if self.aggregate_observed >= self.aggregate_limit && !self.exhausted {
            self.exhausted = true;
            self.exhausted_by = Some(kind);
        }
    }

    /// Whether the aggregate budget has been exhausted.
    pub fn exceeded(&self) -> bool {
        self.exhausted
    }

    /// Snapshot telemetry for the current budget state.
    pub fn telemetry(&self) -> ForgeReadBudgetTelemetry {
        ForgeReadBudgetTelemetry {
            aggregate_limit: self.aggregate_limit,
            aggregate_observed: self.aggregate_observed,
            remaining: self.remaining(),
            request_count: self.request_count,
            exhausted_by: self.exhausted_by,
            per_response_cap_hits: self.per_response_cap_hits,
        }
    }
}

pub(crate) async fn read_with_budget(
    mut resp: eggfetch_core::Response,
    budget: &mut ForgeReadBudget,
    kind: ForgeRequestKind,
) -> Result<Vec<u8>, ForgeReadError> {
    let effective_cap = budget.per_response_limit.min(budget.remaining());
    if effective_cap == 0 {
        return Err(ForgeReadError::AggregateBudgetExhausted);
    }
    if let Some(content_length) = resp.content_length() {
        if content_length > effective_cap as u64 {
            let charge = usize::try_from(content_length)
                .unwrap_or(usize::MAX)
                .min(effective_cap.saturating_add(1));
            if content_length > budget.per_response_limit as u64 {
                budget.per_response_cap_hits += 1;
                budget.consume(charge, kind);
                return Err(ForgeReadError::ContentLengthTooLarge);
            }
            budget.consume(charge, kind);
            return Err(ForgeReadError::AggregateBudgetExhausted);
        }
    }
    let mut body = Vec::with_capacity(effective_cap.min(64 * 1024));
    let mut observed = 0usize;
    let mut stream = resp
        .bytes_stream()
        .map_err(|_| ForgeReadError::StreamReadFailure)?;
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ForgeReadError::StreamReadFailure)?;
        observed += chunk.len();
        if observed > effective_cap {
            if observed > budget.per_response_limit {
                budget.per_response_cap_hits += 1;
                budget.consume(observed, kind);
                return Err(ForgeReadError::PerResponseLimitExceeded);
            }
            budget.consume(observed, kind);
            return Err(ForgeReadError::AggregateBudgetExhausted);
        }
        body.extend_from_slice(&chunk);
    }
    budget.consume(observed, kind);
    Ok(body)
}

const ERROR_BODY_CAP: usize = 8 * 1024;

/// Read a preview of an error response body, capped at 8KB.
///
/// Strips control characters (except newline, carriage return, tab)
/// and truncates at the byte cap. Charges the observed bytes against
/// the aggregate budget.
pub async fn read_error_body_preview(
    mut resp: eggfetch_core::Response,
    budget: &mut ForgeReadBudget,
) -> String {
    let cap = ERROR_BODY_CAP.min(budget.remaining());
    if cap == 0 {
        return String::new();
    }
    let mut body = Vec::with_capacity(cap.min(8192));
    let mut stream = match resp.bytes_stream() {
        Ok(stream) => stream,
        Err(error) => {
            tracing::warn!(error = ?error, "failed to read forge error response body");
            budget.consume(0, ForgeRequestKind::ErrorBody);
            return "[error reading response body]".to_string();
        }
    };
    let mut observed = 0usize;
    let mut stream_read_failed = false;
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => {
                observed += chunk.len();
                if observed > cap {
                    break;
                }
                body.extend_from_slice(&chunk);
            }
            Err(error) => {
                tracing::warn!(error = ?error, "failed to read forge error response body");
                stream_read_failed = true;
                break;
            }
        }
    }
    let charged = observed.min(cap);
    if stream_read_failed {
        const STREAM_FAILURE_MARKER: &[u8] = b"[error reading response body]";
        let keep = cap.saturating_sub(STREAM_FAILURE_MARKER.len());
        body.truncate(keep);
        body.extend_from_slice(STREAM_FAILURE_MARKER);
    }
    budget.consume(charged, ForgeRequestKind::ErrorBody);
    String::from_utf8_lossy(&body)
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
        .collect()
}
