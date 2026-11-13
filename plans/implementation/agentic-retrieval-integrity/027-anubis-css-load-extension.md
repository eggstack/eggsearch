# M027 — Anubis `css-load` Extension Support

Status: closed
Source: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M027
Primary class: capability (extension coverage)
Baseline: `fd2634e`
References: ADR-0009; ADR-0010; ADR-0012 (accepted); `src/meta/challenge/anubis.rs`; `src/meta/challenge/ledger.rs`; `src/meta/engines/startpage.rs`; `src/mcp/tools/web_fetch.rs`; `architecture/engines.md` § Challenge solving; `docs/config.md` § Challenge Solving.
Dependencies: hard ADR-0012 acceptance; hard M026 (extensions are parsed and fail
closed). This plan must not begin until ADR-0012 is accepted and M026 is closed.

## Objective

Support the only open-source Anubis challenge extension, `css-load`, without widening
the permitted class beyond ADR-0012. The extension asserts that the client fetched a
same-origin CSS URL keyed by the challenge ID. Anubis renders:

```html
<style>@import url("/.within.website/x/cmd/anubis/ext/css-load/<challenge-id>");</style>
```

and records a hit when that URL is fetched. A subsequent pass-challenge request passes
the extension validator only if the hit exists.

The implementation is one bounded, same-origin `GET` between parsing and submission.
It forwards no cookie, header, body, or operator value; it follows no redirect; it
discards the response; and it attributes a failure to the existing
`unsupported scheme` key with a specific sentence. Unknown extensions and `headless`
remain refused by M026.

## Current implementation evidence

- M026 adds `extensions` parsing and a fail-closed refusal for any non-empty list.
  Before M027, a `css-load` page is refused with zero requests.
- The extension path is fixed in the open-source Anubis source:
  `anubis.BasePrefix + "/.within.website/x/cmd/anubis/ext/css-load" + "/" + challengeID`.
  The challenge ID is the parsed `challenge.id`.
- The extension route does not require a cookie: `renderCSS` validates that the
  challenge ID was issued and marks it seen. Forwarding the verification cookie is
  unnecessary and is excluded by ADR-0012.
- The live deployments observed on 2026-11-13 publish `extensions: null`; no
  `css-load` deployment has been observed. The plan must treat a live capture as
  optional evidence, not an assumption.

## Non-goals

- No support for `headless`/BotStopper or any extension outside ADR-0012's class.
- No JavaScript, WebAssembly, CSS parsing, image loading, or rendering. The client
  only performs the HTTP fetch the extension checks.
- No new dependency, configuration key, MCP tool, response field, or ledger key.
- No change to the method solvers, the difficulty ceiling, the attempt budget, the
  cookie cache, or the fetch path.
- No retry: one extension request per extension per solve; a failure is terminal.
- No extension request when `[search.challenge].enabled` is false.

## Invariants that cannot regress

- The extension URL is constructed from the validated base prefix, the fixed
  extension path, and the validated challenge ID; it is never copied from page text.
- The request is same-origin, root-relative, redirect-disabled, deadline-bounded, and
  body-bounded (4 KiB, discarded).
- No cookie, authorization header, user agent override, request body, or operator
  value is sent.
- The challenge ID is validated (UUID/hex-and-hyphen shape, bounded length, no
  traversal or control characters) before it reaches a path.
- A non-2xx response, a transport error, or a timeout refuses before the proof is
  submitted, with `blocked` and a specific sentence; no pass request is sent.
- Unknown extensions and `headless` are refused by the M026 path without any
  extension request.
- The ten-key refusal vocabulary and `provider_status` shape are unchanged.
- One solve and one replay per fetch; one solve per provider per TTL on the engine
  path.

## Expected production changes

1. **Extension dispatch.** After M026's parse and before `check_ceiling`, partition
   the extension list:
   - every name is `css-load` → continue;
   - any other name → M026's refusal (zero requests);
   - empty → continue.
2. **One bounded GET.** Fetch
   `{base_url}{validated_base_prefix}/.within.website/x/cmd/anubis/ext/css-load/{validated_id}`
   with the solver client, no cookie header, redirects disabled, the remaining
   deadline, and a 4 KiB bounded body that is discarded. A 2xx continues; anything
   else returns a blocked sentence such as
   `anubis css-load extension was not satisfied (status 404)` and records
   `RefusalReason::UnsupportedScheme`.
3. **Ordering.** Perform the extension request immediately after parsing and before any
   method wait, hash search, or pass submission. A failing extension therefore costs no
   hashing and no wait; a satisfied one is followed by the method path and the pass
   request. The whole sequence stays inside the remaining call deadline.
4. **Tests.**
   - `css-load` 200 → exactly one extension GET, then one pass request.
   - `css-load` 404/500 → exactly one extension GET, zero pass requests, `blocked`
     with the named sentence.
   - Unknown extension → zero extension requests, zero pass requests.
   - `headless` → zero extension requests, zero pass requests.
   - Disabled challenge → zero extension requests.
   - A cross-origin base URL, a traversal-shaped ID, and a control-character ID are
     refused before any request.
   - A canary asserts no cookie or authorization header reaches the extension
     request.
   - A large extension body is bounded and discarded.
5. **Docs.** `architecture/engines.md` § Challenge solving, `docs/config.md`
   § Challenge Solving, `tests/fixtures/engines/README.md` if a live capture is
   obtained, `CHANGELOG.md`.

## Ordered work packages

1. Extension dispatch and ID/path validation, with refusal tests.
2. The bounded same-origin GET and its failure attribution.
3. Mock flows for satisfied, unsatisfied, disabled, and unknown cases.
4. Documentation, live record if reachable, and the closure record.

## Verification

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo test --locked --all-features --lib meta::challenge
cargo test --locked --all-features --lib meta::engines::startpage
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features
```

Live, recorded rather than asserted: find a deployment publishing
`"extensions":["css-load"]` and fetch it with challenge solving enabled. Record the
challenge method, the extension status, whether the pass request followed, and
whether the document was returned. If no deployment is reachable, the closure record
must name that limitation and mark the capability synthetic-only; do not claim a live
pass.

## Compatibility and closure evidence

No MCP tool, response-schema, or configuration-key change. The extension runs only
when `[search.challenge].enabled` is true and ADR-0012 is accepted; a deployment with
no extension list, or with an unknown extension, behaves exactly as M026 defines. No
new dependency or ledger key is introduced, and the ten-key vocabulary is unchanged.

Required closure evidence: the implementation commit; the focused and full test
outputs; the extension GET/pass flow and the failure, unknown, and disabled cases; a
live capture if a `css-load` deployment is reachable, or the named synthetic-only
limitation if it is not; the documentation updates; and the residual risk if the live
extension path differs from the open-source shape. The record must not claim a live
pass without a recorded URL and status.

## Acceptance and stop conditions

- A `css-load` challenge performs exactly one same-origin extension GET, then one
  pass request, and returns the document.
- An unsatisfied extension performs one GET, zero pass requests, and reports
  `blocked` with the specific sentence and `unsupported scheme`.
- Unknown and `headless` extensions perform zero extension and zero pass requests.
- The extension request carries no cookie or authorization header.
- No new dependency, key, tool, or response field.
- Stop and return for a decision if the live extension URL differs from the
  open-source path, if the extension requires a cookie or a header, if the challenge
  ID cannot be validated safely, or if satisfying it would require executing code.
  Do not widen ADR-0012's class in this plan.
