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

1. Initial paired full+diagnostic and compact+brief API trials on Gemma E2B
   and Bonsai 27B, including extraction and security failure interpretation,
   are recorded in §7 with tool validity, evidence quality, actual usage, and
   latency. Repeat Bonsai's security task after M005 adds accepted goal values;
   expand the task set to research, PDF, repository/evidence, and grouped
   failures before closing M003.
2. Exact prompt/completion usage is now recorded for completed requests in §7.
   LM Studio did not return counters for the Bonsai full-profile timeout, and
   no tokenizer identity was exposed. Keep usage, context capacity, and
   serialized bytes separate; additional prompt/token attribution is still
   required for the broader acceptance matrix.
3. Characterize LM Studio's structured output and text fallback handling,
   visibility after config refresh/new chat, and compare one generic MCP
   inspector/client.
4. For API-specific qualification, the LM Studio server requires
   authentication before enabling “Allow calling servers from mcp.json.” No
   credentials were created and no security-sensitive setting was changed;
   complete that prerequisite in LM Studio, then run the API smoke and restore
   prior settings.
5. M004's hosted Linux/macOS/Windows canonical gate now passes on candidate
   `755008c0a1da658835dd8f215d3b82bd1cb3dee5`; see its closure status. The
   earlier local `bounded_command` stall did not reproduce in hosted CI.
6. M003 remains blocked on the post-correction Bonsai retry, broader domain
   coverage, LM Studio UI structured-content/refresh behavior, protected API
   smoke, and remediation or explicit acceptance of the quality failures in
   §7. Close M003 only after its remaining live acceptance criteria pass.

## 6. Security and operational disposition

No API authentication, credential, or MCP permission setting was changed.
The temporary global MCP config was restored and verified byte-identical to
its pre-test backup. LM Studio interactive chat qualification is now recorded,
so the remaining block is broader than the earlier API permission denial:
model comparison, exact token attribution, host behavior checks, and the
substantial live quality failures remain open. Do not claim a general LM
Studio token-saving or quality win from the single paired task.

## 7. Paired local inference API measurements (M005)

The later M005 trial used LM Studio's local OpenAI-compatible inference API
with the compact/full tool definitions obtained from the branch MCP stdio
server. The harness supplied the definitions to the model, executed the
returned MCP call against the local server, and returned the tool result to the
model. These are real model turns with per-request usage counters, but they
are not the LM Studio chat UI's native MCP integration and do not satisfy its
protected API permission check. The trials used LM Studio 0.4.26+4 and the
already loaded models `google/gemma-4-e2b` (MLX 4-bit, 10,703-token loaded
context) and `prism-ml/bonsai-27b` (MLX 2-bit, 262,144-token loaded context).

### Paired Rust 1.90.0 document extraction

Both profiles received the same question and official Rust 1.90.0 release
announcement URL. The branch MCP server fetched the document once and served
the cached same document to the paired request. Its diagnostic and brief
projections were both 4,498 characters, byte-for-byte equal; retrieval marked
neither document truncated. The brief response's outer warning metadata was
clipped, recorded as `output_truncated=true`, while the extracted document
text itself remained intact and longer than 800 characters.

| Model | Profile | Tool call | Actual usage (prompt + completion) | Result |
|---|---|---|---:|---|
| Gemma E2B | full / diagnostic | One valid `web_fetch`, exact URL and query focus | 9,273 + 1,319 = 10,592 tokens across the tool-call and answer requests | Correct first change, then blank final answer at the 10,703-token loaded-context ceiling; quality fail. |
| Gemma E2B | compact / brief | One valid `web_fetch`, same URL and query focus | 6,172 + 798 = 6,970 tokens | Correctly named LLD as the default linker for `x86_64-unknown-linux-gnu` and Cargo's native `cargo publish --workspace`; evidence complete. |
| Bonsai 27B | full / diagnostic | No completed tool turn | Request timed out at 240 seconds | Fail; no usage counter returned. |
| Bonsai 27B | compact / brief | One valid `web_fetch`, same URL and query focus | 13,217 + 750 = 13,967 tokens | Correctly named both release changes with an official-page citation; 157.4 seconds. |

The compact schema bytes in the direct inference requests were 18,718 versus
68,736 for full. The initialized MCP stdio smoke separately measured 18,990
versus 72,923 serialized `tools/list` bytes. Do not conflate schema bytes with
the actual usage counts above.

### Paired failure interpretation

Both models received the same compact-profile Log4j 2.14.1 / CVE-2021-44228
applicability prompt. The actual `security_search` result recorded 12 provider
attempts, 6 completions, 3 failures, no absence markers, and an HTTP 500 from
Yahoo; no native advisory provider was available, and the returned evidence
was generic web material.

| Model | Tool-call validity | Actual usage (prompt + completion) | Interpretation |
|---|---|---:|---|
| Gemma E2B | One valid `security_search` call; no runtime error | 6,403 + 900 = 7,303 tokens over two inference requests; 25.31 seconds | Correctly said Yahoo's 500 does not prove absence, but overstated generic web pages as authoritative advisories and omitted the untrusted-source and unavailable-severity warnings. Partial, with evidence-trust failure. |
| Bonsai 27B | Called `security_search` with unsupported `goal="assess_applicability"`; runtime rejected it and the following repair attempt was not executed by the one-call harness | 11,280 + 367 = 11,647 tokens; 77.6 seconds | No usable final answer; invalid tool arguments and failure-interpretation task failed. |

The Bonsai call exposed that compact `security_search` listed `goal` without
its accepted values. M005 adds the runtime's canonical security-goal enum and
a schema contract test. The post-correction Bonsai retry was run against that
schema, but the LM Studio request timed out after 300 seconds before returning
a tool call, final answer, or usage counters. This is recorded as a failed
retry, not as a pass; the initial invalid call remains in the table above.

### Remaining M003 acceptance

The paired model matrix now has exact prompt/completion usage for successful
requests and explicit timeouts where the host returned no counter, including
the 300-second post-correction Bonsai retry. It still
does not pass M003: the full Gemma extraction ended blank, Gemma's security
answer misstated evidence authority, Bonsai's security call failed argument
validation, and Bonsai full-profile inference timed out. Native LM Studio UI
tool-result handling and refresh behavior, structured-content/text fallback,
and the protected API smoke also remain open. No API authentication or MCP
permission setting was changed. Keep M003 blocked until these criteria pass or
are explicitly re-scoped by a future plan.
