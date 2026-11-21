# M044 Challenge Solving On by Default — Closure Evidence

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/044-challenge-solving-on-by-default.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md` M044

Corrective to: M035 (`plans/closure/agentic-retrieval-integrity/035-status.md`) and
`plans/adrs/ADR-0019-broadened-challenge-coverage-and-browser-identity.md` §4.

Decision authority: accepted ADR-0019 §4, "Solving is on by default, bounded". No new ADR, tool,
argument, schema property, configuration key, or dependency.

## Exact code candidate

Implementation candidate `7b28436ce913bbed52bd7fdc3ba5cf14f9c85f70` (parent `54a0520`), on
`feat/auto-20261117-044c4885`. Every command, probe, and hosted job below ran against that tree.
This closure commit carries this record, the plan status, the roadmap status, and the registry
rows together.

## Gaps found

1. ADR-0019 §4 states "`[search.challenge].enabled` defaults to true". `ChallengeSection::default()`
   returned `enabled: false`, so M035 shipped the capability without the default the ADR accepted,
   while the registry row described the flip as done.
2. Even with `enabled = true`, the shipped ceilings (`max_difficulty 5`, `max_attempts 2_000_000`,
   `timeout_ms 2000`) could not solve either supported wall: Startpage publishes Anubis difficulty
   6, refused by the ceiling before hashing, and Mojeek's ALTCHA solve has no room in 2 s.
   Default-on at those ceilings would have changed nothing.
3. `architecture/engines.md` records a measured difficulty-6 solve at **23.07 M candidates**. An
   attempts ceiling at 20 M would have refused roughly a third of solvable walls, so the ceiling had
   to be set where the wall clock, not the counter, is the binding constraint.

## Change

1. `ChallengeSection::default()` and the serde field default are now `enabled: true`,
   `max_difficulty: 6`, `max_attempts: 100_000_000`, `timeout_ms: 10_000`,
   `cookie_ttl_seconds: 240`. `10_000` remains below the documented difficulty-6 requirement of
   `8000` ms plus measured headroom, and the caller's own deadline still caps every solve.
   `100_000_000` sits above what the wall clock can spend at measured release speed, so the clock
   governs cost while a projected difficulty 7 (`16^7`) is still refused.
2. `enabled = false` keeps the previous behaviour exactly, and an explicit test pins that.
3. Tests that pinned the old default now assert the new one, and `provider_status` continues to
   report `challenge_solver.enabled` from the effective configuration.
4. Documentation that described the solver as opt-in is updated: `docs/config.md`,
   `docs/provider-setup.md`, `docs/features.md`, `architecture/config.md`,
   `architecture/engines.md`, and `CHANGELOG.md`.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | An empty config enables the solver with the documented ceilings | `core::config::tests::challenge_defaults_are_enabled_and_bounded` and `config_without_challenge_table_enables_the_solver`; `web_search_integration::challenge_config_is_enabled_by_default`; `provider_routing::challenge_solver_block_reports_the_shared_ledger` reads `challenge_solver.enabled = true`. |
| 2 | `enabled = false` disables it | `core::config::tests::challenge_can_still_be_disabled_explicitly` parses a serialized config with the flag set and asserts `false`. |
| 3 | Mojeek and Startpage return results on a default config | Release build, config with **no** `[search.challenge]` table: Mojeek 5 results in 1.6 s then 0.3 s; Startpage 5 results in 2.6 s then 0.4 s (pass-cookie reuse); DuckDuckGo unaffected at 0.8 s / 0.6 s. |
| 4 | The first solve fits the stock deadline | With the shipped 8 s search deadline: Startpage 3.9 s then 0.4 s and 0.4 s over three calls. |
| 5 | The fetch path solves by default and still refuses interactive challenges | `fetch` of `https://www.startpage.com/sp/search?query=rust` returned status 200 in 7.2 s with `browser_escalated: false`; `https://nowsecure.nl/` returned the interactive-challenge refusal. |
| 6 | Budget matches the measured wall | `architecture/engines.md` records difficulty 6 at 23.07 M candidates / 3.05 s; the live default build solved in 2.6–7.6 s across runs, inside the 10 s ceiling and the documented 8 s requirement. |
| 7 | The prohibited classes are unchanged | No change to the solver's method set, the interactive-CAPTCHA prohibition, the one-solve-per-provider rule, the deadline cap, or pass-cookie handling; `static_guards` and the challenge suites pass. |
| 8 | Gates pass on the exact candidate | `make check` exit 0 (6 554 passed, 0 failed, 28 ignored); `make docs-check` exit 0; `packaged-assets` guard ok. |

## Live before and after

