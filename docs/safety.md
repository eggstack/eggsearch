# Safety and Fetch Behavior

eggsearch treats all retrieved content as evidence, not instructions.

For the full operator threat model — including trust boundaries, configuration escape hatches, prompt-injection handling, local workspace caveats, provider disclosure notes, and recommended host-agent policy — see [threat-model.md](threat-model.md).

## Trust Labels

- `external_untrusted` for web and remote content
- `local_trusted` for operator-configured local workspace content
- `unknown` when trust cannot be determined

`external_untrusted` is the default posture for search and fetch output. Even `local_trusted` content is provenance-trusted only; comments and text can still be adversarial.

## Fetch Boundaries

`web_fetch` accepts one explicit HTTP(S) URL at a time. It does not crawl, does not execute JavaScript, and only follows a bounded number of validated redirects.

Code-host source-file URLs are rewritten to raw fetch targets, then run through the same validation path as ordinary URLs.

For DNS-backed hosts, eggsearch performs preflight address classification before the outbound request. The HTTP client resolves DNS independently at connection time, so this is not connection-time DNS pinning and does not eliminate DNS-rebinding TOCTOU risk. Redirect targets are rejected by the forge client and independently revalidated by the user-fetch path.

## Blocked Address Ranges

eggsearch classifies every IP address into one of eight categories: Loopback, Private, LinkLocal, CarrierGradeNat, Documentation, Multicast, Reserved, or Public. Two independent boolean operators control access:

| `allow_localhost` | `allow_private_network` | Loopback | Private / LinkLocal / CGNAT / Documentation / Reserved / Multicast | Public |
|:-:|:-:|:-:|:-:|:-:|
| false | false | blocked | blocked | allowed |
| false | true | blocked | allowed | allowed |
| true | false | allowed | blocked | allowed |
| true | true | allowed | allowed | allowed |

`allow_localhost` controls only loopback addresses (127.0.0.0/8, ::1). `allow_private_network` controls all other non-public ranges. The two flags are fully independent — setting one does not affect the other.

IPv4 blocked ranges (when the relevant flag is false):

| Range | RFC | Purpose |
|-------|-----|---------|
| `0.0.0.0/8` | RFC 1122 | "This" network |
| `10.0.0.0/8` | RFC 1918 | Private (Class A) |
| `100.64.0.0/10` | RFC 6598 | Shared address space (carrier-grade NAT) |
| `127.0.0.0/8` | RFC 1122 | Loopback |
| `169.254.0.0/16` | RFC 3927 | Link-local |
| `172.16.0.0/12` | RFC 1918 | Private (Class B) |
| `192.0.0.0/24` | RFC 6890 | IETF protocol assignments |
| `192.0.2.0/24` | RFC 5737 | Documentation (TEST-NET-1) |
| `192.88.99.0/24` | RFC 3068 | 6to4 relay (deprecated) |
| `192.168.0.0/16` | RFC 1918 | Private (Class C) |
| `198.18.0.0/15` | RFC 2544 | Benchmarking |
| `198.51.100.0/24` | RFC 5737 | Documentation (TEST-NET-2) |
| `203.0.113.0/24` | RFC 5737 | Documentation (TEST-NET-3) |
| `224.0.0.0/4` | RFC 5771 | Multicast |
| `240.0.0.0/4` | RFC 1112 | Reserved |

IPv6 blocked ranges (when the relevant flag is false): loopback (`::1`), unspecified (`::`), unique-local (`fc00::/7`), link-local (`fe80::/10`), multicast (`ff00::/8`), documentation (`2001:db8::/32`), benchmarking (`2001:2::/48`), discard-only (`2001::/32`), deprecated 6to4 (`2002::/16`), and IPv4-mapped addresses targeting any blocked IPv4 range.

Redirect targets are revalidated against these same ranges before being followed.

`allow_private_network = true` and `allow_localhost = true` are operator escape hatches. Keep them off for general MCP exposure.

Local repository fetches use separate path validation. The path checks reject:

