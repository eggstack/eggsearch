# Captured HTML search-engine fixtures

Every HTML scraper in `src/meta/engines/` has three fixtures here:

| File | Contract exercised |
|---|---|
| `results.html` | Result container present with parseable rows |
| `empty.html` | No rows, but a documented empty-state marker present |
| `blocked.html` | No rows, but a challenge / consent / anti-bot marker present |

The conformance suite `tests/engine_parse_conformance.rs` is table-driven over
`brave`, `duckduckgo`, `mojeek`, `startpage`, and `yahoo`, and asserts all three
outcomes plus a seeded-drift case (the result container class renamed in
`results.html`, which must yield `Err(EngineError::ParseFailed)`).

## Provenance

Captures were attempted from this environment on 2026-11-10 UTC with a real query
(`rust programming language`) and a nonsense query (`zqxjvwkkplmnrtbqzz`); an
impossible `site:` query was added to provoke Brave's empty state. The `curl`
client used its default user agent; standard `Accept` / `Accept-Language` headers
were also tried where the first attempt was blocked. Where that client was
blocked, a second bounded client (`web_fetch`) was tried before falling back to
a synthetic reconstruction.

| Provider | Fixture | Provenance | Capture detail |
|---|---|---|---|
| `brave` | `results.html` | **Live-captured** | `GET https://search.brave.com/search?q=rust+programming+language`, HTTP 200, 321,801 bytes; trimmed to two result blocks |
| `brave` | `empty.html` | **Live-captured** | `GET https://search.brave.com/search?q=site%3Azqxjvwkkplmnrtbqzz.invalid+eggsearch`, HTTP 200, 98,964 bytes; trimmed to the rendered `#bad-results-info-banner` |
| `brave` | `blocked.html` | Synthetic reconstruction | No challenge page could be obtained on demand; minimal reconstruction of a verification interstitial |
| `duckduckgo` | `results.html` | Synthetic reconstruction | `GET https://html.duckduckgo.com/html/?q=...` returned HTTP 202 anti-bot for every query, so no result page was observable |
| `duckduckgo` | `empty.html` | Synthetic reconstruction | Same HTTP 202 anti-bot block |
| `duckduckgo` | `blocked.html` | **Live-captured** | `GET https://html.duckduckgo.com/html/?q=rust+programming+language`, HTTP 202, 14,266-byte anomaly page; trimmed, challenge-image identifiers and `gk` / `p` / `ti` tokens removed |
| `mojeek` | `results.html` | Synthetic reconstruction | `GET https://www.mojeek.com/search?q=...` returned HTTP 403 to the `curl` client |
| `mojeek` | `empty.html` | Synthetic reconstruction | Same HTTP 403 block |
| `mojeek` | `blocked.html` | **Live-captured** | `web_fetch` `GET https://www.mojeek.com/search?q=rust+programming+language`, HTTP 200 CAPTCHA page; trimmed, `data-id` identifier and scripts removed. A second live variant (`curl`, HTTP 403, body text "Sorry your network appears to be sending automated queries") was also observed |
| `startpage` | `results.html` | **Live-captured** | `GET https://www.startpage.com/search?q=rust+programming+language` with a solved Anubis `spchal-auth` cookie, HTTP 200, 251,915 bytes, 2026-11-11 22:56 UTC, difficulty 6; payload reduced to `render.presenter.regions.mainline` and trimmed to the two `web-google` result blocks |
| `startpage` | `empty.html` | Synthetic reconstruction | No live empty SERP was observable: every query, including a nonsense string, returned ten fuzzy `web-google` results. The file reproduces the captured payload shape with an empty `web-google` region plus the documented `div.no-results` marker |
| `startpage` | `blocked.html` | **Live-captured** | `GET https://www.startpage.com/search?q=...` → HTTP 302 → `https://cdn.startpage.com/sp/cdn/error-pages/blocked.html`, HTTP 200, 38,957 bytes; trimmed to the page shell and title |
| `yahoo` | `results.html` | Synthetic reconstruction | `GET https://search.yahoo.com/search?p=...` returned HTTP 500 with an empty body from the edge (ATS) for every query and header set |
| `yahoo` | `empty.html` | Synthetic reconstruction | Same HTTP 500 edge block |
| `yahoo` | `blocked.html` | Synthetic reconstruction | The edge returned an empty 500, not a renderable page; minimal reconstruction of the Yahoo consent wall |

