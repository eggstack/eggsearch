# M031 — Fetch-Path Challenge Solve Parity

Status: proposed

Class: capability

Source roadmap: plans/subsystems/agentic-retrieval-integrity-roadmap.md (M031)

Requires: ADR-0017 acceptance (hard dependency). Not dependency-ready until that ADR is accepted.

Planning baseline: 556462bb0b8e7c15150f20548e02fe00a1e03923 (current committed code candidate; M030 closure)

References: ADR-0009; ADR-0010; ADR-0012; ADR-0013; ADR-0017 (proposed); ADR-0016;
`architecture/fetch.md` § Challenge pages on the HTTP path; `architecture/engines.md` § Challenge
solving; `architecture/maintenance.md` § Plans registry; `src/fetch/challenge.rs`;
`src/mcp/tools/web_fetch.rs`; `src/mcp/tools/repo_fetch.rs`; `src/commands/fetch.rs`;
`src/meta/challenge/anubis.rs`.

## Objective

Every fetch entry point that can observe a published challenge attempts a bounded solve under the
operator's existing ceilings, through one shared implementation, and the recorded `batch_fetch`
residual is proven by test and its stale documentation corrected.

## Current implementation evidence

- `src/fetch/challenge.rs` classifies Anubis, ALTCHA, interactive, and verification walls from the
  title and body. `src/fetch/client.rs` attaches the classification as `structured_warnings` and
  sets `manual_interaction_required`, so every `FetchClient` caller marks a wall.
- `src/mcp/tools/web_fetch.rs` owns a private `replay_anubis_challenge` that solves a detected
  Anubis page once and replays once with the cookie pairs the pass response issued. It is reached
  from `run_web_fetch_inner`, after the browser-escalation branch, so a successful `render:
  "auto"` render keeps priority.
- `src/mcp/tools/batch_fetch_item.rs` calls `run_web_fetch_with_prepared_client`, which enters
  the same `run_web_fetch_inner`. The batch web path therefore executes the same solve.
  `architecture/fetch.md` still states that `batch_fetch` does not solve; that note was written on
  2026-11-13, before batch was routed through the shared web handler on 2026-11-17, and no test
  asserts the inherited behavior either way.
- `src/mcp/tools/repo_fetch.rs` contains no challenge handling. Its remote path fetches raw source
  through `FetchClient` and never classifies a wall, so a protected code host returns the wall as
  content.
- `src/commands/fetch.rs` calls `FetchClient::fetch` directly, below the tool layer. Its JSON
  payload carries `warnings` but not `structured_warnings` or `manual_interaction_required`, so a
  challenged page reaches the operator as ordinary content.
- The solver lives in `src/meta/challenge/` and the client in `src/fetch/`; `src/fetch/**` must
  stay independent of `src/meta/**`, and the orchestration currently sits inside a `pub(crate)`
  MCP tool module that the CLI cannot reach.

## Scope

In scope:

1. One shared orchestration module implementing detect, solve, replay, and classify, outside
   `src/fetch/**` and outside `src/meta/**`, callable from both `src/mcp/**` and
   `src/commands/**`.
2. The MCP web family moves onto that module with no behavior change.
3. `repo_fetch`'s remote path gains detection, marking, the solve, and one replay.
4. The CLI `eggsearch fetch` command gains detection, marking, the solve, and one replay.
5. A behavioral test that proves `batch_fetch` inherits the solve, and the correction of the stale
   `architecture/fetch.md` residual note to the verified behavior.

Out of scope: ALTCHA on the general fetch path (M032); fetch-side pass-cookie reuse (M033); Anubis
`argon2id` and `hashx`; any new, renamed, or removed configuration key; any change to the
shipped defaults; any MCP tool, argument, or advertised-schema change; any new dependency; any
change to ADR-0009's prohibition list; interactive CAPTCHA, OCR, solving services, proxy rotation,
or user-agent spoofing.

## Invariants that cannot regress

- No file under `src/fetch/**` imports `crate::meta`, the shared orchestration module, or the
  solver. Detection and marking stay in the fetch layer for every caller.
