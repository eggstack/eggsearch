# Standalone MCP Local-Inference M003 — Operational Status

Status: blocked (live LM Studio interaction is qualified, but the acceptance matrix is incomplete and Gemma quality failures require a corrective decision)

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/003-lm-studio-consumer-qualification-and-docs.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

Implementation candidate exercised: `02a8af6fe4f87971c7daca7d1fca20c087b328ec`.
Predecessor closures: M001 `d983fa5`; M002 `plans/closure/standalone-mcp-local-inference/002-status.md`.

## 1. Current finding

M003 is not closed. The installed-candidate stdio smoke and the interactive
LM Studio chat path both worked with the candidate, and all ten tools remained
available. This is the first real model-turn evidence for the branch. It does
not establish that compact is a safe default: one paired web task showed a
large context-meter reduction, but specialist research and direct-fetch tasks
had material quality failures. The tests used only `google/gemma-4-e2b` in
LM Studio 0.4.26+4, so the required larger-reference comparison remains open.

Model/server facts observed in LM Studio:

- Model identifier: `google/gemma-4-e2b`, MLX 4-bit, 4.37 GB on disk.
- Context limit shown by the app: 10,703 tokens (above the plan's 8k minimum).
- Model advertised tool-use capability. A per-request tokenizer identity and
  exact per-chat prompt-token usage were not exposed in the app chat UI.
- The local server was running at `http://127.0.0.1:1234`.
- Temporary LM Studio config selected the branch release binary and compact
  profile. The original `~/.lmstudio/mcp.json` was restored and byte-compared
  with its backup after the tests. No API authentication or server permission
  setting was changed.

## 2. Live task evidence

All chats were fresh LM Studio conversations. Tool approval was granted once
per call; the persistent “Always allow” option was not selected.

| Profile/task | Tool behavior | Result quality | Observed context / timing |
|---|---|---|---|
| Full: official Rust 1.90.0 announcement | Selected `web_search`; valid arguments; one call; no repair | Correct official title, URL, and source classification | Context meter 96.1% of 10,703; total latency not captured |
| Compact: same official announcement task | Selected `web_search`; valid arguments; one call; no repair | Same correct title, URL, and source classification | Context meter 51.9% of 10,703. Paired UI-meter difference: 44.2 percentage points (about 4,730 context slots); this is a single-task observation, not a general token-savings claim. Model thought durations were 12.79 s before and 9.91 s after the tool call; end-to-end latency was not captured. |
| Compact: Log4j 2.14.1 / CVE-2021-44228 | Selected `security_search`; valid applicability arguments; one call | Correctly reported affected status and CVE; accurately retained Yahoo HTTP 500 provider failures, untrusted-result warning, and no severity inference | Context meter 44.3%; end-to-end latency not captured |
| Compact: two concrete Rust 1.90.0 changes with evidence | Four `research_search` calls (three argument failures, one partial search); one irrelevant current release-index `web_fetch`; one `web_search` argument failure followed by a valid retry | Final answer misclassified secondary `releases.rs` as official release documentation and gave weakly supported claims. The first two schema errors used invalid research-domain/goal values; the web-search retry corrected an invalid safe-search value. | Context meter reached 109% of 10,703; task overran the available context. Latency not captured. |
| Compact: fetch the official Rust 1.90.0 announcement and name two changes | Selected `web_fetch`; valid URL; one call | Fetch succeeded and LLD-default change was correctly identified. The second item was an update command, not a concrete release change, so the requested two-change extraction failed. | Context meter 31.3%; model thought durations 5.84 s before and 12.12 s after fetch; end-to-end latency not captured. |

The paired web search is evidence that this LM Studio interactive path used the
two profiles differently. Its task and model were paired, but there was only
one prompt pair and the UI context meter is not an independently tokenized
schema/result attribution. No exact tokenizer counts were captured. The model
server log showed aggregate generation and an unrelated request; it did not
provide a reliable per-chat usage breakdown for this test set.

## 3. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Remaining work |
|---|---|---|---|
| Copyable installed-binary `mcp.json` setup | `docs/lm-studio.md`; config temporarily pointed at branch release binary | pass | None for configuration syntax |
| Windows/macOS/Linux paths and stdio logging contract | `docs/lm-studio.md` | pass | None for documentation |
| Reproducible installed-candidate stdio smoke | `scripts/lm_studio_stdio_smoke.py` | pass | Offline protocol evidence only |
| Ten tools and profile list sizes | Real initialized child-process smoke: 72,923 full / 18,545 compact bytes; all ten names; `provider_status` with probing off | pass | Measure model-tokenized attribution |
| LM Studio tool visibility and interactive tool use | Full and compact chats each called `web_search`; compact also called `security_search`, `research_search`, and `web_fetch` | partial | Add more paired tasks and verify refresh/new-chat behavior explicitly |
| Paired small/larger model comparison | One full/compact pair on Gemma E2B only | blocked | Run the same task matrix on a larger local reference model |
| Tool choice, argument validity, repairs, evidence and failure truthfulness | Five task records above | partial / quality concern | Investigate research argument retries, context overrun, and source classification; add paired specialist/fetch cases |
| Exact tokenizer and prompt-token attribution | App context meter only; no per-chat token counts or tokenizer identifier | blocked | Capture host-injected prompt and response usage for each profile |
| Output schema / structuredContent vs text fallback | Offline protocol coverage only | partial | Inspect actual LM Studio handling and compare a generic MCP inspector/client |
| Protected API MCP smoke | Previous API call was permission-denied; “Allow calling servers from mcp.json” remained disabled | blocked | Operator must configure LM Studio authentication and authorize that server permission before API qualification; no such security settings were changed |
| Config rollback | Restored original `~/.lmstudio/mcp.json`; `cmp` against backup succeeded | pass | None |

## 4. Work completed

`docs/lm-studio.md` documents the opt-in compact/brief TOML, absolute-path
`mcp.json` entry, Windows escaping, full-profile fallback, stdio logging,
refresh behavior, API `allowed_tools`, protected API setting, and limits of
byte-based estimates. `scripts/lm_studio_stdio_smoke.py` provides a local,
network-free handshake/list/call check. Context-efficiency records actual
serialized bytes and keeps those separate from model-token estimates.

The offline smoke passed for both profiles with ten tools and successful
`provider_status`. Interactive tests demonstrated that Gemma can select and
call core and specialist tools, but also recorded argument repair, context
overrun, weak source classification, and an incomplete extraction. These
negative findings rule out closure. M004 is registered to expose runtime-
accepted research goal/domain values in compact schemas and repeat the focused
qualification.

## 5. Remaining evidence to unblock and close

1. Run the same paired full+diagnostic and compact+brief prompts on Gemma E2B
   and a larger reference model, including security, research, page/PDF fetch,
   repo/evidence, and grouped-failure tasks. Capture tool choice, argument
   validity, repairs, evidence, failure interpretation, calls, total latency,
   prompt processing time, and context usage.
2. Capture exact tokenizer identity and host-injected prompt/completion token
   counts for each paired task. Keep those distinct from context-meter
   percentages and serialized JSON byte counts.
3. Characterize LM Studio's structured output and text fallback handling,
   visibility after config refresh/new chat, and compare one generic MCP
   inspector/client.
4. For API-specific qualification, the LM Studio server requires
   authentication before enabling “Allow calling servers from mcp.json.” No
   credentials were created and no security-sensitive setting was changed;
   complete that prerequisite in LM Studio, then run the API smoke and restore
   prior settings.
5. M004 is implemented in source at candidate
   `776f8ca4a8246e467041696b2220600eb7b2648d` to expose accepted research goal/domain
   values in compact schemas. Focused contract tests, all-features compile,
   release build, and installed-child smoke pass. The release binary advertised
   all ten tools at 18,990 compact schema bytes. A post-fix Gemma turn was not
   completed: LM Studio's exposed accessibility state and visible window
   diverged during integration refresh, so results from the previous child
   were not attributed to the new candidate. See
   `plans/closure/standalone-mcp-local-inference/004-status.md`.
6. Run the focused live tasks against the corrected exact candidate, then
   update this status with the result; close M003 only after its full live
   acceptance criteria pass.

## 6. Security and operational disposition

No API authentication, credential, or MCP permission setting was changed.
The temporary global MCP config was restored and verified byte-identical to
its pre-test backup. LM Studio interactive chat qualification is now recorded,
so the remaining block is broader than the earlier API permission denial:
model comparison, exact token attribution, host behavior checks, and the
substantial live quality failures remain open. Do not claim a general LM
Studio token-saving or quality win from the single paired task.