- empty paths
- absolute or prefix paths (cross-platform)
- parent-directory (`..`) traversal components
- known binary extensions
- skipped directories (`.git`, `target`, `node_modules`, etc.) regardless of `include_hidden`
- hidden path components when `include_hidden = false`
- symlinks in any path component when `follow_symlinks = false`
- paths that escape the configured workspace root

When `follow_symlinks = true`, the kernel enforces containment via `RESOLVE_BENEATH` on Linux. On non-Linux Unix platforms, `follow_symlinks = true` returns `SafeSymlinkFollowingUnsupported` because no race-safe containment primitive is available.

Filenames that merely contain two dots (e.g. `foo..bar.rs`) are accepted.

## Sanitization Defaults

Both search and fetch default to `sanitize_output = true`.

That means:

1. Control characters are stripped.
2. Text is length-bounded.
3. Untrusted text is framed with `<<<EXTERNAL_UNTRUSTED>>>` delimiters.
4. Prompt-injection marker scanning is enabled.

Setting `sanitize_output = false` disables framing and marker scanning, but control-character stripping and length bounding still happen.

### What gets sanitized

- **Title and description fields** — Tier 1 (strip + bound at 200/500 chars respectively), plus Tier 2/3 when sanitization is enabled.
- **Search excerpts** — same pipeline as snippets (bound at 500 chars per excerpt, 1,200 chars total per card), with injection hits merged into the card's `trust_markers`.
- **Developer Index passages** — Firecrawl matched markdown passages arrive as bounded `ProviderPassage` excerpts through the same pipeline. They remain search-result evidence (`external_untrusted`, `fetched=false`); they never authorize code changes and never enter instruction-trusted content. Fetch the issue/PR/docs URL explicitly to read full content.
- **Body text** — Tier 1 (strip + bound at `max_chars`), plus Tier 2/3 when sanitization is enabled.
- **Focused fetch chunks** — Tier-1 text drawn from the already-sanitized stored document; the response-level `trust_markers` cover the extracted text the selection was drawn from.
- **Document block text** — Tier 1 only (strip + bound per block).
- **Document outline titles** — Tier 1 (strip + bound at 500 chars) applied to all outline entries from HTML, Markdown, and notebook renderers.
- **Outline anchors** — HTML `id` attributes passed through as-is; `make_slug()`-generated anchors are filtered to alphanumeric/hyphen/underscore.

### raw_text (internal)

`raw_text` is an internal field on `WebFetchResponse`, bounded by `max_chars_cap` (default 50k). It is **not** serialized in MCP tool output. It provides Tier-1-only text for internal consumers like `repo_fetch` line/span selection. Related metadata fields (`raw_text_chars_returned`, `raw_text_truncated`, `raw_text_cap`) are also internal-only.

## `metadata_only`

`web_fetch` supports `extract_mode = "metadata_only"`.

- For HTML, eggsearch still reads a bounded response body so it can extract title and description metadata, but it suppresses body text and the structured document.
- For plain-text or other non-HTML responses, eggsearch suppresses body text and does not build a structured document.
- For PDF responses when the `pdf` feature is enabled, eggsearch returns a minimal document with fetch context but no extracted body text.

Use `metadata_only` when you need page metadata but do not need the body content itself.

## Focused Fetch and Cache Controls

`web_fetch` accepts an optional `focus` query for deterministic query-focused chunk selection. Focus ranking is lexical and local (token overlap, exact-phrase, heading, and code-symbol boosts); it performs no extra URL traversal and calls no model. Focused chunk texts are projections of the already-fetched document, never generated summaries.

`cache_policy` (`default`/`bypass`/`refresh`) and `max_cache_age_seconds` affect cache reuse and revalidation only. They never bypass target validation, redirect checks, origin concurrency/circuit breakers, browser-profile isolation, content limits, or sanitization. `bypass` still stores the fresh response unless the origin forbids caching; `refresh` revalidates with `ETag`/`Last-Modified` when validators exist.

## Trust Markers

`trust_markers` records what eggsearch did to the returned text fields. The important fields are:

- `text_sanitized`
- `text_framed`
- `text_truncated`
- `control_chars_removed`
- `injection_hits`

`injection_hits > 0` means eggsearch saw prompt-injection markers and framed the text when sanitization is enabled.