- The advertised tool inventory and every advertised schema are unchanged.
- At most one solve and one replay per fetch call, on every surface.
- Every solve stays inside `max_difficulty`, `max_attempts`, `timeout_ms`, and the call
  deadline. The attempt ceiling binds before the wall clock and the refusal names the operative
  ceiling.
- The replayed cookie is attached on the first hop only, so a redirect cannot forward it to another
  origin.
- A refused solve, a transport error, an exhausted deadline, a malformed challenge, or a replay
  that is itself a challenge leaves the original response exactly as it is.
- No cookie, nonce, salt, challenge id, or pass token appears in a log, an error, a warning, a
  `Debug` output, a fixture, or a `provider_status` block.
- A browser render under `render: "auto"` keeps priority over the solver.
- Solving stays opt-in and off by default. With the opt-in off, every surface behaves exactly as it
  does today.
- The refusal vocabulary stays fixed at its ten keys and gains no new member.
- `repo_fetch`'s workspace path performs no network request and is unchanged.

## Expected production changes

1. **A shared orchestration module.** Its name and location are the implementer's choice within the
   ADR-0017 constraint: it must not live under `src/fetch/**` or `src/meta/**`, it may depend on
   both `crate::fetch` and `crate::meta::challenge`, and neither of those trees may depend on
   it. It exposes one operation that takes a fetch response (with its bounded raw body), the
   challenge configuration, a client, the request URL, the captured challenge cookies, and the
   remaining deadline, and returns an optional replacement response. It owns the whole
   detect-then-solve-then-replay sequence, including the "the replay is still a challenge" check.
2. **The MCP web family moves onto that module.** `src/mcp/tools/web_fetch.rs` drops its private
   `replay_anubis_challenge` in favor of the shared call. Behavior must be identical, including
   the browser-first ordering, the one-solve-one-replay rule, and the unchanged response when a
   solve is refused.
3. **`repo_fetch`'s remote path classifies and solves.** The remote raw-source response is
   classified with the shared detector; when the operator opted in, the shared orchestration
   attempts one solve and one replay and the replayed document is returned. The additive
   `manual_interaction_required` and `fetch_challenge_page` fields appear on a detected wall.
   The workspace path is untouched.
4. **The CLI classifies and solves.** `src/commands/fetch.rs` classifies its response and, when
   the operator opted in, attempts one solve and one replay through the shared module. Its client
   construction, its lack of a cache, its payload key set, and its text and JSON output modes are
   unchanged; the two challenge fields are added to the JSON payload and to the human-readable
   output.
5. **Nothing else changes.** No configuration key, no default, no dependency, no solver ceiling, no
   refusal key, and no advertised schema.

## Migration and compatibility effects

- No MCP tool, argument, or advertised-schema change; the compatibility aliases keep their
  contracts.
- `repo_fetch` remote responses gain two additive fields on a detected wall. `docs/tool-matrix.md`
  must record this.
- The CLI JSON payload gains the same two fields on a detected wall; every other key is unchanged,
  and the human-readable output gains the same information the MCP tool already shows.
- `[search.challenge].enabled` remains the single opt-in, remains off by default, and its meaning
  now covers fetch surfaces in addition to search upstreams.
- No storage, migration, or protocol change.

## Ordered work packages

1. **Prove the batch inheritance before touching the note.** Add a behavioral test that runs a
   batch web item against a published Anubis wall with solving enabled and asserts the replayed
   document, `manual_interaction_required: false`, and no `fetch_challenge_page` warning. Then
   correct `architecture/fetch.md` to state the verified behavior and remove the stale residual
   sentence. If the test fails, the residual was real: record that and treat the batch path as a
   fourth consumer in work package 2 rather than editing the note to match the expectation.
2. **Land the shared orchestration module** with the existing MCP web behavior unchanged, and move
   `src/mcp/tools/web_fetch.rs` onto it. The MCP web behavior must be identical before and after.
