# Standalone MCP Local-Inference M005 — Corrective Closure

Status: closed (brief extraction preserves the paired document; compact security goal values are discoverable; final hosted CI passes)

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/005-brief-document-extraction-and-paired-requalification.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

Qualified candidate: `de69f1ddd7a7e4acc067fb9892c438f6165cfe83`.
This corrective preserves the historical M002 closure and leaves the broader
M003 LM Studio acceptance status open; its exact model records are in
`plans/closure/standalone-mcp-local-inference/003-status.md`.

## 1. Brief extraction correction

`src/mcp/projection.rs::project_brief` no longer clips every string at 800
characters before applying the configured response budget. It keeps the
original field text when the serialized response fits and progressively
reduces text only when necessary. A deterministic regression projects the
same long extracted document and query in diagnostic and brief modes and
checks equality when the brief response fits.

The live paired qualification used the official Rust 1.90.0 announcement and
the identical question, “What two concrete Rust 1.90.0 changes are stated on
this page?” The diagnostic and brief document projections were each 4,498
characters and byte-for-byte equal. Both retrieval results marked the fetched
document untruncated, and the document text exceeded 800 characters. Brief
projection did mark outer warning metadata as output-truncated; it did not
clip the document text. This behavior is recorded separately from retrieval
truncation.

## 2. Paired local-model evidence

The real local model trials, including actual per-request usage, call
arguments, evidence completeness, latency, and failure interpretation, are
recorded in §7 of M003 status. They used LM Studio 0.4.26+4 and its local
OpenAI-compatible inference API with tool definitions fetched from the branch
MCP stdio server. The harness executed model-returned calls through that
server. It did not exercise LM Studio chat UI's native MCP result handling or
the protected API permission path.

The initial Bonsai 27B security call used an unsupported free-form
`goal="assess_applicability"`. M005 therefore adds the runtime's canonical
`SECURITY_GOAL_VALUES` enum to compact `security_search` and a test asserting
that the compact advertised values match that source of truth. On a post-fix
repeat, Bonsai's LM Studio request timed out after 300 seconds without usage,
a tool call, or a final answer. The timeout is a recorded failure, not a pass.

M003 stays open because the matrix found a blank full-profile Gemma answer, a
Gemma security answer that overstated the authority of generic web evidence,
the invalid and timed-out Bonsai security attempts, and a Bonsai full-profile
timeout. The post-fix compact extraction was successful for both models.

## 3. Verification

- `cargo fmt --check` — pass.
- `cargo test --locked --all-features --test mcp_schema_slimming` — 17 passed.
- `cargo test --locked --all-features --test mcp_projection` — 22 passed.
- `cargo build --locked --release --all-features` — pass.
- `python3 scripts/lm_studio_stdio_smoke.py target/release/eggsearch` — full
  and compact profiles both passed; ten tools and `provider_status` succeeded.
  Serialized `tools/list` sizes were 72,923 bytes full and 19,281 bytes
  compact.
- Hosted final-candidate CI run
  [`41160264796`](https://github.com/eggstack/eggsearch/actions/runs/41160264796)
  — all jobs passed: Linux canonical `make ci`, macOS canonical `make ci` plus
  local-filesystem portability, and Windows all-features portability. The
  process-group/cap-breach stall previously seen on the local macOS host did
  not reproduce.
- PR #15 checks — 47 passed, zero failed. The separate egress, Python native,
  and release drift workflows also passed on the same candidate.
- `~/.lmstudio/mcp.json` was restored from its saved original and verified
  byte-for-byte: both original and restored SHA-256 are
  `b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c`.
  No LM Studio API authentication or MCP permission setting was changed.

## 4. Disposition

M005 acceptance is satisfied: brief extraction retains the full document when
it fits, the paired projections use the same query/document, the local-model
successes and failures are recorded with actual usage where available, the
schema regression is tested, the original config is restored, and the exact
candidate passed hosted Linux, macOS, and Windows checks. M004's hosted gate is
also closed on this candidate. M003 remains blocked on its listed operational
and quality criteria; this corrective does not claim a general token-saving
or model-quality win.