Release build, one server process per configuration, five results requested:

| Provider | Solver off (previous default) | No `[search.challenge]` table (new default) |
|---|---|---|
| `duckduckgo` | 5 results, 0.8 s | 5 results, 0.8 s / 0.6 s |
| `mojeek` | `blocked: challenge page detected (div.captcha-wrap)` | **5 results, 1.6 s / 0.3 s** |
| `startpage` | `blocked: challenge page detected (#anubis_challenge)` | **5 results, 2.6 s / 0.4 s** |
| `brave` | HTTP 429 | HTTP 429 (upstream, see below) |
| `yahoo` | HTTP 500 | HTTP 500 (upstream, see below) |

A debug build is roughly 4–5× slower: the same Difficulty-6 wall exhausted a 30 s debug ceiling
while the release build solved it in 2.6–7.6 s. This is a developer-facing caveat, not a shipped
configuration: the release path is the qualified one.

## Brave 429 and Yahoo 500 (recorded, out of scope)

Both are upstream refusals of this egress, not request-shape defects, and both are classified
rather than swallowed:

- **Brave**: `429` on every attempt across sessions hours apart, with no `Retry-After` header;
  plain `curl` with a browser user agent also receives `429` with an 80 KiB body. The engine
  reports `rate_limited` and the health registry cools the provider down for 60 s after three
  consecutive failures. What resolves it: a `BRAVE_API_KEY` with the shipped `brave_api` provider,
  or provider traffic through a different exit address (`[egress]`, which is build-feature gated
  and absent from prebuilt binaries), or removing Brave from `default_providers`.
- **Yahoo**: `search.yahoo.com/` serves 200 (263 KiB) but `/search?p=…` returns `500` with zero
  bytes for `p` and `q`, with and without `fr=`, with a browser user agent, `Accept`/`Accept-Language`,
  compression, a cookie jar seeded from the host, and a referer; `nojs=1` and the cookie-bootstrap
  attempt fail at the transport layer (`000`). The engine reports `http_status`. No client-side
  parameter or cookie recovers it, so the options are a different exit address or an API provider.
- Both providers are in the shipped `default_providers`. DuckDuckGo carries the default set here.
  With Mojeek and Startpage now working out of the box, rebalancing that set is a separate product
  decision; this milestone deliberately does not change it.

**Reconciled 2026-11-21:** the paragraph above was wrong to describe these failures as egress
refusals. In-app browser verification on the same egress shows both origins serving results to a
real browser: Brave serves a proof-of-work captcha (its HTTP 429 body **is** the 80 KiB challenge
document, a script-rendered shell whose text lives in the embedded i18n dictionary) and returns
results after the browser completes it, reusing the pass cookie on the next search; Yahoo renders a
complete results page with no visible challenge. The defect is our client's detection and challenge
handling, not the address. Brave's classification is corrected under
`plans/implementation/agentic-retrieval-integrity/045-browser-fidelity-provider-access.md`
work package 1 (a non-success challenge page is now `blocked`, not `rate_limited`); work package 2
of that milestone owns browser-rendered SERP access, which is what actually produces results.

## Hosted qualification

On candidate `7b28436`, all on attempt 1:

- `CI` run `44859709721`: `ci`, `windows portability`, and `macOS canonical verification and local
  filesystem portability` all success.
- `Python native qualification` run `44859709648`: success, including the extracted-sdist rebuild.
- `Egress feature qualification` run `44859709752`: success.
- `Release drift guard` run `44859709669`: success.

## Residual risk

1. **Default cost.** A challenged provider can spend up to the solver ceiling (10 s) or the caller's
   deadline, whichever is smaller, on the first request per reuse window. The pass cookie then makes
   later requests fast (0.3–0.4 s measured). An operator who prefers the old behaviour sets
   `enabled = false`.
2. **First solve versus the shipped deadline.** Startpage's difficulty-6 solve measured 2.6–7.6 s
   against the default 8 s search deadline. On a slow host the first call can be cut, and the
   provider then reports `blocked`; raise `[search].timeout_ms` if that matters.
3. **Upstream difficulty drift.** If a supported upstream raises its published difficulty above 6,
   the default refuses it by ceiling rather than overspending. That is the intended failure mode and
   is reported as `blocked` with the bound named.
4. **Debug builds.** Challenge solving is impractical under `target/debug`; qualify with a release
   build.
5. **Brave and Yahoo** remain unusable from this egress; see above. Nothing in this milestone
   changes their classification or the default set.

## Not done

No change to the permitted challenge class, the interactive-CAPTCHA prohibition, the default
provider set, or the request deadline rule. No new provider, tool, argument, schema property, or
dependency.
