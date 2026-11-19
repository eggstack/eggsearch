# ADR-0017: Fetch-Path Challenge Resolution

Status: accepted

Date: 2026-11-19

Accepted: 2026-11-19

Decision owners: project maintainers

Related decisions: ADR-0009, ADR-0010, ADR-0011, ADR-0012, ADR-0013, ADR-0016

Affected subsystem roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md

## Context and forces

ADR-0009 permitted bounded in-process solving of published computational challenges, and scoped
the permission to "keyless HTML upstreams" — the search engines. M023 later reused that solver on
the fetch path, so `web_fetch` attempts one bounded solve and one replay when it meets a published
Anubis wall. That reuse was never written down as a scope decision: the fetch path is not a search
upstream, and ADR-0009's permitted class does not name it.

The measured state of the repository at baseline `556462bb0b8e7c15150f20548e02fe00a1e03923`:

- The solver is a reviewed module, `src/meta/challenge/`, with four ADRs governing what it may
  attempt. The detection layer, `src/fetch/challenge.rs`, classifies Anubis, ALTCHA, interactive,
  and verification walls for every caller of `FetchClient`.
- The solve is reachable only from the MCP web family. `src/mcp/tools/web_fetch.rs` owns a private
  `replay_anubis_challenge`; `src/mcp/tools/batch_fetch_item.rs` reaches it through the shared
  web-fetch handler, so the batch path inherits it.
- `src/mcp/tools/repo_fetch.rs` contains no challenge handling at all. Its remote path fetches raw
  source through `FetchClient` and never classifies a wall.
- `src/commands/fetch.rs` calls `FetchClient::fetch` directly, below the tool layer. Its payload
  carries neither `structured_warnings` nor `manual_interaction_required`, so an Anubis wall is
  presented to the operator as ordinary content.
- The fetch path detects ALTCHA but never solves it. `src/meta/challenge/altcha.rs` takes a
  challenge URL and a verify URL, and its only caller supplies Mojeek's fixed same-origin paths, so
  a general page's embedded challenge has no reviewed path to a solve.
- A fetch-side pass cookie is never reused. Every challenged fetch pays a full solve, and the
  upstream-published lifetime that M014 made safe to parse is not applied on the fetch path.

A second discrepancy is documentary. `architecture/fetch.md` records that `batch_fetch` does not
solve. That note was written on 2026-11-13; batch web items were routed through the shared web-fetch
handler on 2026-11-17. The code path suggests the note is now false, and no test proves the
inherited behavior either way.

The architectural tension is that the solver lives in `src/meta/`, the fetch client in
`src/fetch/`, and `src/fetch/**` must stay independent of `src/meta/**`. Only `src/mcp/**` and
`src/commands/**` may see both trees, and the orchestration today sits inside a `pub(crate)` MCP
tool module, so the CLI cannot reuse it.

The forces are: (a) keep the permitted class narrow and machine-verifiable, with ADR-0009's
prohibitions untouched; (b) keep one implementation of the solve-and-replay contract rather than a
copy per surface; (c) keep solving opt-in and off by default, and keep the shipped defaults
unchanged; (d) keep detection and marking in the fetch layer for every caller; (e) record the
upstream coverage risk without pretending it is solved.

## Considered alternatives

1. Keep the solve where it is and accept that `repo_fetch`, the CLI, and ALTCHA do not solve.
   Rejected: the asymmetry is an accident of where the orchestration was placed, not a policy. A
   capability whose cost is already paid is worse when only one surface can reach it.
2. Move the solver into `src/core` so every layer can call it. Rejected for now: `core` is the
   pure, HTTP-free foundation the whole crate rests on, and the solver needs `eggfetch-core`
   transport plus `http::HeaderMap`. Adding a transport dependency to `core` changes a boundary
   far wider than this capability. Reconsider only if a consumer appears outside the MCP and CLI
   surfaces.
3. Let `src/fetch/**` depend on `src/meta/challenge/**`. Rejected: it inverts the documented
   dependency flow for one feature and weakens the guard that keeps the fetch layer transport-only.
4. Give the CLI its own solve-and-replay implementation. Rejected: a second copy of the
   orchestration drifts from the first, and the repository already forbids per-engine solver copies.
5. Introduce one shared orchestration module that depends on both trees and is called by the MCP
   tool layer and the CLI. Selected: one implementation, one contract, no change to the fetch
   layer's independence.
6. Route the CLI through the SDK or the MCP handler so it inherits the whole tool behavior.
   Rejected for now: it would also hand the CLI the MCP cache, the browser escalation, and the
   MCP payload shape. That is a larger user-visible change than the capability requires, and it
   would need its own measured acceptance.

## Decision

1. **ADR-0009's prohibition list is unchanged.** Interactive CAPTCHA and visual puzzle answering,
   OCR of challenge images, third-party or human solving services, credential or login bypass,
   session or cookie theft, proxy rotation, user-agent spoofing, and retry widening to conceal an
   outage remain prohibited. Nothing in this ADR reopens them, and no milestone may weaken them.