3. **Wire `repo_fetch`'s remote path** to the shared module, with its additive response fields.
4. **Wire the CLI** to the shared module, with its additive output fields and no other change.
5. **Update the documentation set:** `architecture/fetch.md` § Challenge pages on the HTTP path
   and its module inventory, `architecture/overview.md` module dependency flow and component index,
   `architecture/maintenance.md` ownership table if a new module is added, `docs/features.md` for
   the solve reach, `docs/tool-matrix.md` for the `repo_fetch` additive fields, and
   `skills/eggsearch-mcp` only if a caller-visible rule changed. Update `AGENTS.md` only if a
   contributor rule changed, and then as an index into `architecture/`.
6. **Run the verification below** and record the outcomes.

## Verification

Focused tests, using the repository-pinned Rust 1.89 toolchain:

```bash
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --test web_fetch_integration
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --test batch_fetch_retrieval
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --lib meta::challenge
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH cargo test --locked --all-features --test static_guards
```

Broad gate:

```bash
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH make check
PATH=/Users/davidbowman/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$PATH make docs-check
```

The new tests must use loopback fixtures, never a public upstream, and must not require network
access, credentials, or a browser.

## Static guards and documentation to update

- The existing guard that keeps `src/fetch/**` free of `crate::meta` imports must still pass.
- Add a guard that no `src/fetch/**` file imports the shared orchestration module or the solver, and
  that the shared module is the only place combining the solver with the fetch client.
- Add a guard that the CLI and `repo_fetch` call the shared module rather than re-implementing
  detect, solve, or replay, so a second copy cannot appear.
- Documentation: `architecture/fetch.md`, `architecture/overview.md`,
  `architecture/maintenance.md`, `docs/features.md`, `docs/tool-matrix.md`, and
  `plans/subsystems/agentic-retrieval-integrity-roadmap.md` plus `plans/registry.md` at closure.

## Acceptance criteria

1. A batch web item against a published Anubis wall returns the replayed document with
   `manual_interaction_required: false` and no `fetch_challenge_page` warning, proven by a test
   in the behavioral suite.
2. `architecture/fetch.md` states the verified batch behavior and no longer claims that
   `batch_fetch` does not solve.
3. `repo_fetch`'s remote path classifies a published Anubis wall and, with the opt-in enabled,
   returns the replayed document; with the opt-in disabled it returns the marked wall exactly as it
   does today.
4. The CLI `eggsearch fetch` command classifies a published Anubis wall and, with the opt-in
   enabled, returns the replayed document and its challenge fields; with the opt-in disabled its
   output is unchanged from the baseline apart from the two added challenge fields.
5. One implementation of detect, solve, replay, and classify serves all three surfaces, and a
   static guard fails closed if a second copy appears.
6. No file under `src/fetch/**` imports the solver or the shared orchestration module.
7. The MCP web family is byte-for-byte unchanged in behavior: the same response on a solved wall,
   the same marked response on a refused or disabled solve, the same browser-first ordering, and at
   most one solve and one replay.
8. No configuration key, default, dependency, refusal key, advertised tool, or advertised schema
   changed.
9. The broad gate passes on the exact candidate, and every test that was run is recorded with its
   outcome.

## Stop conditions

Stop and leave M031 open if ADR-0017 has not been accepted; if the batch inheritance test fails
and the batch path turns out to need its own solve path; if the shared module cannot be placed
without violating the `src/fetch/**` independence guard; if the CLI's existing output contract
cannot be preserved alongside the two added challenge fields; if the solve contract would need more
than one solve or one replay per call to work; or if any implementation would require a new
dependency, a configuration migration, or a relaxation of ADR-0009's prohibition list.

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/031-status.md` records the implementation commits, a
requirement-to-evidence matrix mapping each acceptance criterion to the test or command that
satisfies it, the focused and broad command outcomes including anything unrun, the unchanged-safety
evidence (the MCP web behavior comparison and the prohibition list), the documentation set that was
updated, known limitations, residual risk, and the recommendation. The closure commit carries this
plan, the closure record, the roadmap status, and the registry row together.