Six fixtures are live-captured (two Brave, three block pages, one Startpage
solved payload). The other nine are small synthetic reconstructions that mirror
the selectors and page shape each parser already targets; they exist so the
conformance suite can exercise the `results` and `empty` outcomes for providers
that refuse automated requests from this environment. No fixture claims to be a
live capture unless the table above says so.

## Startpage payload capture (2026-11-11 UTC)

Startpage serves no result markup: the page builds its results client-side from
an inline React payload. `startpage/results.html` derives from one solved
session:

- URL `https://www.startpage.com/search?q=rust+programming+language`
- HTTP 200, 2026-11-11 22:56:13 UTC, 251,915 bytes
- Anubis `fast` challenge at difficulty 6; the pass cookie was obtained by
  solving that exact challenge and replaying `spchal-auth`
- The live challenge object carries no `redir` field. The shipped solver's
  `fast` parser requires one and refuses such a page with `malformed
  parameters`, so this capture's cookie came from solving the same published
  challenge outside the repository (one SHA-256 over `randomData` + nonce until
  six leading hex zeros) and replaying it with the same honest user agent
- Category token `AppSerpWeb`; `display_type` `web-google`
- `mainline` carried 9 regions (spellsuggest, two enrichment slots, two top ad
  slots, one bottom ad slot, product ads, and one web region with 10 results)

The committed file keeps only `render.presenter.regions.mainline[].display_type`
and `results[].clickUrl` / `title` / `description` for two `web-google` results.
The top ad region is retained with an empty `results` list: its live entries are
nothing but tracking identifiers (`thash`, `imp_thash`, `page_options`) and were
dropped rather than redacted. No cookie, JWT, challenge id, impression token,
`thash`, session id, or page id is retained.

## Sanitisation and size

- No cookies, credentials, tokens, session identifiers, challenge-image hashes,
  `_ylt` / `RS` redirect identifiers, or personal data are retained.
- Non-fixed-format identifiers in live captures were replaced with `redacted` or
  removed; the query text retained is the public nonsense probe.
- Each live capture was trimmed to the smallest fragment that still exercises the
  parser. Every committed file is under 2 KB; the repository hard ceiling for a
  fixture is 100 KB.

## Empty-state and block markers

Markers are declared as constants next to each parser (`SHAPE` in
`src/meta/engines/<provider>.rs`). Text markers are matched against rendered
page text only; `<script>` and `<style>` contents are ignored so locale bundles
in live pages cannot masquerade as an empty state.

| Provider | Result container | Empty markers | Blocked markers |
|---|---|---|---|
| `brave` | `div[data-type='web']` | `#bad-results-info-banner`, `div.fallback-snippet`, text `Not many great matches came back for your search` | text `Please verify you are a human` (synthetic) |
| `duckduckgo` | `div.result` | `div.no-results` (synthetic), text `No results.` (synthetic) | `#challenge-form`, `div.anomaly-modal`, text `Unfortunately, bots use DuckDuckGo too.` (live) |
| `mojeek` | `ul.results-standard, ol.results-standard` | `div.no-results` (synthetic), text `No pages found` (synthetic) | `div.captcha-wrap`, text `JavaScript is required to complete this challenge`, text `your network appears to be sending automated queries` (live) |
| `startpage` | `div.result` (payload-first: `React.createElement(UIStartpage.AppSerpWeb, …)` → `render.presenter.regions.mainline[]`, `display_type` prefix `web`) | `div.no-results` (synthetic), text `No search results were found`, text `No results found for` (synthetic) | text `Access Denied - Startpage` (live), `#anubis_challenge`, `#anubis_version` (live, 2026-11-11) |
| `yahoo` | `div.algo-sr` | text `We did not find results for` (synthetic) | text `Before you continue to Yahoo` (synthetic) |

The Brave empty markers are the only empty-state markers derived from live-rendered
markup; the DuckDuckGo, Mojeek, and Startpage blocked markers are also live-derived.
All remaining markers are synthetic: they use the provider's documented wording or
class convention and must be re-confirmed at the next successful live capture.
Until then they are a reconstruction, not an observation.

