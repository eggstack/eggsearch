# Standalone MCP Local-Inference M004 — Corrective Status

Status: closed (hosted canonical Linux, macOS, and Windows portability qualification passed on final candidate `de69f1ddd7a7e4acc067fb9892c438f6165cfe83`)

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/004-compact-schema-discoverability-corrective.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

Corrected implementation candidate: `776f8ca4a8246e467041696b2220600eb7b2648d`.
Original finding: M003 `02a8af6fe4f87971c7daca7d1fca20c087b328ec`, closure
`plans/closure/standalone-mcp-local-inference/003-status.md`.

## 1. Change and evidence

The compact `research_search` schema now includes finite enum values for
`goal` and `research_domain`, including supported aliases. Goal values come
from the runtime canonical list; domain aliases are enumerated beside the
existing parser contract. Full-profile schema generation and runtime
validation are unchanged.

The schema contract test verifies that all advertised values are present and
accepted by the runtime parsers. It also retains the ten-tool, deterministic
fingerprint, and compact-size checks. The optimized all-features release
candidate passed `scripts/lm_studio_stdio_smoke.py`:

| Profile | Tool count | Serialized `tools/list` bytes | `provider_status` | Exit |
|---|---:|---:|---|---:|
| Full | 10 | 72,923 | pass | 0 |
| Compact | 10 | 18,990 | pass | 0 |

Compact remains below the 36,000-byte ceiling; the enum additions increase the
measured list by 445 bytes over M003's 18,545-byte candidate.

## 2. Verification

- `cargo fmt --check` — pass.
- `cargo test --locked --all-features --test mcp_tool_contract` — 15 passed.
- `cargo test --locked --all-features --test static_guards` — 60 passed.
- `cargo check --locked --all-features --all-targets` — pass.
- `cargo build --locked --release --all-features` — pass.
- `scripts/lm_studio_stdio_smoke.py target/release/eggsearch` — full and
  compact profiles pass, ten tools each, `provider_status` succeeds.
- `make docs-check` — pass.
- `make check` — started but manually interrupted after the
  `bounded_command` test binary stalled. A standalone rerun showed four
  existing process-group/cap-breach tests running over 60 seconds:
  `test_bounded_command_inventory_cap_breach_terminates`,
  `test_cap_breach_returns_before_timeout`,
  `test_inventory_fallback_on_stdout_cap_reason`, and
  `test_no_zombie_after_cap_breach`. No assertion failure was reported before
  interruption; the canonical gate is not green on this host.

## 3. Live qualification and disposition

The LM Studio `mcp.json` was temporarily pointed at the corrected
`target/release/eggsearch` binary with the compact profile and then restored
byte-for-byte to the original installed-binary configuration (`cmp` passed).
The model was `google/gemma-4-e2b` in LM Studio 0.4.26+4, MLX 4-bit, with a
10,703-token context. Both focused prompts were run in fresh chats. Tool
approval was per-call; neither persistent “Always allow” option was selected.
No API authentication or MCP permission setting was changed.

| Task | Tool behavior | Result quality | Context / disposition |
|---|---|---|---|
| Research: Rust 1.90.0 changes, `goal=understand`, `research_domain=language_ecosystem` | First prompt was treated as underspecified and did not call a tool. On an explicit-query retry, Gemma called `research_search` once with `query="Rust 1.90.0 release notes new features"`, `goal="understand"`, and `research_domain="language_ecosystem"`; values were valid with no argument repair. | Returned two changes and named official release-note pages, but citations were page-level and not deep links. | Final chat context meter was approximately 49.6% (5,314/10,703). The enum correction supported a valid call once the query was explicit; it did not address prompt ambiguity or establish general research quality. |
| Fetch official Rust 1.90.0 announcement and report two changes | Gemma called `web_fetch` once with the requested URL and valid arguments. | Fetch succeeded, but the visible fetched payload contained only “LLD is now the default.” Gemma correctly identified that item, then incorrectly used `$ rustup update stable` as its second “change”; that is an installation command, not a release change. | Final chat context meter was approximately 31.4% (3,359/10,703). The focused extraction criterion failed and confirms the previously recorded M003 quality issue. |

The live schema-discoverability task and compact fetch task are therefore
requalified on the exact implementation candidate, and the original MCP
configuration rollback is verified. The canonical gate was rerun with
`rtk make check` on branch HEAD `1608fe904408c29d3bfe2d3ce35d4ccc79f683fa`
(whose production code is unchanged from candidate
`776f8ca4a8246e467041696b2220600eb7b2648d`). It reached
`tests/bounded_command.rs` and remained there without test output for over two
minutes, then was manually interrupted (exit 130). No assertion failure was
reported. The same host-specific stall was recorded in the original M004
qualification. A serial retry,
`rtk cargo test --locked --all-features --test bounded_command -- --test-threads=1`,
also remained silent for over two minutes and was interrupted (exit 130), so
test parallelism did not resolve the stall. At that point, the canonical gate
was still open pending hosted qualification; the hosted result is recorded
below.

## 4. Hosted canonical requalification

The final candidate `de69f1ddd7a7e4acc067fb9892c438f6165cfe83` includes the
M004 research schema correction and passed the hosted CI workflow's canonical
`make ci` job plus Windows portability and the added macOS canonical/local
filesystem job in GitHub Actions run
[`41160264796`](https://github.com/eggstack/eggsearch/actions/runs/41160264796).
All three jobs completed successfully. The process-group/cap-breach stall did
not reproduce on hosted Linux or macOS. The same correction had also passed
earlier hosted CI run `41141113615`; the final-candidate run supersedes that
earlier evidence for closure. The prior local interruption remains useful
host-specific history, not a current blocker.

M003 remains blocked on broader paired model trials, exact tokenizer and
prompt-token attribution, structured-content and refresh behavior, protected
API qualification, and resolution/explicit acceptance of its observed
quality issues. Do not claim that the enum correction fixes prompt ambiguity,
general research quality, or incomplete fetch extraction.
