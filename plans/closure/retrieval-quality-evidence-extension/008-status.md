# M008 — Mojeek Generic-Default Qualification Status

Status: blocked
Recommendation: do not add Mojeek to the shipped generic defaults on this candidate.
Candidate source SHA: `7ec251df2ba715705468f2eb1e5392fa87fc4d32`
Release CLI binary SHA-256: `14370900e193c56273c46409b2736ff7ac795a1a4c96ddf16510c870959d56fe`
Runner SHA-256: `2bb24640027801e9c29f2839e1f8e84b8007a66a3ceb4cb7d489e8f4c72767d3`
Qualification config SHA-256: `6483e7b6beb1503f4032d3c6c95745c6cf6af2b0a9bc366ab4fe580243ca6174`
Host: `macOS-26.6.2-arm64-arm-64bit-Mach-O`

## Evidence

The frozen runner executed 72 bounded, keyless attempts: DuckDuckGo, Brave, Yahoo, and Mojeek × SciFact, NQ, and FEVER × three selected held-out queries × two repetitions. The exact seed was `m008-mojeek-default-qualification-v1`; each attempt requested ten results with an eighteen-second ceiling. There were no retries. All 12 JSON reports were recovered from the captured runner outputs and saved under `plans/closure/retrieval-quality-evidence-extension/evidence/`.

| Provider | SciFact | NQ | FEVER | Valid / 18 | Distinct qrel pairs |
|---|---|---|---|---:|---:|
| DuckDuckGo | 6 results | 4 results, 2 blocked | 6 blocked | 10 | 1 |
| Brave | 6 results | 2 results, 4 blocked | 6 blocked | 8 | 1 |
| Yahoo | 6 HTTP-status errors | 6 HTTP-status errors | 6 HTTP-status errors | 0 | 0 |
| Mojeek | 5 results, 1 HTTP-status error | 6 HTTP-status errors | 6 HTTP-status errors | 5 | 0 |

The only positive match from DuckDuckGo and Brave was the same NQ qrel pair, `nq:test1742` / `doc61640`, on one repetition. Mojeek matched no qrel document. Mojeek's five valid results all came from SciFact; the 13 other attempts were provider errors classified `http_status`. Its CLI-process latency was 393–1,980 ms, median 438 ms. No successful empty responses or timeouts occurred for Mojeek.

Dataset fingerprints were SciFact `40f78fb516f4a69bfc4fc13b5b4f1d7ee784638d937ccf338c6b6436934613d2`, NQ `76f3240642b95adaccc4a0e6cb7ef75776ec105e6763585f37ade994663673ed`, FEVER `4f359df04ad5a9a29c2f78009e7229d2a1c8eb0bf42d953853811373dea13305`. Selected IDs: SciFact `775`, `100`, `619`; NQ `nq:test2352`, `nq:test1742`, `nq:test2376`; FEVER `fever:test31743`, `fever:test120259`, `fever:test194565`.

## Acceptance matrix

| Requirement | Result | Evidence |
|---|---|---|
| Exact candidate, binary, runner, config, seed, query IDs, and all 72 outcomes recorded | Pass | Twelve per-arm JSON reports; provenance above |
| Mojeek ≥17/18 valid; ≥5/6 valid and ≥4/6 non-empty per dataset | Fail | Mojeek 5/18 valid; dataset counts 5/6, 0/6, 0/6; non-empty only 5/6 SciFact |
| Each existing default ≥4/6 valid in each dataset | Fail | DuckDuckGo 4/6 NQ and 0/6 FEVER; Brave 2/6 NQ and 0/6 FEVER; Yahoo 0/6 throughout |
| ≥3 distinct Mojeek qrel pairs over ≥2 datasets and ≥1 unique fully paired match | Fail | 0 Mojeek matches; no fully comparable default coverage |
| Preserve defaults if any gate condition fails | Pass | No production config, tests, or default docs changed |

The sample uses positive qrels and URL/normalized-title matching only; unlisted results are unjudged, not irrelevant. The errors are current egress observations and do not prove Mojeek is intrinsically unavailable. The runner does not retain raw result bodies and cannot distinguish source quality from the observed blocked/HTTP-status conditions.

## Changes and checks

No production provider defaults, tests, or provider setup documentation were changed. The live default-fan-out smoke and implementation gates were not run because qualification failed. The temporary config retained the shipped default list and enabled the four keyless HTML providers only for explicit isolated trials; no credential or paid-provider configuration was used. The previous three-query functional smoke remains functional evidence only and does not override this held-out gate.

