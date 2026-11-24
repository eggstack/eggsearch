# M010 — Browser-Enabled Mojeek Default-Qualification Rerun Status

Status: blocked
Recommendation: do not add Mojeek to the shipped generic defaults.
Protocol: `m010-paced-v1` (25–40 s per-attempt pacing, stop after 3 consecutive blocked)
Source SHA: `7ec251df2ba715705468f2eb1e5392fa87fc4d32` — **working tree dirty** (see Limitations)
Release CLI binary SHA-256: `5c8c65fe099f7370bcfc04fd85cb6ba717eed7c4d83dfa0fa9ae1d4d27bcc65f` (default features: `browser`, `pdf`)
Runner SHA-256: `d70d086974e73fd1209f98fc2eecb25c9671366a75670b96b8851ad146e2082f`
Qualification config SHA-256: `01e08d77178bf091707ece340bc1ac27614bfa92a0f2b9999547c653c0093530`
Host: `macOS-26.6.2-arm64-arm-64bit-Mach-O`
Window: 2026-11-23 22:17:34Z – 22:49:21Z

## Evidence

Twelve paced arms — DuckDuckGo, Brave, Yahoo, and Mojeek × SciFact, NQ, and FEVER × the same three
held-out queries and two repetitions (seed `m008-mojeek-default-qualification-v1`), ten results,
eighteen-second ceiling, 72 attempts, no retries. Reports:
`evidence/mojeek-<provider>-<dataset>-m010.json`.

| Provider | SciFact | NQ | FEVER | Valid / 18 | Distinct qrel pairs | hit@10 | micro recall@10 | latency p50 |
|---|---|---|---:|---:|---:|---:|---:|---:|
| DuckDuckGo | 6/6 | 6/6 | 6/6 | 18 | 5 | 9/18 | 9/20 | 1,002 ms |
| Brave | 6/6 | 6/6 | 6/6 | 18 | 4 | 8/18 | 8/20 | 838 ms |
| Yahoo | 6/6 | 6/6 | 6/6 | 18 | 4 | 8/18 | 8/20 | 6,028 ms |
| Mojeek | 6/6 | 6/6 | 6/6 | 18 | **1** | 2/18 | 2/20 | 1,723 ms |

Every arm returned non-empty results for all attempts. No arm aborted; the stop rule never fired.
Pacing was honoured: minimum observed interval 25.478 s, zero declared violations, all rows carry
`planned_interval_seconds`, `actual_interval_seconds`, `started_at_utc`, and `http_status`.

Yahoo's p50 of 6.0 s (max 7.6 s) is the browser-render cost: it answered `500` over HTTP in M009 and
is served only by SERP escalation, which this default-features candidate provides.

## Acceptance matrix

| Requirement | Result | Evidence |
|---|---|---|
| 1. Trial integrity: same source, binary, runner, protocol, seed, per-dataset query IDs, limit, timeout, pacing parameters; all attempts represented | Pass on report content | Twelve reports agree; 72 attempts; one config fingerprint; no credential or paid provider |
| 2. Mojeek ≥17/18 valid; ≥5/6 valid and ≥4/6 non-empty per dataset | **Pass** | 18/18 valid, 18/18 non-empty, 6/6 everywhere |
| 3. Each default ≥4/6 valid per dataset; paired uniqueness | **Fail** on uniqueness | All three controls are 18/18 and all 18 Mojeek trials have three valid controls, so pairing is established; Mojeek's single matched pair is not unique — the controls also matched it |
| 4. ≥3 distinct Mojeek pairs over ≥2 datasets, ≥1 unique | **Fail** | 1 pair (`fever:test194565` → `Kyle_MacLachlan`), one dataset family, 0 unique |
| 5. Per-arm counts, matches, hit@10, recall, latency, limitations reported | Pass | Above, and the reports |

Defaults remain `[duckduckgo, brave, yahoo]`; Mojeek remains disabled. No production configuration,
test, or user-facing default documentation changed.

