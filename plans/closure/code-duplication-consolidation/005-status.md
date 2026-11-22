# M005 Shared Fetch Escalation Seam — Closure Evidence

Status: closed

Candidate: the working tree on `feat/auto-20261117-044c4885` above baseline
`fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64`, uncommitted because the request did not ask for a
commit. The canonical gate ran on exactly this tree; its output and the diff fingerprint are in
`plans/closure/code-duplication-consolidation/gate.md`.

Source plan: `plans/implementation/code-duplication-consolidation/005-shared-fetch-escalation-seam.md`

Roadmap: `plans/subsystems/code-duplication-consolidation-roadmap.md` M005

Decision authority: accepted ADR-0017 §4, ADR-0019 §6, ADR-0014. No new ADR, flag, or response
field.

## Change

- `src/fetch/browser/escalation.rs` (433 lines including tests) owns `gated_status`,
  `EscalationRequest`, `EscalationOutcome`, and `escalate(request, render)`: the availability
  check, the 2 s remaining-time floor, the timed caller-supplied render, error mapping, the
  profile-aware manual-interaction action, the `rendered_result_serves` return rule, and the
  DOM-to-response conversion. It records nothing; `challenge_completed` is returned.
- `src/mcp/tools/web_fetch_escalation.rs`: 199 → 112 lines. It builds the request, passes
  `run_browser_fetch`, and keeps MCP permits, profiles, origin-success recording, the
  completion ledger, and `browser_escalated`.
- `src/commands/fetch.rs`: 774 → 714 lines. The CLI copy (`gated_status`,
  `escalate_gated_status`, `gated_error`) is deleted; its direct `browser` branch, output
  keys, exit codes, flags, and completion ledger stay as they were.
- The CLI's two distinct unavailable reasons are preserved: the request carries an optional
  `unavailable_reason`, so a discovered-but-unavailable lifecycle still says
  "no Chrome/Chromium executable was found" rather than collapsing into "no browser is
  available". This is the only caller-visible wording difference the seam needed to keep.
- `src/fetch/browser/escalation.rs` imports only `crate::core` and `crate::fetch`; no
  layering inversion.

## Equivalence evidence

- `cargo test --locked --all-features --lib fetch::browser`: 108 passed, including nine new
  fake-render tests plus `unavailable_reason_is_preserved_for_the_caller` (ten new cases in
  total).
- `cargo test --locked --all-features --test web_fetch_integration`: 128 passed, 1 ignored,
  including the six unchanged M046 cases (gated 429, gated 500, unserved render, unavailable
  escalation, refusal status, `http_only`).
- `cargo test --locked --all-features --bin eggsearch commands::fetch`: 8 passed, including
  `cli_render_policy_reads_the_operator_setting`.
- `cargo check --locked --no-default-features`: passed.

## Residual risk

`architecture/fetch.md` does not describe the CLI copy as current, so no prose correction was
needed there. The M046 closure record's historical residual-risk sentence is append-only
history and was not rewritten. The MCP adapter still owns completion and origin-success
recording by design; that is the one remaining caller step.
