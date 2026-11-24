# M005 — Shared Fetch Escalation Seam

Status: closed
Closure: `plans/closure/code-duplication-consolidation/005-status.md`
Date: 2026-11-22
Subsystem: code duplication consolidation
Milestone class: infrastructure (behaviour-preserving)
Baseline: `fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64` on `feat/auto-20261117-044c4885`
Decision authority: accepted ADR-0017 §4 (one orchestration module above `fetch`),
ADR-0019 §6 (refusals are never answered), ADR-0014 (render bounds). No new ADR; this milestone
discharges the duplication that `plans/closure/agentic-retrieval-integrity/046-status.md`
recorded as residual risk.

References: `plans/subsystems/code-duplication-consolidation-roadmap.md` §4, §7;
`architecture/fetch.md` § Browser challenge completion; `docs/tool-matrix.md` (render policy);
`plans/closure/agentic-retrieval-integrity/046-status.md`.

Dependencies: none. Interface: ADR-0017 §4 and the M046 acceptance cases.

## Objective

Give the MCP fetch surfaces and the CLI one implementation of the gated-status escalation: the
availability check, the 2 s remaining-time floor, the timed render, the return rule, the manual
interaction message, and the completion accounting. The render stays caller-supplied so the MCP
keeps its permits, profiles, screenshots, and mock override and `fetch` never depends on `mcp`.

## Current evidence

Measured 2026-11-22 on the baseline:

- MCP: `src/mcp/tools/web_fetch_escalation.rs:78` (`gated_status`) and `:93`
  (`escalate_gated_status`), plus the `GatedEscalation` outcome and the `unavailable` helper.
- CLI: `src/commands/fetch.rs:281` (`gated_status`), `:299` (`escalate_gated_status`), and
  `:388` (`gated_error`). The CLI copy repeats the availability check, the 2 s floor, the timed
  render, the `InteractiveChallenge` message, the `rendered_result_serves` return rule, the
  completion counter, and the DOM-to-response conversion.
- M046's closure records why the first sharing attempt stopped: `run_browser_fetch` owns the
  global and per-origin permits, the profile lifecycle, and the mock-browser override, and lives
  at `src/mcp/tools/common.rs:830`; `challenge_replay` sits below `mcp`, so calling it from there
  would invert layering. The closure names roughly 40 lines of orchestration as the residual cost
  and `src/commands/fetch.rs` as the copy.
- The decision functions are already shared: `is_browser_gated_status`
  (`src/fetch/challenge.rs`), `rendered_result_serves` (`src/fetch/browser/classify.rs`), and
  `browser_result_to_response`. Only the sequence around them is duplicated.
- Drift is already visible: the CLI hardcodes `--profile <name>` in the login hint, while the MCP
  message names the profile actually used.

## Invariants that cannot regress

- Exactly one escalation per call, inside the caller's deadline, with the 2 s remaining-time
  floor and the existing timeout.
- `http_only` never renders; `browser` stays a direct render; `auto` escalates only for the
  gated status class (`429` and `5xx`).
- `401`, `402`, `403`, and `404` never reach the browser (ADR-0019 §6).
- The return rule is unchanged: a rendered document replaces the error only when it ended on a
  `2xx`/`3xx` document status or completed a recognised wall; otherwise the original
  `http_status` error is returned with the attempted escalation named.
- Completion accounting is unchanged: `record_browser_completion` runs only for a completed wall.
- The MCP keeps its global and per-origin permits, profile lifecycle, screenshot handling,
  `browser_escalated` flag, and origin-success recording. The CLI never gains profiles,
  screenshots, a transport field, or a new flag.
- Layering: `src/fetch/**` never imports `src/mcp/**` or `src/meta/**`; `challenge_replay` stays
  above both.

## Work packages

1. **Add `src/fetch/browser/escalation.rs`** (feature-gated `browser`) with:
   - `pub fn gated_status(error: &FetchError, policy: &RenderPolicy) -> Option<u16>`;
   - `pub struct EscalationRequest<'a>` carrying `url`, `status`, `policy`, `browser_available`,
     `profile_name: Option<&'a str>`, `budgets`, `extract_mode`, `include_links`,
     `sanitize_output`, and `deadline`;
   - `pub enum EscalationOutcome` with `Served { response, challenge_completed }`,
     `Failed { error, escalation_unavailable }`, and `ManualInteraction { origin, message, next_action }`;
   - `pub async fn escalate<F, Fut>(request: EscalationRequest<'_>, render: F) -> EscalationOutcome`
     where `F: FnOnce() -> Fut`, `Fut: Future<Output = Result<BrowserFetchOutcome, BrowserFetchError>>`.
   The helper performs the availability check, the 2 s floor, `tokio::time::timeout(remaining, render())`,
   the `InteractiveChallenge` mapping (including the profile-aware `next_action`), the other error
   mappings, the `rendered_result_serves` check, and the DOM-to-response conversion. It does
   **not** call `record_browser_completion`; it returns `challenge_completed` so the caller keeps
   the existing ledger call (a `fetch` module may not depend on `challenge_replay`).