## Security Search

`security_search` and the advisory-backed paths return advisory data and severity metadata for triage. They do not decide exploitability for a specific deployment, patch state, or runtime reachability profile.

Native security lookups (CVE/GHSA/OSV/RustSec/KEV) are instrumented for failure visibility — every selected-provider operation produces a `RetrievalAttempt` record in the retrieval ledger, including capability skips, zero results, failures, deadline interruptions, and KEV lookup failures. Advisory records may deduplicate across providers, but attempts and provider identities are retained.

Capability and absence semantics are intentionally separate. A provider that
cannot perform an applicable native operation is `provider_capability_unavailable`,
not a successful zero-result lookup. A provider that completed with no match is
`no_matching_evidence_found`. A required role skipped by capability or policy
remains indeterminate for coverage.

Candidate-limit saturation without provider metadata is possible truncation only:
the attempt uses `truncation_evidence = limit_reached_unknown`, leaves the legacy
`truncated` flag false, and increments `limit_reached_unknown_count`. Confirmed
truncation requires an Eggsearch cap or explicit provider evidence.

## Forge Endpoint Safety

Forge API base URLs (GitHub, GitLab, Gitea, Forgejo, Codeberg) are validated by `validate_base_url()` before any API request:

- **Embedded credentials** in the URL are always rejected (username or password in URL)
- **HTTPS URLs** pointing to localhost, loopback, or private/link-local/reserved IPv4/IPv6 ranges are rejected by default (configurable via `ForgeEndpointPolicy.allow_loopback` and `allow_private_network`)
- **HTTP URLs** are only allowed for localhost development use; HTTP with an API key is rejected except on loopback
- **IPv6 addresses** are fully classified: loopback, ULA (private), link-local, documentation, reserved, multicast, public
- **DNS resolution** resolves hostnames and classifies all resolved addresses against the policy (residual DNS rebinding risk documented in architecture)

This prevents forge adapters from being redirected to internal services or leaking API keys over plaintext connections.

Primary forge tree and paginated responses are read through `read_bounded_response()` with a hard byte cap (10MB per response, cumulative aggregate cap). `ForgeReadBudget` tracks aggregate bytes across all requests within a single tool invocation (operation-wide, not per-response); pagination stops when the aggregate budget is exhausted. Error-body previews (rate-limit detection, permission-denied diagnostics) are read through `read_error_preview()` with an 8KB cap and control-character sanitization. Default-branch metadata lookups use bounded response reading. Forge API clients use `Policy::none()`, rejecting all redirects; the fetch client also uses `Policy::none()` for outbound HTTP requests.

## Browser Rendering Safety

When the `browser` feature is enabled and configured, browser rendering adds these safety properties:

- **Public-network-only**: Browser transport rejects localhost, private IPv4/IPv6, link-local, and cloud metadata addresses regardless of `allow_localhost`/`allow_private_network` settings.
- **No browser download**: eggsearch discovers an already-installed Chrome/Chromium. It never downloads, installs, or manages browser updates.
- **No challenge solving**: Interactive challenges (CAPTCHAs, Turnstile) are detected and reported as structured MCP error codes (`browser_manual_interaction_required`, `browser_profile_requires_attention`). eggsearch never clicks, simulates input, or uses external solving services.
- **Profile isolation**: Anonymous browser contexts are ephemeral and incognito. Persistent profile fetches use only the Eggsearch-owned `chrome-data` directory established by `browser-login`; the user's ordinary Chrome profile is never used. Persistent fetches are request-scoped and serialized by the profile lock.
- **Deterministic executable path**: An explicitly configured invalid browser executable path fails deterministically. It does not silently fall back to auto-discovery.
- **Request interception**: All observable requests are intercepted and checked against the network policy.
- **Bounded extraction**: DOM size, request count, navigation time, and post-load wait are all bounded by configuration.
- **Configured runtime**: Startup, navigation, verification, DOM, request, and media limits are taken from the configured browser section for both anonymous and persistent execution.
- **Existing sanitation pipeline**: Rendered DOM flows through the same HTML extraction, text bounding, and prompt-injection sanitation as ordinary HTTP fetches.
