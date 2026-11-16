# M025 Anubis `metarefresh` Presentation Corrective — Closure Status

Status: closed

Source plan: `plans/archive/implementation/agentic-retrieval-integrity/025-metarefresh-presentation-corrective.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `fd2634e`

Implementation candidate and closure commit: the commit that carries this record.

## Result

`metarefresh` no longer depends on how the challenge page presents the refresh, and the echo token
is the published `challenge.randomData`.

- `AnubisChallenge::parse` treats the presentation as optional: `published_meta_refresh` now returns
  `Option<(Duration, Option<String>)>` instead of `BOUND_MALFORMED` when no `<meta>` exists.
  `from_metarefresh` accepts `randomData` first and a legacy `challenge` field only when
  `randomData` is absent; a page carrying both uses `randomData`.
- Wait: the published meta wait wins and is checked against a bounded JSON `wait`; without a
  presentation the JSON `wait` is used when present, otherwise `difficulty + 1` seconds. Every
  requirement is floored at `difficulty × 800 ms` and refused when it exceeds the remaining
  deadline (`wall clock exhausted`).
- Submission: `metarefresh_submission` builds
  `{origin}{base_prefix}/.within.website/x/cmd/anubis/api/pass-challenge?redir=<request URL>&challenge=<randomData>&id=<id>`,
  uses a published target only after a same-origin check, and never appends `result=`. The removed
  `metarefresh_url`/`result=` shape is gone.
- `metarefresh` still consumes no `SolveBudget` attempts.

## Live evidence (2026-11-13 UTC, protocol level)

`https://anubis.techaro.lol/docs/design/how-anubis-works/` (v1.28.3) published
`"rules":{"algorithm":"metarefresh","difficulty":1}` with
`challenge.randomData` (128 hex), `challenge.extensions: null`, and an
`<meta http-equiv="refresh" content="2; url=...pass-challenge?challenge=...&id=...&redir=%2Fdocs%2F">`.
An earlier capture of the same page on the same day carried the refresh only in the `Refresh:`
response header, which is the parity-dependent presentation
(`showMeta = randomData[0] % 2 == 0` upstream); both presentations are now accepted.

Submitting `challenge=<randomData>&id=<id>&redir=/docs/` with the issuance verification cookie after
the wait returned `302 location: /` plus the clearance cookie. That confirms the token field, the
wait floor, and the synthesized URL shape. No end-to-end `web_fetch` run was performed in this
environment; the record is protocol-level plus synthetic.

## Verification

```bash
cargo test --locked --all-features --lib meta::challenge   # 118 passed; includes header-only, presentation-less, legacy-token and cross-origin tests
cargo test --locked --all-features --lib meta::engines::startpage  # 55 passed; header-only engine flow
```
cargo test --locked --all-features --no-fail-fast   # 6306 passed, 0 failed, 28 ignored (85 test binaries)