2. **Collapse `src/mcp/tools/web_fetch_escalation.rs`** to the MCP adapter: build the request,
   pass `|| run_browser_fetch(state, url, sanitize, policy, profile_dir, false)` as the render,
   record the origin success and the completion flag, set `browser_escalated`, and keep
   `GatedEscalation` as the mapping the caller already understands. Delete `gated_status` and
   the duplicated sequence.
3. **Adopt it in `src/commands/fetch.rs`** with `|| browser_fetch_with_policy(...)` as the render
   and the CLI policy value. Delete `gated_status`, `escalate_gated_status`, and `gated_error`
   from the command module; keep the CLI's text/JSON output shape unchanged.
4. **Unit-test the helper without a network or a browser**: a fake render future proves the
   unavailable path, the 2 s floor, the timeout mapping, the manual-interaction message for
   `Some(profile)` and `None`, the return rule for a served and an unserved render, and that
   `challenge_completed` is surfaced rather than recorded inside the helper.
5. **Re-run the M046 acceptance cases** unchanged:
   `web_fetch_auto_escalates_a_gated_429_to_the_browser`,
   `web_fetch_auto_escalates_a_gated_500_to_the_browser`,
   `web_fetch_auto_keeps_the_error_when_the_render_did_not_improve`,
   `web_fetch_auto_names_the_unavailable_escalation`,
   `web_fetch_auto_does_not_escalate_a_refusal_status`,
   `web_fetch_http_only_never_escalates_a_gated_status`, and the CLI unit test
   `cli_render_policy_reads_the_operator_setting`.
6. **Correct the recorded risk** in `architecture/fetch.md` if it still describes the CLI copy as
   the status quo, and note the one remaining caller step (completion recording) in the closure
   record.

## Expected production changes

| File | Change |
|---|---|
| `src/fetch/browser/escalation.rs` | New shared orchestration and outcome type |
| `src/fetch/browser/mod.rs` | Module declaration |
| `src/mcp/tools/web_fetch_escalation.rs` | Collapses to the MCP adapter |
| `src/commands/fetch.rs` | Deletes its copy and calls the shared helper |

No tool, argument, schema, configuration, or response-field change.

## Non-goals

- Moving `run_browser_fetch` itself out of `mcp`; the permits and mock override stay there.
- Changing the browser lifecycle, concurrency caps, or render policy.
- Adding a CLI flag or a transport field to CLI output.
- Sharing the challenge solver path, which already lives in `challenge_replay`.
- Extending escalation to `repo_fetch`'s forge API path.

## Compatibility and migration

None. The CLI keeps its output keys and exit codes; the MCP keeps every response field. The only
observable difference is that the CLI's manual-interaction hint now names the configured profile
when one is set, matching the MCP message.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test web_fetch_integration
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib fetch::browser
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --bin eggsearch commands::fetch
RUSTUP_TOOLCHAIN=1.89.0 cargo check --locked --no-default-features
RUSTUP_TOOLCHAIN=1.89.0 make check
```

Equivalence evidence: the six M046 integration cases and the CLI policy test pass without an
assertion change; the new helper unit tests prove the mapping the CLI previously had no test for.

## Acceptance and stop conditions

- One implementation of the availability check, 2 s floor, timed render, return rule, and message
  construction; both callers use it.
- No layering inversion: `src/fetch/browser/escalation.rs` imports only `crate::fetch` and
  `crate::core`.
- The M046 cases pass unchanged; the CLI surface is unchanged.
- **Stop:** if the callback cannot preserve the MCP mock-browser override
  (`state.browser_fetch_override`) or the profile lifecycle, keep the MCP adapter as it is, leave
  the CLI on the shared decision functions, and record the residual duplication again. Do not
  move `run_browser_fetch` into `fetch` to make the callback unnecessary.

## Required closure evidence

The before/after line counts for both call sites, the helper unit-test output, the six M046 case
results, the CLI policy test, the layering grep (`fetch/browser/escalation.rs` imports), and an
optional live 429/500/403 table if the operator runs it.
