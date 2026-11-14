# M012 Startpage Payload Extraction — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/012-startpage-payload-extraction.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `d751bd7471a647e651ee41b8e51199c66d04d01a`

Implementation candidate and closure commit: the commit that carries this record. Implementation,
closure evidence, roadmap status, and the registry row all moved in that single commit on top of the
planning baseline above.

## Result

A solved Startpage SERP parses. `src/meta/engines/startpage_payload.rs` exposes

```rust
pub(super) fn parse_payload(html: &str, max_results: usize) -> Option<Vec<SearchResult>>
```

`None` means "no payload on this page" so the caller falls back to the DOM parser; `Some(vec![])`
means "payload present, no usable rows" and still flows through `finalize_scrape` for
empty-versus-broken classification.

Extraction, matching what the upstream actually serves:

1. locate `React.createElement(UIStartpage.AppSerp…, {`;
2. take everything after that `{` up to the closing `}}` and rebuild the object as
   `"{" + <captured> + "}}"`;
3. read `render.presenter.regions.mainline[]`;
4. keep entries whose `display_type` starts with `web`;
5. map `clickUrl` → `url` (http/https only), `title` → `title` (HTML-to-text, trimmed, non-empty),
   `description` → `snippet` (HTML-to-text, whitespace-normalised, bounded);
6. skip entries without a usable URL or title, exactly like the DOM path.

`src/meta/engines/startpage.rs` tries the payload first when present and keeps the existing
`div.result` selectors as a fallback. Both paths converge on
`finalize_scrape(ENGINE, &document, &SHAPE, results)`, so the four-outcome contract is unchanged and
a challenge page is still classified `blocked` before any payload work.

## Live verification (2026-11-12 UTC)

Through the shipped release binary with `[search.challenge] enabled = true`,
`max_difficulty = 6`, `max_attempts = 32000000`, `timeout_ms = 14000`,
`[search].timeout_ms = 15000`:

| Configuration | Observation |
|---|---|
| `enabled = false` | 0 results, `error_class: blocked`, reason `challenge page detected (#anubis_challenge)` |
| `enabled = true`, `max_difficulty = 5` | 0 results, `blocked`, reason `anubis difficulty 6 exceeds configured ceiling 5` |
| `enabled = true`, `max_difficulty = 6` | **5/5 rows** with titles, URLs and snippets, 3.88 s wall clock including the solve |

That closes the chain the milestone was created for: a shipped `startpage` search now returns rows
instead of `parse_error`.

## Defect found during capture, fixed in M011's module

The live Anubis `fast` challenge publishes **no `redir` field**:

```json
{"rules":{"algorithm":"fast","difficulty":6},"challenge":{"issuedAt":"…","id":"…","method":"fast","randomData":"…","policyRuleHash":"…","difficulty":6,"spent":false}}
```

M011's parser required `redir` as a mandatory field, so the shipped solver refused every real
Startpage challenge as `malformed parameters`. The correct behaviour is what SearXNG does: the
redirect target is the original request URL, not a published field.

Fixed by making `redir` optional and resolving it from the request URL
(`AnubisChallenge::resolved_redir`), threading the real request URL
(`https://www.startpage.com/search?q=…`) through `solve_and_verify`. A published `redir` still wins
when present. Two regression tests pin both behaviours, including one that parses the exact live
payload shape above.

This is the milestone chain working as designed: without a live capture, M011 would have read as
complete while being unable to solve anything in practice.

## Probabilistic solve, recorded honestly

The nonce is uniform, so a difficulty-6 attempt can legitimately exhaust `max_attempts`. At
`32000000` — roughly twice the 16.8M expected candidates — about one attempt in seven reports
`challenge not solved: attempts exhausted` and `blocked`. This is a retryable provider condition,
not a defect, and it is documented in `docs/provider-setup.md` and `docs/config.md` rather than
being smoothed over.

## Fixtures

`tests/fixtures/engines/startpage/results.html` is derived from a live solved capture; the raw body
was kept under `target/` and never committed. The committed fixture is trimmed to two result blocks
with the payload reduced to the fields the parser reads and all tokens, identifiers and cookies
removed. Provenance is recorded in `tests/fixtures/engines/README.md`. The existing synthetic
`blocked.html` and `empty.html` fixtures remain the drift and blocked cases.

The conformance table gained a payload-shaped `startpage` case plus a seeded drift that renames
`clickUrl`, proving `ParseFailed` rather than a silent empty result.

## Verification

- `cargo fmt --all`: clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `cargo test --locked --all-features --lib meta::engines::startpage`: passed (33 unit tests across
  the startpage module at the time of the run).
- `cargo test --locked --all-features --lib meta::challenge::anubis`: passed, including the two new
  `redir` regression tests.
- `cargo test --locked --all-features --test engine_parse_conformance`: 6 passed.
- `cargo test --locked --all-features --test static_guards`: 61 passed.
- Full `cargo test --locked --all-features`: see `009-status.md` for the shared final count; the
  closure commit records the authoritative number in `docs/test-inventory.md`.

## Residual risk

- The payload is an inline React tree, not a documented API. A category token change, a renamed
  `mainline` region, or a new `display_type` prefix is a structural break by design and surfaces as
  `parse_error`, not as absence.
- Only the web/mainline region is read; news, images and video categories are out of scope.
- The DOM fallback exists so an older or differently-served page still parses, but if the upstream
  removes the payload and the DOM simultaneously the provider degrades to `parse_error`, which is the
  intended honest outcome.
- Challenge cost is risk-scored and can rise; a difficulty-7 wall needs roughly 16× the work and
  would be refused at the validated `max_attempts` maximum.
