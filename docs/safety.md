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

`allow_private_network = true` and `allow_localhost = true` are operator escape hatches. Keep them off for general MCP exposure.

Local repository fetches use separate path validation. The path checks reject:

- empty paths
- absolute paths
- `..` traversal
- known binary extensions
- symlinks when `follow_symlinks = false`
- paths that escape the configured workspace root

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
