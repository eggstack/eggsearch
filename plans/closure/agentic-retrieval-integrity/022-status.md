# M022 Fetch-Side Challenge Classification — Closure Status

Status: closed

Source plan: `plans/implementation/agentic-retrieval-integrity/022-fetch-side-challenge-classification.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Policy: ADR-0009 and ADR-0010 (solver scope unchanged; this milestone never solves or submits)

Planning baseline: `462fe6b` (the commit that closed M021)

Implementation candidate and closure commit: the commit that carries this record.

## Result

`src/fetch/challenge.rs` adds an ungated detector for four challenge families — Anubis, ALTCHA,
interactive (Turnstile/reCAPTCHA/hCaptcha), and verification interstitials. The plain HTTP
extraction path classifies the bounded HTML body and, on a hit:

- sets `manual_interaction_required: true` on the response;
- appends one `AgentWarning` with the new stable code `fetch_challenge_page` naming the family;
- leaves `status`, `fetched`, `text`, and `document` untouched, so the body the server sent is
  still returned.

`src/mcp/tools/web_fetch.rs` merges the response warnings and now also uses the detector as an
escalation trigger under `render: "auto"`. `src/mcp/tools/batch_fetch_item.rs` propagates the flag
and the warning per item on the live fetch path.

## Measured payloads

Before (release without `browser`, 2026-11-13):

```
status: 200, fetched: true, manual_interaction_required: false, text_chars_returned: 116
text: "Loading...\n\nAnubis could not load its JavaScript. The server may be overloaded."
warnings: [fetch_warning]
```

After (`render: "http_only"`):

```
status: 200, fetched: true, manual_interaction_required: true
structured_warnings: [fetch_warning, fetch_challenge_page]
message: "challenge page detected (anubis): the response is an anti-bot challenge, not the requested document"
```

An ordinary page (`https://example.com/`) stays `manual_interaction_required: false` with no
`fetch_challenge_page` warning.

## Browser evidence

`[fetch.browser] enabled = true` with the `browser` feature compiled:

| Request | Observation |
|---|---|
| `render: "auto"`, Startpage Anubis wall | escalated; 3.6 s; failed closed with `manual_interaction_required` / "Interactive challenge detected" instead of returning the stub |
| `render: "browser"`, same wall | same structured error; `next_action: eggsearch browser-login … --profile <name>` |
| `render: "browser"`, `https://example.com/` | `transport: "browser"`, 2.32 s, then 1.82 s on a second call in the same session (browser reused) |
| `render: "auto"`, ordinary page | no escalation; `transport: "http"` |

Headless Chrome does not pass this Anubis wall, and the browser path correctly refuses to click,
simulate input, or use a solving service. Before this change the same wall was returned as an
ordinary 116-character document under `auto` because neither classifier recognised it.

## Verification

```bash
cargo fmt --all -- --check                    # clean
cargo clippy --locked --all-targets --all-features -- -D warnings   # clean
cargo check --locked --no-default-features    # clean
cargo test --locked --all-features --test web_fetch_integration web_fetch_marks_anubis_and_altcha  # 1 passed
cargo test --locked --all-features --test batch_fetch_retrieval batch_marks_challenge            # 1 passed
cargo test --locked --all-features --test static_guards                              # 62 passed
cargo test --locked --all-features                                  # 6225 passed / 28 ignored
cargo test --locked --all-features --lib fetch::challenge                                       # 3 passed
```

Unit coverage: `detects_each_family`, `leaves_ordinary_pages_alone`, `family_names_are_stable`.
The negative cases include an ordinary "Loading..." page and a body containing "please wait",
which the detector deliberately does not treat as a challenge.

## Residual risk

- Detection is heuristic. A page that obfuscates its challenge ids or uses a new family is not
  marked; a legitimate page that quotes `anubis_challenge` or `captcha-wrap` would be marked.
  Neither was observed, but the second is a false-positive channel.
- The flag and warning are produced on the live fetch path. Cache-hit paths in
  `batch_fetch_item.rs` still hardcode `manual_interaction_required: false` because the cached
  derived entry carries no warning set; a cached challenge page can therefore lose the signal on a
  hit. The live path is correct and covered.
- The browser path fails closed on Anubis rather than obtaining the document. That is the intended
  policy, not a defect, but it means the browser leg does not make this wall reachable.
- Sequential short-lived MCP processes can leave a chromiumoxide `SingletonLock` in the shared
  temp profile, making the next independent process's first browser launch fail. A single session
  reuses the browser and was not affected; this is a harness/upstream-library artifact, not an
  eggsearch code path.
