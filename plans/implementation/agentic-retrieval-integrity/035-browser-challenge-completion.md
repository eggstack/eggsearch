# M035 — Browser Challenge Completion

Status: ready
Date: 2026-11-19
Subsystem: agentic retrieval integrity
Milestone class: capability
Decision authority: ADR-0019 (accepted), which authorizes script-completed interstitials as a
member of the permitted challenge class.

## Objective

Make the shipped browser a *completion* mechanism for script-driven anti-bot walls, and stop it
from shadowing the cheap solver. With `[fetch].browser.policy = auto`, a fetch that meets a wall
must end with one of exactly two outcomes: the requested document, or a challenge page that is
marked as one. It must never end with a wall passed off as a document, and a solvable puzzle must
never be handed to a browser before the solver has had it.

## Evidence

Collected before this plan, all reproducible:

1. **Measured ordering defect.** The M033 live qualification (candidate `5cf21e8`) recorded that
   with the shipped `policy = auto`, the MCP fetch path escalates to Chrome before the solver runs.
   Chrome received Anubis's "Oh noes!" error page rather than a challenge page, the call returned
   `escalated = true`, and the solver was skipped, so no solve was recorded and no document was
   returned. See `plans/closure/agentic-retrieval-integrity/034-status.md` § Live qualification.
2. **Completion is title-only.** `src/fetch/browser/navigate.rs` polls every 500 ms and decides the
   verification cleared by testing four title substrings — `just a moment`, `checking`,
   `verifying`, `please wait`. A wall that clears without changing its title is treated as
   unresolved; a legitimate page whose title contains one of those strings is treated as a wall.
3. **Classification already exists and is not reused.** `src/fetch/browser/classify.rs` classifies
   status, content type, title, text length, and body into `UsefulContent`, `JavascriptShell`,
   `NonInteractiveVerification`, `InteractiveChallenge`, `RateLimited`, `AccessDenied`,
   `AuthenticationRequired`, `ServerError`. The post-wait loop does not call it.
4. **Solver and browser are mutually exclusive.** `src/mcp/tools/web_fetch.rs` runs the browser
   escalation, and the challenge-replay block beneath it is guarded by `if !escalated`. No path
   lets one mechanism hand off to the other inside a single call.
5. **Browser completions are not observable.** `provider_status.challenge_solver.providers` reports
   `solves_attempted`, `solves_succeeded`, `cookie_hits`, and `cookie_misses`. A wall completed by
   the browser appears in no counter.

## Scope

In scope:

- the escalation order between `src/challenge_replay/` and `src/fetch/browser/` on the fetch path;
- live re-classification during completion, reusing `classify_response`;
- the outcome when a wall does not resolve;
- a bounded, secret-free completion counter in the existing challenge ledger surface;
- the fetch-path surfaces that already escalate: MCP `fetch` (and its legacy aliases), `repo_fetch`,
  and the CLI.

Out of scope:

- new browser capabilities, a new render mode, or a new config key beyond what ADR-0019 requires;
- `RenderPolicy::HttpOnly` and `RenderPolicy::Browser` semantics, which are explicit operator
  choices and keep their meaning;
- Cloudflare Turnstile/reCAPTCHA-class interactive challenges, which stay
  `manual_interaction_required`;
- `402 Payment Required` and `403` policy refusals, which are reported, never completed;
- the puzzle-class and identity axes of ADR-0019, which are separate milestones.

## Invariants

- `src/fetch/**` stays independent of `src/meta/**`; shared orchestration stays in
  `src/challenge_replay/`. The browser stays behind the optional `browser` feature and its
  existing render and session caps.
- At most one solver attempt pair and one browser escalation per fetch call, inside the call
  deadline. No retry widening.
- Every wall must be either completed or marked. A challenge page is never returned as an
  unmarked document, and a document is never reported as a challenge.
- Interactive challenges, authentication walls, and policy refusals are never attempted.
- No cookie, nonce, salt, challenge id, pass token, or profile path in logs, errors, `Debug`,
  fixtures, or `provider_status`.
