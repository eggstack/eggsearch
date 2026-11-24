# M009 — Paced Mojeek Default-Qualification Rerun Status

Status: blocked
Recommendation: do not add Mojeek to the shipped generic defaults.
Protocol: `m009-paced-v1` (25–40 s per-attempt pacing, stop after 3 consecutive blocked, 24 h cooldown)
Source SHA: `7ec251df2ba715705468f2eb1e5392fa87fc4d32` — **working tree dirty** (see Limitations)
Release CLI binary SHA-256: `fd90457efcb52aa69acccd306c9af2f4e335dc54259a447df5479bcf66e1daf2`
Runner SHA-256: `d70d086974e73fd1209f98fc2eecb25c9671366a75670b96b8851ad146e2082f`
Qualification config SHA-256: `6483e7b6beb1503f4032d3c6c95745c6cf6af2b0a9bc366ab4fe580243ca6174`
Host: `macOS-26.6.2-arm64-arm-64bit-Mach-O`
Window: 2026-11-23 21:16:13Z – 21:47:03Z

## Evidence

The frozen runner executed twelve paced arms — DuckDuckGo, Brave, Yahoo, and Mojeek × SciFact, NQ,
and FEVER × the same three held-out queries and two repetitions M008 used (seed
`m008-mojeek-default-qualification-v1`), ten results per query, eighteen-second ceiling, 72 attempts,
no retries. Reports: `evidence/mojeek-<provider>-<dataset>-m009.json`.

| Provider | SciFact | NQ | FEVER | Valid / 18 | Distinct qrel pairs |
|---|---|---|---:|---:|---:|
| DuckDuckGo | 6/6 | 6/6 | 6/6 | 18 | 4 |
| Brave | 6/6 | 6/6 | 6/6 | 18 | 4 |
| Yahoo | 0/6 | 0/6 | 0/6 | 0 | 0 |
| Mojeek | 6/6 | 6/6 | 6/6 | 18 | 1 |

Mojeek returned non-empty results for every attempt, with no `blocked` outcome, no `zero_results`,
no timeout, and no provider error. Its latency was 1,454 / 1,823 / 2,700 ms (min / p50 / max); the
ALTCHA solve is included. Its single matched pair is `fever:test194565` → `Kyle_MacLachlan`, at rank 1
on both repetitions.

Yahoo answered `500` to all eighteen attempts (`engine 'yahoo' returned status 500`, p50 1,083 ms).

Pacing was honoured: minimum observed interval 25.228 s across all 72 attempts, zero declared
violations, and every row carries `planned_interval_seconds`, `actual_interval_seconds`,
`started_at_utc`, and `http_status`.

## Acceptance matrix

| Requirement | Result | Evidence |
|---|---|---|
| 1. Trial integrity: same source, binary, runner, protocol, seed, query IDs per dataset, result limit, timeout, pacing parameters; all attempts represented | Pass on report content | Twelve reports agree per dataset; 72 attempts; config fingerprint identical; no paid provider or credential |
| 2. Mojeek ≥17/18 valid; ≥5/6 valid and ≥4/6 non-empty per dataset | **Pass** | 18/18 valid, 18/18 non-empty, 6/6 in each dataset |
| 3. Each existing default ≥4/6 valid per dataset, and paired uniqueness | **Fail** | Yahoo 0/6 in every dataset; 0/18 Mojeek trials have all three controls valid, so no paired or unique match can be established |
| 4. ≥3 distinct Mojeek pairs over ≥2 datasets, ≥1 unique | **Fail** | 1 pair (`fever`), 0 unique |
| 5. Report per-arm counts, matches, hit@10, recall, latency, limitations | Pass | Above, and the reports themselves |

Defaults remain `[duckduckgo, brave, yahoo]`; Mojeek remains disabled. No production configuration,
test, or user-facing default documentation was changed.

## Findings

1. **The refusal was protocol-induced.** Under pacing, Mojeek refused nothing: 18/18 valid against
   M008's 5/18. That confirms the M009 premise — the earlier `403`s tracked request pressure and a
   query-keyed rule, not a blocked network or a client identity.
2. **Held-out contribution is weak on this sample.** One distinct query/document pair against four
   each for DuckDuckGo and Brave, and no pair across a second dataset family. This is a minimum
   evidence floor that was not met, not a claim that Mojeek lacks value.
3. **Condition 3 is unsatisfiable in this candidate class.** Yahoo is a shipped default that answers
   `500` to every HTML request and is served only by the browser escalation path, which
   `--no-default-features` removes. A browser-free keyless candidate therefore cannot produce the
   paired control coverage the gate requires — the same structural reason M008 could not pass. The
   gate, not Mojeek, is the binding constraint here.

## Limitations

- The working tree was dirty at build time: the release binary carries the uncommitted M049
  classification change (`src/meta/engines/mojeek.rs`) and the M009 runner protocol change. The
  reported source SHA identifies the commit, not the exact built tree, so this is not an
  exact-candidate qualification. The classification change affects labelling only, and Mojeek
  produced no refusal to label.
- Three queries per dataset and two repetitions is a small sample; the runner scores only URL or
  normalized-title matches against positive qrels, so unjudged results are not irrelevant.
- One egress, one run, one host; cache state is uncontrolled.
- Pacing bounds are an engineering choice, not a measured optimum. A slower or faster protocol could
  change refusal behaviour.

## Checks

- `python3 -m unittest scripts/test_retrieval_quality_runner.py`: 9 tests pass (schedule determinism,
  interval bounds, status extraction from both message shapes, blocked detection, abort rule, legacy
  unpaced behaviour).
- `./packaging/check-contract.sh`: `planning-consistency: ok`, `packaged-assets: ok`.
- `git diff --check`: clean.

## Next

Condition 3 cannot pass without either a browser-enabled candidate, in which Yahoo and Brave can be
served, or a predeclared control set that excludes providers structurally unable to answer in the
tested configuration. Either change is a gate-design decision and needs its own registered milestone;
this record does not assume one.
