# Standalone MCP Local-Inference M004 — Corrective Status

Status: blocked (schema correction and offline qualification pass; post-fix LM Studio interaction was not reliably refreshed)

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

The LM Studio `mcp.json` on disk was temporarily pointed at the corrected
release candidate and compact profile, then restored byte-for-byte to the
original installed-binary configuration. The app's accessibility tree
continued to report a Chat route while its visible window remained on Model
Server; save/refresh actions could not be verified. No post-fix model turn is
claimed, and no API authentication or MCP permission setting was changed.

M004 therefore remains blocked on a fresh Gemma E2B research-schema and
official-page fetch task against this exact candidate, with LM Studio refresh
and rollback visibly verified. M003 remains blocked on broader paired model
trials, exact tokenizer and prompt-token attribution, structured-content and
refresh behavior, protected API qualification, and resolution of the
observed quality issues. Do not claim the enum correction fixes Gemma's
research quality or the incomplete fetch answer until a new live run confirms
it.
