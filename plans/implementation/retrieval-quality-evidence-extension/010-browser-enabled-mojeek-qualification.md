# M010 — Browser-Enabled Mojeek Default-Qualification Rerun

Status: blocked (executed 2026-11-23; see `plans/closure/retrieval-quality-evidence-extension/010-status.md`)
Date: 2026-11-23
Source: `plans/subsystems/retrieval-quality-evidence-extension-roadmap.md` M010
Primary class: capability (qualification; corrective to the M009 candidate class)
References: M008 plan and closure; M009 plan and closure; ADR-0006; ADR-0019 §5; `architecture/engines.md` § Browser-rendered access; `plans/003-planning-process.md`.

## Objective

Re-run the unchanged admission gate with a candidate that can actually serve every control provider, so
condition 3 tests paired control coverage instead of a structural limitation.

M009 failed condition 3 because Yahoo — a shipped default — answered `500` to all eighteen attempts,
and the `--no-default-features` candidate has no browser. `browser` is a **default** Cargo feature
(`default = ["pdf", "browser"]`), and `engines.md` installs SERP escalation when
`[fetch.browser].enabled` is true and a browser is discovered. M009 therefore measured the candidate
class rather than the gate.

## Candidate class

- Build: `cargo build --release --locked --bin eggsearch` (default features, browser included).
- Config: `target/m010-provider-qualification.toml`, identical to the M009 config plus
  `[fetch.browser] enabled = true`; shipped `default_providers` still retained.
- Pre-flight (2026-11-23T22:16Z): `search --providers yahoo` returned five real results with no
  provider failures in 6.2 s, so browser escalation is available and Google Chrome is installed.

## Protocol

`m010-paced-v1`, identical to `m009-paced-v1`: 25–40 s deterministic per-attempt pacing,
abort after three consecutive `blocked`, per-attempt `http_status` and interval capture, same frozen
query IDs (seed `m008-mojeek-default-qualification-v1`), ten results, eighteen-second ceiling,
72 attempts, no retries.

**Declared deviation.** M009's protocol set a 24 h cooldown before another full rerun from this
egress; M010 runs about 30 minutes later at the operator's direction. The cooldown existed to avoid
driving the host into an escalation; M009 recorded zero Mojeek refusals, and M010 keeps the same
bounded pressure (18 Mojeek attempts, paced). The deviation is recorded rather than hidden, and no
further rerun happens within 24 h of this one.

## Gate

Identical to M008/M009 conditions 1–5. Condition 3 is now expected to be satisfiable, which is the
entire point of the milestone.

## Acceptance

- Twelve reports whose recorded intervals honour the declared minimum and whose source, binary,
  runner, config, protocol, seed, and per-dataset query IDs agree.
- Defaults change only if all five conditions pass.
- A refusal, abort, or control shortfall is recorded; it is not re-pushed.

## Execution outcome

Executed 2026-11-23 under `m010-paced-v1`. All twelve arms returned 18/18 valid and 18/18 non-empty
outcomes with no refusals and no aborts, so paired coverage was established for the first time: every
Mojeek trial had three valid controls.

The gate still failed, now on relevance rather than mechanics. Mojeek produced one distinct qrel pair
(`fever:test194565` → `Kyle_MacLachlan`) across one dataset family with no unique match, against a
floor of three pairs across two families; DuckDuckGo produced five pairs, Brave and Yahoo four each.
Defaults are unchanged and Mojeek remains disabled. See
`plans/closure/retrieval-quality-evidence-extension/010-status.md`.
