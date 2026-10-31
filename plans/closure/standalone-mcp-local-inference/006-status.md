# Standalone MCP Local-Inference M006 — Closure Status

Status: active (local implementation and qualification pass; hosted CI pending)

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/006-compact-direct-fetch-tool-choice-corrective.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

Implementation candidate: pending commit and hosted Linux/macOS/Windows qualification.
Native API test executable SHA-256: `9b70544f64bc1d87a3956480c020074f3ae4e65e5002a927d83bf973b448f801`.
Final local stdio-smoke executable SHA-256: `52385c8454808a0c784f980dd12e79542ac8ab10a6133ace87396c875c4814dc`.

## 1. Corrective

M003's authenticated native LM Studio pair recorded two compact direct-fetch
turns that skipped `web_fetch` while full fetched the explicit user URL. The
compact description was expanded in `src/mcp/server.rs` to say user-provided
URLs are valid targets, fetched text is bounded, `batch_fetch` handles several
targets, and `web_fetch` does not search or crawl. A contract test in
`tests/mcp_schema_slimming.rs` protects those details.

The update preserves all ten tools, schemas, validation rules, defaults, and
transport behavior. Full-profile descriptions remain unchanged.

## 2. Native LM Studio qualification

LM Studio 0.4.26+4's authenticated native `POST /api/v1/chat` plugin path was
used with `lfm3-2b-omni-abliter8`, 24,000-token context, temperature 0, and the
same direct-fetch query, system prompt, allowed tool, and official URL for all
turns. The pre-change executable was retained for a compact baseline.

| Build/profile/trial | Input/output tokens | Tool outcome | Evidence outcome |
|---|---:|---|---|
| Saved baseline compact 1 | 855 / 110 | One valid `web_fetch` | Correct answer; same official document and permission-edit evidence. |
| Saved baseline compact 2 | 855 / 110 | One valid `web_fetch` | Correct answer; exact result hash repeated. |
| Candidate full | 1,628 / 89 | One valid `web_fetch` | Correct answer; 14,067-character result. |
| Candidate compact 1 | 872 / 105 | One valid `web_fetch` | Correct answer cites the official section and URL. |
| Candidate compact 2 | 872 / 105 | One valid `web_fetch` | Correct answer; exact result hash repeated. |

All calls fetched `https://lmstudio.ai/docs/developer/core/authentication`,
returned HTTP 200, and had stable document ID `fetch_de9d81a67951e3de`. The
candidate compact result was 3,713 serialized characters versus 14,067 for
full; the permission-edit sentence was present in both. Candidate compact used
46.4% fewer input tokens and 43.1% fewer total input-plus-output tokens than
candidate full. The compact tools list was 19,370 serialized bytes, below the
existing budget. The saved baseline also passed both compact turns, so these
trials do not prove the description change caused the successful tool calls.
The earlier two no-call failures remain recorded in M003 §11; M003 §12 records
the controlled follow-up and its limits.

The original `~/.lmstudio/mcp.json` was restored byte-for-byte after the
experiment and verified at SHA-256
`b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c`.
Authentication and MCP permission settings were not changed.

## 3. Verification

| Check | Result |
|---|---|
| `cargo fmt --check` | pass |
| `cargo test --locked --all-features --test mcp_schema_slimming` | pass, 18 tests |
| `python3 scripts/lm_studio_stdio_smoke.py target/release/eggsearch` | pass; ten tools, 72,923 full / 19,370 compact bytes; `provider_status` succeeds |
| `make check` | pass locally; 3,486 tests |
| `make docs-check` | pass locally |
| `make bench-check` | pass; optimized Criterion harness compiled with `--no-run` |
| Hosted Linux, macOS, and Windows CI | pending on the committed candidate |

## 4. Disposition

The candidate passes the controlled direct-fetch task gate. The baseline
comparison does not establish that the added description alone changes model
behavior; it clarifies the intended supported URL source without changing
tool behavior or breaching the compact schema budget. M006 can close after
`make bench-check` and hosted CI pass.

M003 remains blocked on the broader task-quality matrix and LM Studio UI
refresh and result-shape behavior. The additional Gemma E2B/E4B results and
the failed UI automation attempt are recorded in
`plans/closure/standalone-mcp-local-inference/003-status.md` §12.