## Findings

1. **The M009 control failure was the candidate class, as suspected.** With `browser` included,
   Yahoo went from 0/18 to 18/18 and the paired-coverage test became meaningful for the first time.
2. **Mojeek is operationally sound and contributes little on this sample.** 18/18 valid at 1.7 s p50
   — faster than the browser-rendered default — but one distinct qrel pair, 2/18 hit@10, and no
   unique match. The failure is now a relevance result, not a mechanics result.
3. **The gate is intact.** Condition 4 is a minimum evidence floor that a supplementary provider with
   this recall does not clear; nothing was re-tuned after seeing the numbers.
4. **A side observation, not acted on:** the shipped default Yahoo now needs browser rendering on this
   egress and costs ~6 s per query. That is a default-set question of its own and is out of scope
   here.

## Limitations

- The working tree was dirty at build time: the release binary carries the uncommitted M049
  classification change and the M009 runner protocol change. The reported source SHA names the
  commit, not the exact built tree, so this is not an exact-candidate qualification.
- Browser rendering is a candidate property: results are conditional on `[fetch.browser].enabled` and
  a discovered Chrome. A browser-free deployment sees M009's Yahoo behaviour.
- Three queries per dataset and two repetitions is a small sample; the runner scores only URL or
  normalized-title matches against positive qrels, so unjudged results are not irrelevant.
- One egress, one run, one host; cache state is uncontrolled. M010 ran ~30 minutes after M009, a
  declared deviation from M009's 24 h cooldown, with identical bounded pressure.

## Shipping verification (2026-11-23)

The shipped configuration was checked rather than assumed.

- `Cargo.toml` declares `default = ["pdf", "browser"]`, and `src/core/config.rs` resolves
  `fetch.browser.enabled` from `cfg!(feature = "browser")`, so browser rendering is enabled by
  default in any default-feature build with no configuration at all.
- Verified live: with a config containing **no** `[fetch.browser]` section, the default-feature
  release binary answered a Yahoo search with three results and no provider failures. Yahoo answers
  `500` over plain HTTP on this egress, so that response can only come through the browser path.
- The generated release workflow builds every target with
  `cargo build --release --locked --target <T> --package eggsearch --bin eggsearch` and never passes
  `--no-default-features`, so prebuilt binaries ship the browser.
- The single artifact without browser rendering is the Python wheel:
  `bindings/python/Cargo.toml` sets `default-features = false, features = ["pdf"]`, which ADR-0014 §7
  and `docs/python-library.md` document deliberately — a wheel still needs a system Chrome, and a
  wheel-local CDP client adds size and a second HTTP client at the Python process boundary. Changing
  that is an ADR-level decision, not a default flip.
- `tests/static_guards.rs` gains `default_features_ship_pdf_and_browser`, which fails closed if the
  default feature set loses `pdf` or `browser`, if `egress` enters it, if the browser config
  default stops tracking the feature, or if a release build command starts passing
  `--no-default-features`. Full suite: 69 guards pass.

## Standing decision

Mojeek remains disabled by default. Three gate attempts failed on three different grounds —
reachability (M008), candidate class (M009), and relevance (M010) — and the last is the substantive
one: Mojeek is reliable and fast but contributed one non-unique held-out pair. Nothing here reopens
the gate, and no further attempt is planned without a new registered milestone.

## Checks

- `python3 -m unittest scripts/test_retrieval_quality_runner.py`: 9 tests pass.
- `./packaging/check-repo-hygiene.sh`: `repo-hygiene: ok`.
- `./packaging/check-contract.sh`: `planning-consistency: ok`, `packaged-assets: ok`.
- `git diff --check`: clean.

## Next

Mojeek is reliable and cheap but contributes one non-unique held-out pair on this sample. Raising its
standing would need either a larger held-out sample that shows a contribution this one does not, or a
deliberate gate revision — neither is assumed here. No further rerun occurs within 24 h.
