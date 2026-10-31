# M005 — Brief Document Extraction and Paired Requalification

Status: closed

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

Corrects M002's brief projection after M003/M004 live qualification found that
the unconditional 800-character per-string clip hid most fetched document text.
The existing M002 closure remains historical and unchanged. This plan also
records the paired M003 acceptance and finishes M004's hosted canonical gate.
The initial Bonsai 27B paired failure-interpretation trial also exposed that
compact `security_search` advertised a free-form goal despite runtime accepting
only canonical values; this plan now includes making those values discoverable
and repeating that call.

## Objective

Preserve all fetched document text that fits the configured brief response
budget, and requalify diagnostic versus brief responses using the same query
and same extracted document. Complete paired interactive LM Studio tasks on
`google/gemma-4-e2b` and the larger installed `prism-ml/bonsai-27b`, measuring
tool-call validity, evidence completeness, failure interpretation, and actual
token use wherever the host exposes counters. Run the canonical `make ci` gate
on hosted Linux and macOS for the final candidate.

## Explicit non-goals

- Reopen or amend the accepted M001 or M002 closure records.
- Change retrieval, extraction, provider, trust, or stable-ID semantics.
- Claim exact prompt/completion token attribution from a rounded context meter
  or serialized byte counts.
- Enable LM Studio API authentication or permission settings.
- Add network-dependent model qualification to routine CI.

## Current evidence and defect

`src/mcp/projection.rs::project_brief` currently clips every `text`, `snippet`,
`message`, and `error` value to 800 characters before checking the configured
8,000-character aggregate budget. A fetched document can therefore be marked
untruncated by retrieval while brief projection silently drops most of its
extract. M003 and M004 live records already contain the same Rust 1.90.0
announcement query and document URL; use those as the paired qualification
case and add a deterministic regression fixture.

## Work packages

1. Change the brief reducer to preserve content while the serialized response
   fits its configured limit, and reduce field text progressively only when
   the response exceeds that limit. Keep truncation and omission indicators
   accurate.
2. Add a deterministic regression that projects one document/query into both
   diagnostic and brief, proves the text is identical when under budget, and
   proves brief text exceeds 800 characters without setting a truncation flag.
3. Run the same Rust 1.90.0 extraction query on the same official URL with
   diagnostic and brief output, on Gemma E2B and Bonsai 27B. Also repeat the
   research/schema and provider-failure interpretation tasks on both models.
   Record each tool call, argument validity, repairs, cited evidence,
   completeness, failure interpretation, latency, and host token/context data.
4. Keep full/diagnostic and compact/brief MCP configs isolated per run and
   restore the original LM Studio `mcp.json` byte-for-byte.
5. Enable manual workflow dispatch for the CI workflow and run the canonical
   `make ci` suite on hosted Linux and macOS. Record whether the prior local
   process-group stall reproduces. Do not treat a timeout as a passing gate.
6. Advertise the canonical runtime-accepted security goals in the compact
   `security_search` schema, add a schema contract assertion, and repeat the
   larger-model failure-interpretation trial to check for a valid call.
7. Update M003/M004 statuses only with exact candidate evidence, this plan's
   closure, the roadmap, and the registry in the same closure commit.

## Verification

```bash
cargo fmt --check
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test mcp_tool_contract
cargo build --locked --release --all-features
python3 scripts/lm_studio_stdio_smoke.py target/release/eggsearch
make ci
```

Hosted macOS/Linux `make ci` runs are required for M004. Interactive LM Studio
qualification is operational and must not be added to routine CI.

## Acceptance

- A document longer than 800 characters remains unchanged in brief output when
  the response fits its configured budget.
- Diagnostic and brief comparisons use the identical query and fetched
  document; all projection truncation is explicitly surfaced.
- Both models complete paired tasks with evidence and failures accurately
  interpreted, or each remaining deficiency is explicitly recorded and M003
  remains open.
- Exact model token counters are recorded only if LM Studio exposes them; UI
  context percentage, context capacity, latency, and serialized bytes are
  reported as distinct measurements.
- Hosted Linux and macOS canonical runs are green on the final implementation
  SHA for M004 closure.
- The original LM Studio MCP config is restored and verified byte-for-byte.

Closure evidence belongs in
`plans/closure/standalone-mcp-local-inference/005-status.md` and must link the
exact source SHA, hosted runs, LM Studio version/config, task records, test
outcomes, restored config check, and any unresolved M003 criterion.
