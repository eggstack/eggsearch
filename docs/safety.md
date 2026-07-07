# Safety and Fetch Behavior

eggsearch treats all retrieved content as evidence, not instructions.

## Trust Labels

- `external_untrusted` for web and remote content
- `local_trusted` for operator-configured local workspace content
- `unknown` when trust cannot be determined

`external_untrusted` is the default posture for search and fetch output. Even `local_trusted` content is provenance-trusted only; comments and text can still be adversarial.

## Fetch Boundaries

`web_fetch` accepts one explicit HTTP(S) URL at a time. It does not crawl, does not execute JavaScript, and only follows a bounded number of validated redirects.

Code-host source-file URLs are rewritten to raw fetch targets, then run through the same validation path as ordinary URLs.

For DNS-backed hosts, eggsearch validates the resolved address set and reuses that validated set for the outbound request attempt. Redirect targets are revalidated and re-pinned before they are followed.

## Blocked Address Ranges

When `allow_private_network = false` (the default), eggsearch blocks fetches to the following IPv4 ranges:

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

When `allow_localhost = false` (the default), loopback addresses are blocked regardless of `allow_private_network`.

IPv6 blocked ranges include: loopback (`::1`), unspecified (`::`), unique-local (`fc00::/7`), link-local (`fe80::/10`), multicast (`ff00::/8`), documentation (`2001:db8::/32`), benchmarking (`2001:2::/48`), discard-only (`2001::/32`), deprecated 6to4 (`2002::/16`), and IPv4-mapped addresses targeting any blocked IPv4 range.

Redirect targets are revalidated against these same ranges before being followed.

`allow_private_network = true` and `allow_localhost = true` are operator escape hatches. Keep them off for general MCP exposure.

Local repository fetches use separate path validation. The path checks reject:

- empty paths
- absolute or prefix paths (cross-platform)
- parent-directory (`..`) traversal components
- known binary extensions
- symlinks when `follow_symlinks = false`
- paths that escape the configured workspace root

Filenames that merely contain two dots (e.g. `foo..bar.rs`) are accepted.

## Sanitization Defaults

Both search and fetch default to `sanitize_output = true`.

That means:

1. Control characters are stripped.
2. Text is length-bounded.
3. Untrusted text is framed with `<<<EXTERNAL_UNTRUSTED>>>` delimiters.
4. Prompt-injection marker scanning is enabled.

Setting `sanitize_output = false` disables framing and marker scanning, but control-character stripping and length bounding still happen.

## `metadata_only`

`web_fetch` supports `extract_mode = "metadata_only"`.

- For HTML, eggsearch still reads a bounded response body so it can extract title and description metadata, but it suppresses body text and the structured document.
- For plain-text or other non-HTML responses, eggsearch suppresses body text and does not build a structured document.
- For PDF responses when the `pdf` feature is enabled, eggsearch returns a minimal document with fetch context but no extracted body text.

Use `metadata_only` when you need page metadata but do not need the body content itself.

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
