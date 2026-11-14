# M027 Anubis `css-load` Extension Support — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/027-anubis-css-load-extension.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `fd2634e`

Implementation candidate and closure commit: the commit that carries this record.

## Result

ADR-0012 authorizes one non-interactive extension class; this milestone implements its only member.
A `css-load` challenge performs exactly one bounded same-origin `GET` before the method proof, then
the normal pass request.

- `satisfy_extensions` partitions the parsed list: every name `css-load` continues; any other name
  takes M026's zero-request refusal. Empty lists continue.
- The URL is built from the validated base prefix, the fixed
  `/.within.website/x/cmd/anubis/ext/css-load/` path, and a validated UUID-shaped challenge id, then
  checked same-origin. A traversal-shaped or control-character id is `malformed parameters` before
  any request.
- The request carries no cookie, authorization header, body, or operator value; redirects are
  disabled, the timeout is the remaining solve deadline, and the 4 KiB body is discarded. A 2xx
  continues; a non-2xx status refuses with
  `anubis css-load extension was not satisfied (status <code>)`, and a transport failure or timeout
  refuses with `anubis css-load extension was not satisfied`. Both record `unsupported scheme` and
  send zero pass requests.
- `headless` and unknown names keep M026's zero-request refusal. The ten-key vocabulary and
  `provider_status` shape are unchanged; no dependency, configuration key, or tool was added.

## Live evidence (2026-11-13 UTC)

No deployment publishing `"extensions": ["css-load"]` was observed. The two live deployments probed
carried no extension list (`extensions: null` on v1.28.3, no key on Startpage v1.26.4), so the
capability is **synthetic-fixture verified only**. The closure does not claim a live pass. The
open-source path and the 200/404 behaviour were confirmed against
`lib/challenge/extension/css-load/cssload.go`, whose `renderCSS` validates the issued UUID and
returns an empty 200 without requiring a cookie.

## Verification

```bash
cargo test --locked --all-features --lib meta::challenge   # 118 passed; one GET then pass, 404 refusal, no-cookie canary, large-body discard, unsafe id
cargo test --locked --all-features --lib meta::engines::startpage  # 55 passed; disabled path performs zero extension requests
cargo test --locked --all-features --test dispatch_fault_injection  # green in the full all-features run
```
cargo test --locked --all-features --no-fail-fast   # 6306 passed, 0 failed, 28 ignored (85 test binaries)