`div.dd.algo` was removed from the Yahoo empty markers on 2026-11-11: it matches
the class list of every organic result container (`dd fst algo algo-sr`), so a
drifted title selector would have produced a confident empty state instead of
`parse_error`.

## Verification (2026-11-11 UTC)

Each engine was exercised end to end through the shipped client
(`eggsearch search --providers <id>`), not through `curl`:

| Provider | Observation |
|---|---|
| `duckduckgo` | HTTP 200, 5/5 parsed rows with snippets, `uddg` redirects decoded |
| `brave` | HTTP 200, 5/5 parsed rows; 20 `div[data-type='web']` containers and 20 `a.l1` links |
| `yahoo` | HTTP 200, 5/5 parsed rows, `RU=` redirects decoded. Raw `curl` against `search.yahoo.com` returned HTTP 500 over HTTP/2 but 200 over HTTP/1.1; the shipped client observes 200 |
| `mojeek` | HTTP 200 CAPTCHA page, classified `blocked` through `div.captcha-wrap` |
| `startpage` | HTTP 200 Anubis proof-of-work page on `/search`, `/sp/search`, and `POST /do/search`, including with a browser user agent; classified `blocked` |
| `startpage` | 2026-11-11 22:56 UTC, difficulty-6 challenge solved: the replayed `/search?q=rust+programming+language` returned HTTP 200 with the `UIStartpage.AppSerpWeb` payload and 10 `web-google` results. The five synthetic-DOM unit fixtures still cover the `div.result` fallback |

The `results.html`, `empty.html`, and `blocked.html` fixtures for DuckDuckGo,
Mojeek, Startpage, and Yahoo remain the synthetic reconstructions recorded above
even though DuckDuckGo, Brave, and Yahoo answer live again. A re-capture is
outstanding; do not describe those fixtures as live-captured until it happens.

## Challenge solving (2026-11-11 UTC)

Measured with ADR-0009 solving enabled, through the shipped release binary
(`cargo build --release`; `eggsearch --config … search --providers <id>`):

| Provider | Configuration | Observation |
|---|---|---|
| `mojeek` | `[search.challenge] enabled = false` | 0 results, `error_class: blocked`, no extra request |
| `mojeek` | `enabled = true`, `max_difficulty = 5`, `timeout_ms = 12000` | 5/5 parsed rows in 3.56 s wall clock; no provider failures |

A fresh live `GET https://www.mojeek.com/captcha/challenge` published
`algorithm = PBKDF2/SHA-256`, `cost = 8000`, `keyLength = 32`, and a 16-byte
`keyPrefix`. The reference algorithm resolved it at counter 248 in 0.34 s
outside this repository, confirming that the search space is the counter range
rather than the prefix length.

A **debug** build (`cargo build`) does not complete the same solve inside a
12-second budget: unoptimised `sha2`/`pbkdf2` derivation is roughly two orders
of magnitude slower, so the solve reports `blocked` with
`challenge not solved: wall clock exhausted`. Debug builds are not a valid
performance baseline for this capability; measure with `--release`. The
classification is correct in both builds, and the disabled path is unchanged.


## Challenge solver conformance (2026-11-13 UTC)

Re-run through the shipped release binary after the M013/M016 solver corrections
(`eggsearch --config … search --providers startpage --max-results 3`):

| Provider | Configuration | Observation |
|---|---|---|
| `startpage` | `enabled = true`, `max_difficulty = 6`, `max_attempts = 50000000`, `timeout_ms = 15000` | 3/3 parsed rows, no provider failures; the wall still publishes `fast` at difficulty 6, and the baseline pass-challenge path and query are unchanged |
| `startpage` | `enabled = true`, `max_difficulty = 2`, difficulty 6 published | blocked in 0.27 s with `anubis difficulty 6 exceeds configured ceiling 2`; zero pass-challenge requests |
| `startpage` | `enabled = true`, `max_attempts = 100` | blocked in 0.28 s with `challenge not solved: attempts exhausted` |
| `startpage` | `enabled = true`, `timeout_ms = 100` | blocked in 0.32 s with `challenge not solved: wall clock exhausted` |

No deployment publishing `anubis_base_prefix` with a non-empty value, `method = "slow"`,
`method = "preact"`, or `method = "metarefresh"` was reachable on 2026-11-13. Those paths are
synthetic-fixture verified only and are not described here as live observations.
