# Standalone MCP Local-Inference M003 — Operational Status

Status: blocked (live LM Studio consumer qualification not complete)

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/003-lm-studio-consumer-qualification-and-docs.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

Implementation candidate reviewed: `02a8af6fe4f87971c7daca7d1fca20c087b328ec`.
Predecessor closures: M001 `d983fa5`; M002 `plans/closure/standalone-mcp-local-inference/002-status.md`.

## 1. Current finding

M003 is not closed. The onboarding guide and repeatable offline installed-
binary stdio smoke are implemented. The smoke initializes the actual Eggsearch
child, confirms ten names in each profile, measures the serialized `tools/list`
array (72,923 full / 18,545 compact bytes), calls `provider_status` with
probing disabled, compares structured results and exits successfully. This is
protocol and executable evidence, not an LM Studio model-turn measurement.

The local host is LM Studio 0.4.26+4 with `google/gemma-4-e2b` available. An
API chat request addressing `mcp/eggsearch` returned: `Permission denied to use
plugin 'mcp/eggsearch'. Ensure that the server configuration allows plugin
usage and, if using an API token, it has the necessary permissions.` The
LM Studio setting **Allow calling servers from mcp.json** is currently not
enabled. No setting was changed. No interactive model-visible token count,
paired task result, repair count, latency comparison, or larger-model
reference run has been recorded.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Remaining work |
|---|---|---|---|
| Copyable installed-binary `mcp.json` setup | `docs/lm-studio.md` | pass | Confirm with a live LM Studio chat |
| Windows/macOS/Linux path guidance and stdio logging contract | `docs/lm-studio.md` | pass | None for documentation |
| Reproducible installed-candidate stdio smoke | `scripts/lm_studio_stdio_smoke.py` | pass | Live host integration still required |
| Ten tools and profile list sizes | Real initialized child-process smoke | pass | Measure actual LM Studio prompt inclusion/tokens |
| Interactive small and larger model comparison | No model-turn records | blocked | Run paired prompt set with actual models |
| Tool choice, arguments, repairs, evidence truthfulness, latency | No live task records | blocked | Record per-plan measurements for web, security, research, fetch and failures |
| OutputSchema/structuredContent/text fallback and refresh behavior | Offline protocol coverage only | partial | Observe LM Studio behavior across sessions/config refresh |
| Protected API MCP smoke | Permission-denied API response | blocked | Enable the named LM Studio server setting and validate API authorization |

## 3. Work completed

`docs/lm-studio.md` documents the opt-in compact/brief TOML, absolute-path
`mcp.json` entry, Windows escaping, full-profile fallback, stdio logging,
refresh behavior, API `allowed_tools`, the protected API setting and the limits
of byte-based estimates. `scripts/lm_studio_stdio_smoke.py` provides a local,
network-free handshake/list/call check. Context-efficiency records actual
serialized bytes and clearly separates these from model-token estimates.

## 4. Required evidence to unblock and close

1. With explicit operator authorization, enable **Allow calling servers from
   mcp.json** for the LM Studio API, use authenticated local access, run the
   configured MCP smoke, then restore the setting to its prior state if it was
   enabled only for this qualification.
2. Run paired full+diagnostic and compact+brief tasks on a smaller
   tool-capable model and a larger reference model, with at least 8k context
   where available. Include `security_search` and `research_search` tasks.
3. Record exact model identifiers, tokenizer, context length, host-injected
   prompt tokens, tool-choice and argument success, repairs/retries, evidence
   found, truthful failure interpretation, calls, latency and prompt
   processing time. Separate client `allowed_tools` effects from profile
   effects.
4. Observe output-schema handling, structured content vs text fallback,
   refresh/new-chat visibility, and rollback in the actual LM Studio host.
5. Update this record with measured results and a new exact implementation
   candidate SHA; close only if the plan's live acceptance criteria pass.

## 5. Security and operational disposition

No API setting or MCP permission was changed during this work. Enabling that
setting expands API clients' ability to invoke locally configured MCP tools
and requires explicit action-time authorization. Until the live gate is
authorized and completed, M003 remains blocked and no LM Studio context or
quality improvement is claimed.
