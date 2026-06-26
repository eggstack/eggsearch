# eggsearch Handoff Plan: Normalize `max_results` Semantics and Context Caps

## Goal

Clean up the search result-count API so eggsearch follows common MCP/search-server conventions while preserving a strong server-side context-safety limit.

The intended model is:

- MCP clients may request `max_results` per search.
- Server config owns the default result count.
- Server config owns the hard cap.
- The final SourceCard count is always bounded by the hard cap.
- If a caller requests more than the cap, eggsearch clamps and returns a warning rather than failing the tool call.

This should remove ambiguity between `max_results` and `max_results_cap` while keeping the API familiar to other search MCPs.

## Background

The current naming creates possible confusion because `max_results` and `max_results_cap` can appear redundant. They are only meaningfully distinct if they represent different authorities:

- `request.max_results`: caller's per-request desired result count.
- `config.default_max_results`: server default when the caller omits `max_results`.
- `config.max_results_cap`: server/admin hard upper bound.

Do not expose `max_results_cap` as an MCP tool argument. It is a server-side guardrail.

## Non-goals

Do not add local indexing, caching, Tantivy, or persistent search storage.

Do not add new search providers in this pass.

Do not change the basic `web_search` tool contract beyond clarifying result-count behavior.

Do not expose internal per-provider candidate limits in the MCP schema unless already exposed. If needed, keep them config/internal only.

## Desired Public API

The MCP `web_search` request should expose:

```json
{
  "query": "string",
  "max_results": "integer | null",
  "providers": "string[] | null",
  "timeout_ms": "integer | null",
  "safe_search": "string | null"
}
```

`max_results` means: final maximum number of merged/deduplicated SourceCards the caller wants returned.

The MCP response should include a warning when clamping occurs:

```json
{
  "warnings": [
    "Requested max_results=100 exceeded server cap=25; using 25."
  ]
}
```

The response should not include more than the effective result count.

## Desired Config API

Rename or normalize config fields to this model:

```toml
[search]
default_max_results = 10
max_results_cap = 25
```

If the current config still uses `max_results`, treat it as deprecated and migrate it to `default_max_results`.

Recommended backward-compatible behavior:

1. Accept old config field `max_results` for now.
2. Interpret it as `default_max_results`.
3. Prefer `default_max_results` when both are present.
4. Emit a warning during config load or `doctor` if deprecated `max_results` is used.
5. Update generated/default config files to use only `default_max_results`.

Validation rules:

```text
default_max_results >= 1
max_results_cap >= 1
default_max_results <= max_results_cap
```

If `default_max_results > max_results_cap`, either fail config validation or clamp the default at startup with a clear warning. Prefer failing validation, because this is an operator/configuration error.

## Effective Result Count Resolution

Implement one centralized resolver. Do not duplicate this logic in MCP handlers, adapters, and aggregators.

Suggested function:

```rust
pub struct MaxResultsResolution {
    pub requested: Option<usize>,
    pub effective: usize,
    pub clamped: bool,
    pub warning: Option<String>,
}

pub fn resolve_max_results(
    requested: Option<usize>,
    default_max_results: usize,
    max_results_cap: usize,
) -> Result<MaxResultsResolution, SearchError> {
    let requested_or_default = requested.unwrap_or(default_max_results);

    if requested_or_default == 0 {
        return Err(SearchError::InvalidInput(
            "max_results must be at least 1".to_string(),
        ));
    }

    let effective = requested_or_default.min(max_results_cap);
    let clamped = requested_or_default > max_results_cap;

    let warning = clamped.then(|| {
        format!(
            "Requested max_results={} exceeded server cap={}; using {}.",
            requested_or_default, max_results_cap, effective
        )
    });

    Ok(MaxResultsResolution {
        requested,
        effective,
        clamped,
        warning,
    })
}
```

If schema validation already prevents zero, keep the runtime check anyway for defense in depth.

## Internal Metasearch Candidate Count

Eggsearch is a metasearch engine, so the final result limit is not necessarily the same as the number of candidates requested from each upstream provider.

Keep the MCP surface simple:

- Public field: `max_results`
- Meaning: final SourceCard count

Internally, derive provider candidate count if needed:

```rust
let per_provider_limit = (effective_max_results * 2).min(config.search.per_provider_results_cap);
```

Only add `per_provider_results_cap` if the current implementation needs it. Otherwise defer.

Do not expose `per_provider_limit` to MCP clients in this pass.

## Implementation Tasks

### 1. Audit current config and request types

