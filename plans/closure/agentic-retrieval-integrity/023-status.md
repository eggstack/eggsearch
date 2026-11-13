# M023 Fetch-Side Anubis Solve and Replay — Closure Status

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/023-fetch-side-anubis-solve-and-replay.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `e856468`

Implementation candidate and closure commit: the commit that carries this record.

## Result

When `[search.challenge].enabled` is true and a fetched HTML page is a detected Anubis challenge,
`web_fetch` solves the published challenge once with the existing bounded solver and replays the
request once with the issued `spchal-auth` cookie, returning the real document. Disabled, refused,
errored, expired-deadline, unparseable, or still-challenged outcomes keep M022's marking unchanged.
No new tool, argument, config key, dependency, or serialized field.

- `src/fetch/client.rs`: `fetch_once` joins every `Set-Cookie` name/value pair into one `Cookie`
  header value and returns it beside the response. New `fetch_with_cookie` and `pub(crate)
  fetch_capturing`; `fetch` and `fetch_with_origin_controller` delegate with no cookie, so every
  existing caller is unchanged. The cookie is attached only on the first hop (`redirect_count == 0`).
- `src/fetch/negotiate.rs` only threads the extra parameter/cookie through the markdown→HTML
  fallback; cache keys and negotiation are untouched.
- `src/mcp/tools/web_fetch.rs::replay_anubis_challenge` gates on HTML + `detect_html == Anubis` +
  `[search.challenge].enabled`, derives the origin from `final_url`, builds the solver client with
  the configured fetch user agent, runs `anubis::solve_and_verify` under `SolveBudget::new(
  max_attempts, min(timeout_ms, remaining deadline), max_difficulty)`, and replays once on `Solved`.
  The replay replaces the original response only if it is not itself a detected challenge.
- `src/fetch/**` still imports no `crate::meta`; only `src/mcp/tools/web_fetch.rs` orchestrates.
- The pass value lives only in a local `String` handed to `fetch_with_cookie`; it is never stored on
  a `Debug`/`Serialize` type and never enters an error, warning, log, or fixture. The replay
  replaces the response before the cache write, so the cached entry is the document.

## Live evidence (2026-11-13, release binary, honest fetch user agent)

`web_fetch {"url":"https://www.startpage.com/search?q=rust+programming+language",
"max_chars":500,"render":"http_only"}` with `[search.challenge] enabled = true`,
`max_difficulty = 6`, `max_attempts = 50000000`, `timeout_ms = 15000`:

```
2.32 s, isError=false, status=200, manual_interaction_required=false,
structured_warnings=[fetch_warning] (no fetch_challenge_page)
text: "Rust Programming Language ... Rust is blazingly fast and memory-efficient ..."
```

The same request with solving disabled is M022's marked page. The solver path was also measured
directly in an isolated probe: 22,044-byte challenge, difficulty 6, 4,963,858 candidates, replay
238,715 bytes containing the `UIStartpage.AppSerp` payload, 11 result titles.

## Verification

```bash
cargo fmt --all -- --check                    # clean
cargo clippy --locked --all-targets --all-features -- -D warnings   # clean
cargo check --locked --no-default-features    # clean
cargo test --locked --all-features --test web_fetch_integration     # 93 passed / 1 ignored
cargo test --locked --all-features --test static_guards             # 62 passed
cargo test --locked --all-features --lib                            # 3,833 passed
cargo test --locked --all-features                                 # 6,291 passed / 28 ignored
```

Mock-transport coverage: solved flow returns the document with exactly one pass request and one
replay and no `fetch_challenge_page`; disabled performs one request, marks the page, and issues
zero pass requests; a `max_attempts = 1` refusal marks the page with zero pass requests; a replay
that is still challenged marks the page and performs exactly one solve; a canary asserts no
`pass-cookie-123`, `pow-1`, `spchal-auth`, or `sp_pow` appears in the serialized payload. Unit
coverage: multi-`Set-Cookie` join and first-hop-only attachment.

## Deviations recorded

- The challenge cookies travel beside the response rather than on a new `WebFetchResponse` field:
  the type has exhaustive struct literals in other workers' files, and the sidecar is also safer
  (no cookie on a `Debug + Serialize` type).
- `src/fetch/client.rs` tests moved to `src/fetch/client_tests.rs` because the file sits at its
  2,600-line ratchet; the guard's own remedy is a split.
- The solve runs after the browser escalation so a successful `render: "auto"` render keeps
  priority, as M022's invariant requires.
- The replay targets the caller's URL; a wall reached only after a redirect is not solved because
  the cookie is first-hop-only.


Hosted CI: run `43472592990` on `0019f48` passed all three jobs — Ubuntu `ci` (12m11s),
Windows all-features check, and macOS canonical verification plus filesystem portability.

## Residual risk

- `batch_fetch` and `repo_fetch` do not solve; a wall fetched through them stays marked.
- A redirect before the wall is not solved (first-hop-only cookie).
- The solve consumes the same call deadline after the initial fetch; a slow origin plus a high
  difficulty can leave no budget and keep the mark.
- `web_fetch` solves occupy one of the four solve-ledger provider slots.
- `cookie_ttl_seconds` does not apply to the single fetch replay; the cookie is never stored.