- No new dependency. `make check` must pass keyless and without a browser: browser tests are
  feature-gated or `#[ignore]`d, and the ordering logic must be testable without Chrome.
- No explanatory code comments beyond required doc comments; `cargo fmt` clean.

## Work packages

1. **Paper first.** Add § "Browser challenge completion" to `architecture/fetch.md` stating the
   order of mechanisms, the bounded wait, the live re-classification rule, and the
   completed-or-marked invariant. This lands before any code, as M034 did.
2. **Ordering.** When a wall is detected, attempt the cheap solver first (Anubis/ALTCHA
   classification from `src/fetch/challenge.rs`) and escalate to the browser only when the wall is
   script-driven (`JavascriptShell`, `NonInteractiveVerification`) or the solver did not produce a
   document. Remove the `if !escalated` mutual exclusion in favour of an explicit ordered sequence
   with per-step accounting.
3. **Completion.** Replace the four-title test with `classify_response` over live page state
   (title, `document.body.innerText` length, body snippet), polling inside
   `verification_wait_ms` and the call deadline, and stopping only on `UsefulContent`. Keep
   `InteractiveChallenge` an immediate manual-interaction error.
4. **Honest outcome.** If the wait budget or the deadline expires while the page is still a wall,
   return the response with the existing `fetch_challenge_page` warning and
   `manual_interaction_required` as applicable — never the bare wall.
5. **Observability.** Add a browser-completion counter beside the existing solve counters in
   `src/meta/challenge/ledger.rs`, reporting counts only. Document it in `docs/features.md` and the
   `provider_status` schema description.
6. **Tests.** Extend `tests/web_fetch_integration.rs` with loopback fixtures proving: a puzzle wall
   escalates to the solver, not to the browser, under `auto`; a script-driven wall that resolves
   returns the document; an unresolved wall returns the marked challenge page; an interactive wall
   never reaches completion. Extend `tests/dispatch_fault_injection.rs` for the unavailable-browser
   and expired-deadline paths. A live browser record is optional and, if taken, is recorded as
   live evidence and never substitutes for the deterministic tests.
7. **Docs.** `docs/features.md`, `architecture/engines.md` § Challenge solving, `docs/test-inventory.md`,
   and a `CHANGELOG.md` entry under the existing unreleased section.

## Verification

```bash
cargo test --locked --all-features --test web_fetch_integration
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test static_guards
cargo test --locked --all-features --lib challenge_replay
cargo test --locked --all-features --grep browser  # feature-gated suite
make check
make docs-check
./packaging/check-repo-hygiene.sh
```

## Acceptance criteria

- A fetched Anubis or ALTCHA wall under `policy = auto` is solved, not escalated to the browser,
  and the document is returned; no browser process is launched for it.
- A script-driven wall that resolves inside `verification_wait_ms` returns the document, and
  `provider_status` shows exactly one browser completion.
- A wall that does not resolve returns the response with a challenge warning, and the response is
  never a bare wall.
- An interactive challenge still returns `manual_interaction_required` and attempts no completion.
- No more than one solver attempt pair and one escalation occur per call, verified by call counts
  in the loopback fixtures.
- `make check` passes with 0 failures, keyless and browser-free; `make docs-check` passes.

## Stop conditions

Stop and report rather than working around any of these:

- the ordering fix cannot be expressed without coupling `src/fetch/**` to `src/meta/**`;
- a completion requires a new dependency, a new config key, or a change to `RenderPolicy`;
- a deterministic test for the ordering needs a real browser, which would put Chrome in the default
  gate;
- a policy refusal (`402`, `403`) would need to be attempted to satisfy an acceptance criterion.

## Required closure evidence

A closure record at `plans/closure/agentic-retrieval-integrity/035-status.md` with: the exact
candidate SHA; a requirement-to-evidence matrix; the three suite results; the `make check` suite
and test counts; the observed ordering before and after, with the call counts from the fixtures;
what was and was not demonstrated live; and any residual risk, including behaviour under a wall
that is neither a puzzle nor a script.