2. **The permitted class for the general fetch path is exactly the evaluated set**: Anubis `fast`,
   `slow`, `preact`, `metarefresh`, and WASM `sha256` (ADR-0010 and ADR-0013), plus ALTCHA
   PBKDF2 challenges. A method or extension outside that evaluated set still fails closed as
   `unsupported scheme`; it does not inherit permission from this list.
3. **Solving is extended from search upstreams to the general fetch path.** The opt-in remains the
   single existing key, `[search.challenge].enabled`. It is not renamed, no second switch is
   added, and the shipped default stays `false`. The key name predates this decision and now
   governs both search upstreams and fetch paths; this ADR records that rather than migrating it.
4. **One shared orchestration module owns detect, solve, replay, and classify.** It sits outside
   `src/fetch/**` and outside `src/meta/**`, so both `src/mcp/**` and `src/commands/**` may call
   it, and it is the only place permitted to combine the solver with the fetch client.
   `src/fetch/**` keeps detection and marking only and must never import the solver or the
   orchestration module.
5. **The solve contract is unchanged where it already exists**: at most one solve and one replay per
   fetch call, inside `max_difficulty`, `max_attempts`, `timeout_ms`, and the call deadline; the
   replayed cookie is sent on the first hop only; a refused solve, an error, an exhausted deadline,
   or a replay that is itself a challenge leaves the original response exactly as it is; no cookie,
   nonce, salt, challenge id, or pass token appears in logs, errors, warnings, `Debug`, or
   fixtures; a browser render under `render: "auto"` keeps priority over the solver.
6. **`repo_fetch`'s remote path gains detection, marking, and the solve.** Its workspace path is
   unchanged: no network, no challenge, no cookie.
7. **The CLI `eggsearch fetch` command gains detection, marking, and the solve.** Its own client
   construction, its lack of a cache, its payload keys, and its text and JSON output modes are
   unchanged. It does not gain the MCP cache, the browser escalation, or the MCP payload shape.
8. **A fetch-side pass cookie may be reused for the same origin**, within an upstream-published
   lifetime and the operator's `cookie_ttl_seconds` ceiling, under M014's expiry-safety rules:
   never past a published expiry, never on a malformed lifetime, and evicted when the upstream
   rejects it. This is separate work and is not authorized by any other clause of this ADR.
9. **ALTCHA on the general fetch path requires safe same-origin derivation** of the challenge and
   verify endpoints from what the page publishes. An endpoint that is not same-origin, or that
   cannot be derived without executing code, fails closed as `unsupported scheme`. The permitted
   extension class stays ADR-0012's `css-load` only.
10. **Anubis `argon2id` and `hashx` remain `unsupported scheme`.** Upstream v1.28.2 (released
    2026-10-23) ships them disabled by default with stated intent to enable them by default in a
    later version. Each needs its own ADR with cost and dependency evidence. This ADR records the
    coverage risk; it does not pre-approve the methods.
11. **The stale `batch_fetch` residual note is corrected only after a test proves the inherited
    behavior.** Documentation is not corrected ahead of its evidence.

## Consequences and tradeoffs

Advantages: one implementation of the solve contract serves every fetch surface; the CLI and
`repo_fetch` stop presenting a challenge wall as ordinary content; the ALTCHA scheme that already
exists stops being engine-coupled; a repeated fetch against a protected origin stops paying a full
solve per call.

Tradeoffs: the repository gains one module that depends on both `meta` and `fetch`, which the
dependency-flow diagram must now show; the CLI gains a capability whose evidence must be produced
without the MCP tool layer's cache and escalation behavior; a fetch-side cookie cache is a new
stateful surface with its own bounds and secret-handling rules; and the upstream WASM coverage gap
remains open by design until its own ADR.

## Compatibility, security, and migration

No MCP tool, argument, or advertised-schema change. No configuration key is added, removed, or
renamed, and the shipped defaults are unchanged. No new dependency: `sha2`, `pbkdf2`, and
`base64` are already direct dependencies, and `reqwest` stays out of `src/`. `repo_fetch`
remote responses gain `manual_interaction_required` and a `fetch_challenge_page` structured
warning on a detected wall — additive fields on an existing response that must be reflected in
`docs/tool-matrix.md`. The CLI payload gains the same two fields for a detected wall and keeps
its existing key set otherwise.

Every solve stays behind the existing bounded-body, deadline, SSRF, and redirect rules, and every
request continues through the existing sanitization and trust-framing pipeline. Pass cookies remain
secret-like and must never appear in a log, an error, a warning, `Debug`, a fixture, or a
`provider_status` block.

## Verification and status

Adoption authorizes the class, the seam, and the surface coverage. It does not change behavior.
Implementation lands through M031 (solve parity across fetch surfaces), M032 (ALTCHA on the general
fetch path), and M033 (fetch-side pass-cookie reuse), in that order. Each milestone closes on its
own exact candidate with its own closure record under
`plans/closure/agentic-retrieval-integrity/`, and the roadmap and registry rows update in the same
closure commit. Acceptance makes M031 dependency-ready; M032 and M033 remain dependent on M031's closure.
