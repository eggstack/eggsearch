# M008 Mojeek default qualification — held-out trial summary

Decision: **not qualified; keep the shipped defaults unchanged.** The predeclared admission gate fails on Mojeek operational reliability, comparable-control coverage, and held-out contribution. The trial is an egress-specific observation, not a general claim about Mojeek quality.

## Provenance and protocol

- Source SHA: `7ec251df2ba715705468f2eb1e5392fa87fc4d32`
- Release CLI binary SHA-256: `14370900e193c56273c46409b2736ff7ac795a1a4c96ddf16510c870959d56fe` (release, `--no-default-features`; build succeeded)
- Runner SHA-256: `2bb24640027801e9c29f2839e1f8e84b8007a66a3ceb4cb7d489e8f4c72767d3`
- Temporary config SHA-256: `6483e7b6beb1503f4032d3c6c95745c6cf6af2b0a9bc366ab4fe580243ca6174`; enabled only DuckDuckGo, Brave, Yahoo, and Mojeek for isolated explicit-provider trials, while retaining the existing three-provider default list. No credentials or paid providers were used.
- Host: `macOS-26.6.2-arm64-arm-64bit-Mach-O`
- Fixed seed: `m008-mojeek-default-qualification-v1`; 3 queries × 2 repetitions × 10 results × 18-second per-attempt bound, for each of 4 providers on each of 3 datasets. Total: 72 attempts, no retries.
- Dataset fingerprints: SciFact `40f78fb516f4a69bfc4fc13b5b4f1d7ee784638d937ccf338c6b6436934613d2`; NQ `76f3240642b95adaccc4a0e6cb7ef75776ec105e6763585f37ade994663673ed`; FEVER `4f359df04ad5a9a29c2f78009e7229d2a1c8eb0bf42d953853811373dea13305`.
- Selected query IDs: SciFact `775`, `100`, `619`; NQ `nq:test2352`, `nq:test1742`, `nq:test2376`; FEVER `fever:test31743`, `fever:test120259`, `fever:test194565`.

## Outcomes

Each cell is six attempts; result/error classes are retained separately. The runner stores qrel matches and response hashes, not raw search-result bodies.

| Provider | SciFact | NQ | FEVER | Valid outcomes / 18 | Distinct matched qrel pairs |
|---|---|---|---|---:|---:|
| `duckduckgo` | 6 results | 4 results, 2 blocked | 6 blocked | 10 | 1 (NQ `nq:test1742` / `doc61640`) |
| `brave` | 6 results | 2 results, 4 blocked | 6 blocked | 8 | 1 (same NQ pair) |
| `yahoo` | 6 HTTP-status errors | 6 HTTP-status errors | 6 HTTP-status errors | 0 | 0 |
| `mojeek` | 5 results, 1 HTTP-status error | 6 HTTP-status errors | 6 HTTP-status errors | 5 | 0 |

Mojeek's 5 valid rows were all from SciFact. Across all 18 Mojeek attempts: 5 results, 13 provider errors, no successful empty responses, no timeouts, and no qrel matches. Its CLI-process latency range was 393–1,980 ms (median 438 ms); this is not time-to-first-useful-text and cache state was uncontrolled. The errors were classified as `http_status` by the runner.

DuckDuckGo and Brave each matched the same single NQ positive qrel pair on one valid trial; those matches do not establish broad retrieval quality. Yahoo had no valid rows. Because all three current defaults did not have sufficient valid paired coverage, errors cannot be treated as empty result sets and Mojeek novelty versus the complete default set cannot be established.

## Gate verdict

1. Trial integrity: **pass** — all 12 preserved reports share the source, binary, runner, config, seed, settings, and corresponding query IDs.
2. Mojeek operations: **fail** — 5/18 valid versus the 17/18 floor; per dataset, 5/6 SciFact, 0/6 NQ, 0/6 FEVER, with non-empty results only in SciFact.
3. Comparable controls: **fail** — DuckDuckGo is 4/6 on NQ and 0/6 on FEVER; Brave is 2/6 on NQ and 0/6 on FEVER; Yahoo is 0/6 on every dataset.
4. Held-out contribution: **fail** — Mojeek matched 0 distinct query/document pairs, below the 3-pair floor, and has no unique paired match.
5. Reporting: **complete** — all 72 attempts, outcomes, qrel denominators/matches, latencies, fingerprints, and limitations are in the 12 raw runner reports.

No provider-default code, tests, or user-facing default docs were changed. The default-fan-out live smoke and full implementation gates were skipped because M008 did not pass its admission gate. The prior three-query Mojeek functional smoke is not a substitute for this failed held-out qualification.

Raw reports: `plans/closure/retrieval-quality-evidence-extension/evidence/mojeek-*-m008.json`. Closure record: `plans/closure/retrieval-quality-evidence-extension/008-status.md`.
