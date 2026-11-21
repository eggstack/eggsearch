# M044 — Challenge Solving On by Default

Status: ready
Date: 2026-11-21
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective to M035 and to ADR-0019 §4)
Baseline: `54a05202508126817e8a48a965ef75500ad78f93` on `feat/auto-20261117-044c4885`
Decision authority: accepted ADR-0019 §4 ("Solving is on by default, bounded"). No new ADR.

References: `plans/000-long-term-specification.md` §2; `plans/003-planning-process.md` §7;
`plans/adrs/ADR-0019-broadened-challenge-coverage-and-browser-identity.md` §4 and §7;
`plans/implementation/agentic-retrieval-integrity/035-browser-challenge-completion.md`;
`plans/closure/agentic-retrieval-integrity/035-status.md`; `docs/features.md` § Bounded Challenge
Solving; `docs/config.md` § Challenge Solving.

## Objective

Make the accepted ADR-0019 default real: computational challenge solving runs without operator
opt-in, and its shipped budget is large enough to solve the walls the supported upstreams actually
serve, so a challenged provider returns results instead of `blocked` out of the box.

## Evidence

Observed on 2026-11-20/21 with the release build at `54a0520`, real network:

1. ADR-0019 §4 states "`[search.challenge].enabled` defaults to true". `ChallengeSection::default()`
   still returned `enabled: false` with `max_difficulty: 5`, `max_attempts: 2_000_000`, and
   `timeout_ms: 2000`. M035 shipped the capability without the default flip, and the registry row
   for M035 claims the flip happened.
2. With the solver enabled at a 30 s ceiling, `startpage` (published Anubis difficulty 6) solved and
   replayed in **5.9 s**, and the next query reused the pass cookie in **0.4 s**. `mojeek` (ALTCHA)
   solved in **1.9 s**, then **0.6 s**. Both returned five results.
3. With the solver enabled at the shipped ceilings (`max_difficulty 5`, `timeout_ms 2000`),
   `startpage` is refused by the ceiling before any hashing ("difficulty 6 exceeds configured
   ceiling 5") and `mojeek` has no room to finish. Default-on at the old ceilings would therefore
   change nothing for either provider.
4. Debug builds are roughly 4–5× slower: the same difficulty-6 wall exhausted a 30 s solver ceiling
   on `target/debug`, while the release build solved it in 5.9 s. The shipped default must be
   expressed in wall-clock terms the release path can meet.

## Non-goals and compatibility

- No new provider, tool, argument, schema property, or dependency.
- No change to the permitted challenge class, the prohibition on interactive CAPTCHA/OCR/third-party
  solving, the one-solve-per-provider rule, the request-deadline cap, or pass-cookie handling.
- The `[search.challenge]` table keeps every key; `enabled = false` restores the previous behaviour
  exactly, so an operator can opt out without other changes.
- No change to the default provider set. Brave and Yahoo failures are upstream and out of scope
  (see the closure record).

## Code pointers

- `src/core/config/challenge.rs:15` — `ChallengeSection` fields, their documented defaults, and
  `impl Default`.
- `src/core/config/tests.rs:1268` — `challenge_defaults_are_disabled_and_bounded` and
  `config_without_challenge_table_keeps_today_behaviour` pin the old default.
- `src/meta/engines/mod.rs:1188` — the browser-like default user agent; unchanged.
- `src/commands/doctor.rs:280` — the egress/cookie-reuse advisory already keys off
  `challenge.enabled`.
- `docs/config.md` § Challenge Solving, `docs/provider-setup.md` § Startpage, and
  `docs/features.md` § Bounded Challenge Solving describe the old default.

## Ordered work packages

1. **Default.** `ChallengeSection::default()` becomes `enabled: true`, `max_difficulty: 6`,
   `max_attempts: 20_000_000`, `timeout_ms: 10_000`, `cookie_ttl_seconds: 240`, with the field
   documentation and the `default_challenge_*` helpers updated to match.
2. **Budget justified by the wall, not by taste.** The ceiling must cover the difficulty that a
   supported upstream publishes today (6) with the measured release solve time (5.9 s) plus
   headroom, while the caller's deadline still caps every solve.
3. **Tests.** Replace the two default-pinning tests with default-on assertions, and add a test that
   an explicit `enabled = false` still disables the solver. Fix any engine test that inherited the
   old default while exercising the disabled path.
4. **Docs.** Update `docs/config.md` (prose, sample block, table), `docs/provider-setup.md`
   (keyless-provider intro and Startpage), `docs/features.md` Enabling section, and `CHANGELOG.md`.
5. **Live proof on the default config.** With no `[search.challenge]` table at all, the release
   build must solve `mojeek` and `startpage` and return results, and `duckduckgo` must be
   unaffected.

## Verification

```sh
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib challenge
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test web_fetch_integration
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
```

Plus a release-build live run with a config that contains no `[search.challenge]` table, recording
per-provider results, solve latency, and cookie reuse.

## Acceptance and stop conditions

- An empty config enables the solver with the documented ceilings, and `enabled = false` disables it.
- `mojeek` and `startpage` return results on a default config from the release build; solve and
  reuse latencies are recorded.
- The fetch path solves a published Anubis/ALTCHA wall without operator opt-in and still refuses
  interactive challenges.
- `make check` and `make docs-check` pass on the exact candidate.
- No change to the permitted challenge class, the prohibition list, the deadline cap, or the
  provider set.

**Stop:** enabling by default would require exceeding the request deadline for a current upstream, or
a supported upstream needs a challenge class the ADR prohibits. Record the measured cost and propose
a separate decision instead of widening this milestone.

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/044-status.md` with the ADR reference, the before/after
defaults, the live solve and reuse timings, the fetch-path evidence, the exact-candidate gates, the
hosted run id, and residual risk (default CPU cost, debug-build timing, upstreams that move to a
higher difficulty).