Blocker: Mojeek did not return valid result responses outside the SciFact arm, and existing default controls were too incomplete for a paired qualification. M008 remains blocked; a future qualification would require a separately authorized, newly bounded run after endpoint/control availability changes. The defaults remain `[duckduckgo, brave, yahoo]`; Mojeek remains disabled.

## Correction (2026-11-23)

The paragraph above and the acceptance matrix are left as recorded and corrected here rather than
rewritten.

The thirteen Mojeek attempts recorded above as `http_status` cannot be re-inspected: the runner
retained error classes and no response bodies. What this egress did capture is a **hard refusal
page**, not a transport fault. The qualification-time diagnostic reported
`engine 'mojeek' returned status 403`, and one bounded keyless capture on 2026-11-23 returned
`HTTP/1.1 403 Forbidden`, `Content-Length: 363`, `Server: lighttpd/1.4.53`, and this body:

```html
<html><head><title>403 - Forbidden</title></head><body><h1>403 - Forbidden</h1><h2>Sorry your network appears to be sending automated queries so we can't process your search at this time.</h2>…</body></html>
```

The body carries the marker `mojeek.rs` already declares in `SHAPE.blocked_markers`
(`your network appears to be sending automated queries`), but `send_search()` returned
`BadStatus` before reading the body, so the marker never ran. Under the rule in
`architecture/engines.md` § Scraper Fidelity and M045 work package 1, those thirteen observations
are `blocked`, not `http_status`. M049 corrects the classification.

This changes the label, not the gate result. `blocked` is a non-retryable provider refusal with a
cooldown, which is the honest classification for these responses.

- `https://www.mojeek.com/robots.txt` declares `User-agent: *` / `Disallow: /search`: the HTML
  search page is not offered to automated clients.
- Mojeek gates `/search` with an ALTCHA proof-of-work wall served with `HTTP 200` `div.captcha-wrap`,
  added 2026-06-23.
- Sustained automated traffic escalates to a hard `403` carrying that page. Mojeek's forum documents
  the same page for datacenter and VPN egress and, separately, for single over-broad bot rules that
  fire on the query text.
- The refusal is **query-keyed, not network-wide and not user-agent-wide**. On 2026-11-23 this
  egress (public address `96.236.59.113`, verified identical for the in-app browser and the CLI)
  served Mojeek's homepage with `200`; served `/search` with the solvable `200` ALTCHA wall for
  `test`, `rust`, `official website`, `rust programming language`, a long benign sentence, and the
  held-out question `nq:test2352` (including a reload and a repeat); and the shipped client returned
  five real results for that question with no provider failures. The captured `403` reproduced only
  for the probe string `Rust programming language official website`, from the in-app Chromium engine,
  from curl with a stale Windows Chrome/124 identity, and from curl with a current macOS Chrome/148
  identity plus `sec-ch-ua`/`sec-fetch-*`; changing the user agent and client hints changed nothing.
- An earlier version of this note called the refusal network-scoped. That scope is withdrawn. The
  controls are retained in
  `plans/closure/retrieval-quality-evidence-extension/evidence/mojeek-403-mechanism-controls.json`.
- The supported automated route is `https://api.mojeek.com/search` with `api_key` (PAYG, from
  £2 per 1,000 requests); their terms allow automated access only by an authorised API user.

A related deviation: the M008 candidate was built with `--no-default-features`, which is **not** the
shipped feature set. The shipped CLI builds with default features (`default = ["pdf", "browser"]`),
and `[fetch.browser].enabled` resolves from `cfg!(feature = "browser")`, so browser rendering is on by
default unless an operator disables it. Building the qualification candidate without the browser
removed Yahoo's escalation path and made condition 3 unsatisfiable regardless of Mojeek. M010
corrects the candidate class; see its record.

The M008 gate and the default set are unchanged: `[duckduckgo, brave, yahoo]`, Mojeek disabled. The
held-out gate failed on the day and is not retroactively passed. What changes is the disposition of
the failure: the thirteen refusals are query-keyed `403`s, and the cohort's own 72 rapid attempts are
consistent with the documented escalation path, so the run may have measured its own request
pressure. A paced, bounded rerun is therefore worth authorising; it is not authorised here and was
not run. Mojeek's `robots.txt` still declares `Disallow: /search`, so such a rerun remains an
operator decision rather than a policy-neutral one. Both captures are retained in
`plans/closure/retrieval-quality-evidence-extension/evidence/`.
