# Repository Hardening M003 — rmcp HTTP Verification on Eggfetch

Status: closed

Recommendation: closed

Implementation commits: `cb4048180e29cdeaffb77f2cc86eaec2a8e33b7b`, `85322ac127033bdc4058177ea09cff67417308d0`

## Requirement evidence

| Requirement | Evidence | Result |
|---|---|---|
| Move local HTTP verification to eggfetch | `src/integrations/http_verification.rs` owns bounded health, initialize, initialized notification, tools/list, and session deletion; `common.rs` calls the helper | pass |
| Bound response, session ID, deadlines, and redirects | Redirects disabled; five-second timeout policy; 64 KiB decoded body cap; 256-byte session ID cap; JSON/SSE body parsing before decoding is bounded | pass |
| Preserve stdio integration verification | rmcp `client` and `transport-child-process` features remain enabled; stdio path unchanged | pass |
| Remove rmcp reqwest client feature | `transport-streamable-http-client-reqwest` removed from Cargo manifest and lock resolution | pass |
| Guard dependency graph | `packaging/check-http-dependency-shape.py` fails if default graph gains reqwest or if all-features reverse dependencies differ from chromiumoxide alone | pass |

## Verification

- Private verifier tests: 6 passed, covering ready lifecycle and DELETE, missing tools, malformed initialize, oversized body/session ID, timeout, HTTP failure, redirects, JSON/SSE bounds, and SSE decoding.
- `cargo test --locked --all-features --test mcp_http --test mcp_2026_protocol --test mcp_tool_contract --test mcp_tools`: included in all-features `make check`; pass.
- `packaging/check-http-dependency-shape.py`: default graph contains no reqwest; all-features graph contains reqwest only through chromiumoxide 0.9.1.
- `make check`, `make docs-check`, `make bench-check`: pass.
- `make release-check`: pass, including release build and publish dry-run.
- Exact-candidate GitHub CI passed: run [`36467672672`](https://github.com/eggstack/eggsearch/actions/runs/36467672672).

No public API, registration configuration, MCP tool, or HTTP server transport
changed. The all-features reqwest residue is tracked by M006.
