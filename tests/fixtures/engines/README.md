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
| `startpage` | `results.html` | Synthetic reconstruction | `GET https://www.startpage.com/search?q=...` returned HTTP 302 to the CDN error page for every query |
| `startpage` | `empty.html` | Synthetic reconstruction | Same HTTP 302 redirect to the blocked page |
| `startpage` | `blocked.html` | **Live-captured** | `GET https://www.startpage.com/search?q=...` → HTTP 302 → `https://cdn.startpage.com/sp/cdn/error-pages/blocked.html`, HTTP 200, 38,957 bytes; trimmed to the page shell and title |
| `yahoo` | `results.html` | Synthetic reconstruction | `GET https://search.yahoo.com/search?p=...` returned HTTP 500 with an empty body from the edge (ATS) for every query and header set |
| `yahoo` | `empty.html` | Synthetic reconstruction | Same HTTP 500 edge block |
| `yahoo` | `blocked.html` | Synthetic reconstruction | The edge returned an empty 500, not a renderable page; minimal reconstruction of the Yahoo consent wall |

Five fixtures are live-captured (two Brave, three block pages). The other ten are
small synthetic reconstructions that mirror the selectors and page shape each
parser already targets; they exist so the conformance suite can exercise the
`results` and `empty` outcomes for providers that refuse automated requests from
this environment. No fixture claims to be a live capture unless the table above
says so.

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
| `startpage` | `div.result` | `div.no-results` (synthetic), text `No search results were found`, text `No results found for` (synthetic) | text `Access Denied - Startpage` (live) |
| `yahoo` | `div.algo-sr` | `div.dd.algo`, text `We did not find results for` (synthetic) | text `Before you continue to Yahoo` (synthetic) |

The Brave empty markers are the only empty-state markers derived from live-rendered
markup; the DuckDuckGo, Mojeek, and Startpage blocked markers are also live-derived.
All remaining markers are synthetic: they use the provider's documented wording or
class convention and must be re-confirmed at the next successful live capture.
Until then they are a reconstruction, not an observation.
