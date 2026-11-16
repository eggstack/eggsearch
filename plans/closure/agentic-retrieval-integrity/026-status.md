# M026 Anubis Challenge Extensions Fail Closed — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/026-anubis-extension-fail-closed.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `fd2634e`

Implementation candidate and closure commit: the commit that carries this record.

## Result

`challenge.extensions` is parsed and bounded, and a non-empty list that names anything this build
cannot perform refuses before any work. **No extension is authorized by this milestone**; `css-load`
support is M027 under ADR-0012, and `headless`/BotStopper remains prohibited.

- `parse_extensions` accepts absent, `null`, and `[]`; a present array holds at most 8 names of
  1–64 bytes drawn from ASCII alphanumerics, `-`, `_`, and `.`. Any other type, a longer list, a
  longer name, an empty name, a control character, or a non-string element is
  `malformed parameters`.
- `satisfy_extensions` refuses a list containing any name other than `css-load` immediately after
  parsing, before `check_ceiling`, any wait, any hash, and any request, with
  `anubis challenge declares unsupported extensions [<names>]`. The names are the parsed,
  validated values, so no raw page text is echoed.
- The refusal is recorded under the existing `unsupported scheme` key via `blocked_step`; the
  ten-key vocabulary and the `provider_status.challenge_solver` shape are unchanged.

## Live evidence (2026-11-13 UTC)

`https://anubis.techaro.lol/docs/design/how-anubis-works/` (v1.28.3) carried
`"extensions": null` and continued to solve through the normal metarefresh path, confirming the
null-compatible regression. No deployment publishing a non-empty `extensions` list was observed, so
the refusal path is synthetic-fixture verified only. That limitation is recorded rather than
reported as a live pass.

## Verification

```bash
cargo test --locked --all-features --lib meta::challenge   # 118 passed; absent/null/empty, refusal, malformed shapes, ledger delta, canary
cargo test --locked --all-features --test dispatch_fault_injection  # green in the full all-features run
```
cargo test --locked --all-features --no-fail-fast   # 6306 passed, 0 failed, 28 ignored (85 test binaries)