Find all fields named:

- `max_results`
- `max_results_cap`
- `default_max_results`
- any equivalent count/limit/cap fields

Classify each as one of:

- request-level caller preference
- config default
- config hard cap
- internal provider candidate limit

Remove ambiguous uses.

### 2. Rename config field

If server config currently has:

```toml
[search]
max_results = 10
max_results_cap = 25
```

Change to:

```toml
[search]
default_max_results = 10
max_results_cap = 25
```

Preserve backward compatibility with deprecated `max_results` for at least one minor release.

Update config structs accordingly. Recommended shape:

```rust
pub struct SearchConfig {
    pub default_max_results: usize,
    pub max_results_cap: usize,
    // deprecated alias, if using serde compatibility:
    // pub max_results: Option<usize>,
}
```

If serde aliases are used, document exactly which field wins when both are present.

### 3. Centralize resolution logic

Add `resolve_max_results` in a core module, likely one of:

- `src/core/query.rs`
- `src/core/config.rs`
- `src/core/limits.rs`

All web-search paths should call this function before provider execution.

The aggregator should receive the already-resolved effective final result count.

### 4. Clamp with warning

When request `max_results` exceeds `max_results_cap`:

- Do not fail the MCP call.
- Use the cap.
- Add a warning to the MCP response.

This is more agent-friendly than hard failure and prevents repeated tool-call churn.

### 5. Reject invalid values

If `max_results == 0`, reject validation with a clear error.

If the MCP schema supports minimum constraints, add `minimum: 1`.

Also validate at runtime.

### 6. Update MCP schema and descriptions

Update tool description text to clarify:

```text
max_results is an optional per-request final SourceCard count. The server may clamp this to its configured max_results_cap to limit context size.
```

Do not mention `max_results_cap` as a user-settable field in tool input docs.

### 7. Update README and config examples

Update README examples to use:

```toml
[search]
default_max_results = 10
max_results_cap = 25
```

Document behavior:

```text
If max_results is omitted, eggsearch returns default_max_results.
If max_results exceeds max_results_cap, eggsearch clamps and returns a warning.
The cap exists to avoid context pollution and runaway result dumps.
```

### 8. Update `doctor`

`eggsearch doctor` should report:

```text
default_max_results: 10
max_results_cap: 25
```

If deprecated `max_results` is present in config, report:

```text
warning: search.max_results is deprecated; use search.default_max_results instead.
```

If `default_max_results > max_results_cap`, fail doctor/config validation.

### 9. Add tests

Add unit tests for result-count resolution:

```text
omitted request uses default
request below cap uses request
request equal to cap uses request without warning
request above cap clamps and warns
request zero errors
default greater than cap fails config validation
old config max_results maps to default_max_results
both old and new fields prefer default_max_results
```

Add MCP-level tests if existing test harness supports them:

```text
web_search with max_results omitted returns <= default_max_results
web_search with max_results above cap returns <= cap and warning
web_search with max_results=0 fails validation
```

### 10. Update changelog/version notes

Add a changelog entry:

```text
Changed: search.max_results config field is deprecated in favor of search.default_max_results.
Clarified: MCP request max_results is a per-call final result-count preference, clamped by server max_results_cap.
Added: warning when requested max_results exceeds configured cap.
```

## Acceptance Criteria

The implementation is complete when:

- MCP request exposes only `max_results`, not `max_results_cap`.
- Config uses `default_max_results` and `max_results_cap`.
- Old `search.max_results` config still works as a deprecated alias.
- Effective result count is resolved in one place.
- Requests over cap are clamped with a response warning.
- Requests for zero results are rejected.
- Final returned SourceCard count never exceeds the effective value.
- README, config examples, MCP tool descriptions, and doctor output all agree.
- Tests cover default, override, cap, clamp warning, invalid zero, and deprecated config alias behavior.

## Suggested Commit Breakdown

1. `config: rename search max_results default`
2. `core: centralize max_results resolution`
3. `mcp: clamp oversized search requests with warning`
4. `docs: clarify search result count semantics`
5. `test: cover max_results default override and cap behavior`

## Notes for Implementer

Keep this pass narrow. Do not refactor provider internals unless required to pass the effective result count cleanly.

Avoid introducing provider-specific count fields in the public MCP schema. If the aggregator needs extra provider candidates for dedupe/RRF quality, derive that internally from the effective final count.

The purpose of `max_results_cap` is context control. Treat it as a harness safety/resource limit, not as a user-facing search preference.

