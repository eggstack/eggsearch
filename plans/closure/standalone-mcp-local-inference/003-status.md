# Standalone MCP Local-Inference M003 — Operational Status

Status: blocked (the live UI and authenticated native API paths work, but M003's host-contract characterization, broader paired task matrix, and documented quality failures remain unresolved)

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/003-lm-studio-consumer-qualification-and-docs.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

Final implementation candidate exercised by the installed-child smoke and native API: `de69f1ddd7a7e4acc067fb9892c438f6165cfe83` (`eggsearch 0.4.4`; M004/M005 changes included). Earlier interactive and model-turn evidence remains separately identified below.
Predecessor closures: M001 `d983fa5`; M002 `plans/closure/standalone-mcp-local-inference/002-status.md`.

## 1. Current finding

M003 is not closed. The installed-candidate stdio smoke, interactive LM
Studio chat path, and authenticated native `/api/v1/chat` path worked with the
candidate, and all ten tools remained available. The paired direct-inference
trials included `google/gemma-4-e2b`, `prism-ml/bonsai-27b`, and
`lfm3-2b-omni-abliter8`; the required same-task comparison with a larger
reference model remains incomplete. Existing evidence does not establish
that compact is a safe default: specialist research and direct-fetch tasks
had material quality failures, and several runs timed out or exhausted the
available context.

The final candidate's installed-child stdio smoke passed on 2026-10-31:
full exposed all ten tools in 72,923 serialized `tools/list` bytes, compact
exposed the same ten in 19,281 bytes, and `provider_status` succeeded in both.
These byte counts do not estimate LM Studio tokenizer usage.

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
| Ten tools and profile list sizes | Final-candidate initialized child-process smoke: 72,923 full / 19,281 compact bytes; all ten names; `provider_status` with probing off | pass | Byte counts are not token counts |
| LM Studio tool visibility and interactive tool use | Prior full/compact qualification chats called `web_search`; compact also called `security_search`, `research_search`, and `web_fetch`. The app rendered a `web_fetch mcp/eggsearch` result card. | partial | Verify refresh/new-chat behavior against the final candidate; determine whether the app consumes structured content, text fallback, or both |
| Paired small/larger model comparison | Gemma E2B and Bonsai 27B received the same extraction task; Bonsai full timed out while compact completed. Additional same-task pairs are recorded for Gemma E2B and LFM3 2B. | partial | Complete specialist/research and fetch pairs on the larger model; interpret timeouts as failed trials |
| Tool choice, argument validity, repairs, evidence and failure truthfulness | Five task records above | partial / quality concern | Investigate research argument retries, context overrun, and source classification; add paired specialist/fetch cases |
| Exact tokenizer and prompt-token attribution | Native API reports 1,575 input / 113 output tokens for one configured-server call; UI still exposes only context-meter readings and no tokenizer identity | partial | Capture same-task API usage across both profiles and characterize UI attribution |
| Output schema / structuredContent vs text fallback | Offline protocol coverage plus a successful rendered UI tool-result card and native API tool call | partial | Isolate which result representation the UI consumes; compare a generic MCP inspector/client if available |
| Protected API MCP smoke | Authenticated native `/api/v1/chat` call through configured `mcp/eggsearch`, with only `web_fetch` allowed | pass | UI structured-content/text fallback and refresh behavior remain uncharacterized |
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
   latency. The post-M005 Bonsai security retry timed out without a result;
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
4. The authenticated native configured-server API smoke now passes; see §10.
   The user enabled authentication and “Allow calling servers from mcp.json.”
   UI structured-content/text fallback and refresh behavior remain untested.
5. M004 and M005 hosted Linux/macOS/Windows canonical gates pass on final
   candidate `de69f1ddd7a7e4acc067fb9892c438f6165cfe83` in CI run
   `41160264796`; see their closure records. The
   earlier local `bounded_command` stall did not reproduce in hosted CI.
6. M003 remains blocked on broader domain coverage, LM Studio UI
   structured-content/refresh behavior, and remediation or explicit
   acceptance of the quality failures in §7. Close M003 only after its
   remaining live acceptance criteria pass.

## 6. Security and operational disposition

The agent did not change API authentication, credentials, or MCP permission
settings. The user enabled authentication and configured-server access after
the earlier tests. The temporary global MCP config was restored and verified
byte-identical to its pre-test backup
(`b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c`). The
live UI and native API paths work, but the larger-model task matrix, host
refresh/structured-content behavior, and quality decision remain open. Do not
claim a general LM Studio token-saving or quality win from these limited
paired trials.

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
and broader domain coverage remain open. The protected API smoke is later
qualified in §10. Keep M003 blocked until remaining criteria pass or are
explicitly re-scoped by a future plan.

## 8. LFM3-2B paired search and extraction trials

The user authorized additional trials with `lfm3-2b-omni-abliter8`. LM Studio
reported it loaded as a 2B Q4_K_M GGUF model with tool-use capability and a
128,000-token context. As with §7, the model received tool schemas from the
branch MCP stdio server through the OpenAI-compatible `/v1/chat/completions`
endpoint; the harness executed tool calls through eggsearch and returned the
MCP result. This measures local model turns and token usage, but is not LM
Studio's native MCP API integration.

### Search pair

Both profiles received the same request to find the official Rust 1.90.0
announcement, report two concrete changes, cite evidence and URLs, and
distinguish the official source. Each profile first called `web_search` with
`excerpt_count=5`, which runtime rejected because the maximum is 3. The model
recovered with a second call using `excerpt_count=3` and completed its answer.

| Profile | Actual usage (prompt + completion) | Calls / time | Result |
|---|---:|---|---|
| Full / diagnostic | 14,563 + 470 = 15,033 tokens | Two calls; 21.52 seconds | Correctly identified the Rust Blog as the official announcement, but cited `releases.rs` and Mastodon for the two changes, then mislabeled the official Rust Language Mastodon account as community discussion. Evidence/source classification was partial. |
| Compact / brief | 8,718 + 352 = 9,070 tokens | Two calls; 13.15 seconds | Correctly named both changes and the official Rust Blog, but attributed the workspace-publishing claim to the `releases.rs` result, whose returned snippet did not establish that claim. Evidence attribution failed. |

The successful full search result reported 10 DuckDuckGo results, zero
Startpage results, and a Yahoo HTTP 500. The compact retry returned results
without a provider failure. Both outputs therefore also show why provider
failure evidence must be interpreted from each actual result rather than
carried across paired runs.

### Direct-fetch pair

Both profiles received the same official Rust 1.90.0 URL and request for two
changes with supporting evidence. Both made one valid `web_fetch` call and
identified the LLD default change and Cargo workspace publishing. Each cited
the official announcement URL. The text itself was not marked retrieval
truncated in either result.

| Profile | Fetch arguments | Actual usage (prompt + completion) | Result |
|---|---|---:|---|
| Full / diagnostic | Exact URL, `extract_mode=text`, `max_chars=2000` | 8,816 + 495 = 9,311 tokens; 11.56 seconds | Correct changes and official URL; grounded both in excerpts from the fetched announcement. |
| Compact / brief | Exact URL and `extract_mode=text` | 4,962 + 418 = 5,380 tokens; 7.73 seconds | Correct changes and official URL with direct excerpts. Fetched text was 4,498 characters, over 800; `output_truncated=true` applied to outer projection metadata, while `truncated=false` for the fetched document. |

These direct-fetch answers are a positive result for LFM3 on this task, not a
general quality claim. Relative to full, compact used 3,931 fewer total
prompt-plus-completion tokens (about 42.2%) in this one paired trial. This
comparison is the local API's reported usage; it is separate from serialized
MCP byte counts and from the LM Studio UI context meter.

### Native MCP API route check

The local server was started on eggsearch's documented loopback-only
Streamable HTTP endpoint (`127.0.0.1:11320/mcp`). LM Studio's native REST
`/api/v1/chat` with an `ephemeral_mcp` integration rejected that URL because
dynamic remote MCP integrations allow only public addresses. No tunnel or
public exposure was introduced.

The LM Studio settings page showed “Allow per-request MCPs” on, “Require
Authentication” off, and “Allow calling servers from mcp.json” disabled. The
last setting requires authentication. The configured-server path is therefore
not qualified; enabling it and creating an API token would change
security-sensitive settings and require explicit operator participation.
No such setting or credential was changed. The loopback server was stopped
after the request.

## 9. Bearer-token probe with authentication disabled

At the user's request, a loopback-only `POST /api/v1/chat` request included
the supplied bearer token and a temporary configured-server alias
`mcp/eggsearch-test`. It returned HTTP 403 with LM Studio's `Permission denied
to use plugin` error. At the time of the probe, `Require Authentication` and
`Allow calling servers from mcp.json` were both off; neither setting was
changed. A plain authenticated inference request succeeded. The token value
is intentionally not recorded here.

The 403 was specific to the temporary alias and disabled configured-server
permission, rather than evidence that the token lacked MCP permission. The
follow-up in §10 tests the actual `mcp/eggsearch` entry after the user enabled
authentication and configured-server access. LM Studio requires an API token
for every request when authentication is enabled; the user enabled this
setting after the earlier probe.

## 10. Authenticated native MCP API acceptance

After the user enabled authentication and “Allow calling servers from
mcp.json,” a loopback-only `POST http://127.0.0.1:1234/api/v1/chat` request
used the actual `mcp/eggsearch` integration from the restored LM Studio
configuration. LM Studio 0.4.26+4 ran `lfm3-2b-omni-abliter8` (Q4_K_M,
128,000-token model context). No token value is recorded.

The request restricted the integration to `web_fetch` and asked the model to
read LM Studio's official authentication documentation. The native API
returned HTTP 200, emitted one valid `web_fetch` call for the requested URL,
and returned a correct answer identifying the Authentication page and
explaining that an existing token's permissions can be edited in the API
Tokens modal. LM Studio reported 1,575 input tokens and 113 output tokens;
the local request took about six seconds. Eggsearch returned a successful
fetch of the official page. The request used a 24,000-token context setting.

A first request exposing the full Eggsearch tool palette at a 12,000-token
context setting failed before inference because the resolved prompt required
18,737 tokens against an available 12,032. Raising context to 32,768 allowed
the native request to run. Restricting `allowed_tools` to the single required
tool therefore produced a much smaller measured prompt (1,575 input tokens)
and successfully completed the retrieval. These are API-reported usage
figures for this request, not a general quality or token-saving claim.

The user enabled the server authentication and configured-server settings;
the agent did not change them. The original `~/.lmstudio/mcp.json` was
restored byte-for-byte after the temporary test alias was removed. The
protected native MCP API criterion is now passed. Interactive UI
structured-content versus text fallback, config refresh behavior, broader
paired-task quality, and the documented negative model outcomes remain open,
so M003 is still blocked.
