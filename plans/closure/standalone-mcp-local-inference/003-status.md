# Standalone MCP Local-Inference M003 — Operational Status

Status: active (scope clarified: qualify small-model tool use and context reduction through actual API turns; UI refresh/result-shape behavior is out of scope; paired specialist-task outcomes remain mixed)

Source implementation plan:
`plans/implementation/standalone-mcp-local-inference/003-lm-studio-consumer-qualification-and-docs.md`

Source subsystem roadmap:
`plans/subsystems/standalone-mcp-local-inference-roadmap.md`

Final prior candidate exercised by the installed-child smoke and native API: `de69f1ddd7a7e4acc067fb9892c438f6165cfe83` (`eggsearch 0.4.4`; M004/M005 changes included). The M006 corrective candidate is recorded in §12 and its closure record.
Predecessor closures: M001 `d983fa5`; M002 `plans/closure/standalone-mcp-local-inference/002-status.md`.

## 1. Current finding

M003 is not closed. The installed-candidate stdio smoke, interactive LM
Studio chat path, and authenticated native `/api/v1/chat` path work, and all
ten tools remain available. Native API trials now include `google/gemma-4-e2b`,
`google/gemma-4-e4b`, `prism-ml/bonsai-27b`, and
`lfm3-2b-omni-abliter8`. The controlled E4B research/security turns exposed
valid tool calls alongside missing synthesis, weak source support, invalid
arguments, and a host-side plugin-unloaded error. The direct-fetch compact
no-call outcome also did not reproduce in the controlled baseline/candidate
comparison. This mixed evidence does not establish a general quality or
reliability improvement. The qualification scope was clarified on 2026-10-31:
LM Studio is the available local API host for MLX and other local models, while
the target is small-model tool-use quality and context cost. App UI refresh and
presentation behavior are not acceptance criteria.

The final candidate's installed-child stdio smoke passed on 2026-10-31:
full exposed all ten tools in 72,923 serialized `tools/list` bytes, compact
exposed the same ten in 19,370 bytes, and `provider_status` succeeded in both.
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
| Ten tools and profile list sizes | M006-candidate initialized child-process smoke: 72,923 full / 19,370 compact bytes; all ten names; `provider_status` with probing off | pass | Byte counts are not token counts |
| Configured MCP API path | Authenticated native `/api/v1/chat` calls through configured `mcp/eggsearch`, with task-specific `allowed_tools`; exact config rollback hash recorded | pass | None for the tested local host; this verifies the model/API path, not LM Studio UI behavior |
| Paired small-model comparison | §12-15 include direct fetch, web search, research, security applicability, PDF extraction, and a successful LFM3 browser-render pair | partial / mixed | Repeat research and browser tasks; improve compact warning interpretation and separate host failures from model outcomes |
| Tool choice, argument validity, evidence and failure truthfulness | §12-14 include valid calls, wrong-tool selection, argument repair, weak evidence, extraction errors, provider failures, and API/browser failures | partial / quality concern | Improve consistency on research and exact applicability ranges; distinguish host failures from model failures |
| Actual context/token reduction | LM Studio native API reports `input_tokens` including formatting, tool definitions, and prior messages; §12-14 record paired full/compact usage and tool-result lengths | pass for measured tasks | Repeat key pairs to estimate variability; no tokenizer identity or UI context meter is required |
| Eggsearch result contract | Offline MCP protocol tests cover structured results and text fallback; native API exposes returned tool output to the model | pass for Eggsearch contract | No host-specific result-shape or UI-consumption test is required |
| Config rollback | Restored original `~/.lmstudio/mcp.json`; SHA-256 is `b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c` | pass | None |

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

## 5. Earlier remaining-evidence list

The following list records the state before the scope clarification in §13. Its
references to UI behavior as a gate are superseded; UI refresh, UI result cards,
and LM Studio's internal structured-content/fallback selection are not M003
acceptance criteria.

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
6. M003 remains blocked on broader domain coverage and LM Studio UI
   structured-content/refresh behavior. The compact direct-fetch concern in
   §11 is requalified in §12 and tracked by M006; it is not a reproducible
   failure in the controlled baseline/candidate pair. Close M003 only after
   its remaining live acceptance criteria pass.
7. Current branch verification passes on `a339cce` (test-fixture-only
   stabilization over implementation candidate `de69f1d`): hosted CI run
   `41175183998` passed primary CI, macOS canonical/filesystem checks, and
   Windows portability; release-drift, egress, and Python-native workflows
   also passed. Local `make check` passed on `a339cce`; `make docs-check` and
   `make bench-check` passed on its documentation-only parent `3bd46b1` before
   the test-fixture-only change. The preceding documentation commit's hosted
   macOS run failed the cold local-workspace search fixture; that fixture
   relied on an implicit 500 ms budget derived from its 50,000-file cap. The
   10-second explicit test budget passes locally and on the hosted rerun. The
   timeout explanation is an inference from the configured budget and failure
   behavior; the hosted log did not print the response's timeout warning. This
   CI result does not
   close M003's live-model acceptance gaps.
8. A configured-server native API pair on 2026-10-31 (LM Studio 0.4.26+4,
   `lfm3-2b-omni-abliter8`, 24,000-token context, identical task and system
   prompt, same allowed tool and URL) exposed a compact tool-choice failure;
   see §11. A controlled repeat on the saved baseline and M006 candidate
   passed all three candidate turns and both baseline compact turns; see §12.
   The original failure remains a valid negative trial, but the correction's
   causal effect is not established.

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

### Remaining M003 acceptance (historical assessment before §13)

The paired model matrix now has exact prompt/completion usage for successful
requests and explicit timeouts where the host returned no counter, including
the 300-second post-correction Bonsai retry. It still
does not pass M003: the full Gemma extraction ended blank, Gemma's security
answer misstated evidence authority, Bonsai's security call failed argument
validation, and Bonsai full-profile inference timed out. Native LM Studio UI
tool-result handling and refresh behavior, structured-content/text fallback,
and broader domain coverage remained open at that time. The protected API
smoke is later qualified in §10. The UI-specific criteria in this historical
assessment are superseded by the scope clarification in §13.

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
protected native MCP API smoke criterion is passed for the single-tool success
path. A later same-task profile pair is recorded in §11 and shows a repeated
compact no-call failure. Interactive UI structured-content versus text
fallback and config refresh behavior also remain open, so M003 is still
blocked.

## 11. Native configured-server full/compact direct-fetch pair

With the user's pause window, `~/.lmstudio/mcp.json` was temporarily pointed
at the exact `de69f1ddd7a7e4acc067fb9892c438f6165cfe83` release candidate and
full then compact profile configs. Both requests used LM Studio's native
`POST /api/v1/chat` plugin integration, not the OpenAI-compatible manual
tool-definition harness. They shared the same model (`lfm3-2b-omni-abliter8`),
task and system prompt, allowed tool, context length, temperature, and
authentication page URL. LM Studio returned loaded instance IDs ending `:3`
for full and `:2` for compact.

The task asked the model to read the official LM Studio API authentication
page and explain where existing API-token permissions are edited. The system
instruction required exactly one `web_fetch`, prohibited setting
`response_detail`, and required evidence from that page.

| Profile / trial | Tool call | Actual usage (input + output) | Latency | Result |
|---|---|---:|---:|---|
| Full | One valid `web_fetch` for the exact documentation URL; returned page text was 14,067 characters | 1,627 + 97 tokens | 8.93 s | Correctly named the Edit button in the API Tokens modal and cited the page. |
| Compact, trial 1 | No tool call despite the exact URL, `allowed_tools=["web_fetch"]`, and explicit system instruction | 854 + 39 tokens | 1.92 s | Returned a generic “Authentication section” answer without fetched evidence. Quality fail. |
| Compact, repeated | No tool call under the same request conditions | 854 + 39 tokens | 2.46 s | Repeated the unsupported generic answer. Quality fail. |

The compact requests used 47.5% fewer input tokens and 59.8% fewer total
input-plus-output tokens than the full request, but skipped required retrieval
twice. These are LM Studio-reported token counts, not tokenizer-specific
attribution. This repeated difference shows the compact native API path is
not an acceptable direct-fetch default for this model and task without a
corrective decision. It does not establish that every model or task fails.

The original `mcp.json` bytes were restored after the test block and verified
against SHA-256
`b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c`.
Authentication and MCP permission settings were not changed. Corrective M006
(`plans/implementation/standalone-mcp-local-inference/006-compact-direct-fetch-tool-choice-corrective.md`)
is active to compare the compact description and test a minimal fix. The
controlled baseline/candidate follow-up is recorded in §12. UI chat refresh
and structured-content/text-fallback behavior are still not isolated.

## 12. M006 corrective and native specialist-task requalification

### Compact direct-fetch baseline/candidate comparison

On 2026-10-31, the saved pre-change executable (SHA-256 `da803489495080f30ad83095cd6f0fe56cceaa4ee1b4897a9767bf0ee1fec00a`) and rebuilt M006 candidate executable (SHA-256 `9b70544f64bc1d87a3956480c020074f3ae4e65e5002a927d83bf973b448f801`) were exercised through LM Studio 0.4.26+4 native `POST /api/v1/chat`. All turns used `lfm3-2b-omni-abliter8`, 24,000 context, temperature 0, the same system and user prompt, the same allowed `web_fetch`, and the exact same URL. The system required one fetch, forbade `response_detail`, and directed the model to answer only from the fetched evidence.

| Build / profile / trial | Actual usage (input + output) | Latency | Tool and evidence result |
|---|---:|---:|---|
| Saved baseline / compact 1 | 855 + 110 | 7.74 s | One valid fetch; HTTP 200; stable ID `fetch_de9d81a67951e3de`; returned text included the Edit-button sentence; answer cited the API Tokens modal. |
| Saved baseline / compact 2 | 855 + 110 | 4.05 s | Same valid tool call and identical result hash; answer quoted the permission-edit sentence. |
| Candidate / full | 1,628 + 89 | 7.08 s | One valid fetch; same stable ID; 14,067-character tool result; answered correctly. |
| Candidate / compact 1 | 872 + 105 | 3.73 s | One valid fetch; same stable ID and result hash as both baseline compact turns; answer cited the official section and URL. |
| Candidate / compact 2 | 872 + 105 | 3.72 s | Same valid call and answer evidence as candidate compact 1. |

Candidate compact used 46.4% fewer input tokens and 43.1% fewer total input-plus-output tokens than candidate full. Its initialized tools list was 19,370 serialized bytes, up 89 bytes from the prior 19,281-byte candidate and below the existing compact limit. The compact result was 3,713 serialized characters versus 14,067 for full; both carried the relevant edit-permission evidence and the same stable document identity. LM Studio's `output_truncated` flag was true for brief output, but the required sentence was present.

The first M003 native pair in §11 had two compact no-call failures. Under this controlled prompt, both saved-baseline compact turns and both M006 compact turns called and used the same source. That does not establish that the description edit caused the change; compact tool choice is variable across the recorded prompt conditions. The candidate meets M006's direct-fetch task gate, while the broader M003 model-quality gate remains open. The exact original config hash was restored after the test block, and authentication and permission settings were not modified.

### Gemma specialist-task observations

Additional native LM Studio API turns used `google/gemma-4-e2b` and the larger `google/gemma-4-e4b`, both at 24,000-token context, with one allowed specialist tool and 450 maximum output tokens. The E2B research full/compact turns each exhausted the generation budget in reasoning before a tool call (510/371 input tokens; 449 output tokens, 447 reasoning tokens). Its security full/compact requests returned HTTP 500 `Plugin unloaded` without usage counters.

E4B called `research_search` once with valid query arguments in both profiles. The full and compact results were 37,830 and 6,126 serialized characters respectively. At the 450-token output setting, neither trial synthesized a final answer. In a follow-up full-profile research turn with 1,800 output tokens, E4B called the tool and produced an answer, but the retrieved set had no primary or official source, included mostly low-confidence snippet-only records, and the answer made unsupported comparative claims. The paired compact retry returned HTTP 500 `Plugin unloaded`, so that higher-budget pair is incomplete.

For the security task, E4B full called `security_search` without a query or ecosystem; the tool correctly returned `invalid_semantic_value`, and the model truthfully stated that applicability could not be established. E4B compact supplied valid package, version, ecosystem, query, and goal arguments; no authoritative matching advisory was retrieved, and the model correctly did not infer that the package was safe. This is partial evidence of compact specialist reachability, not a completed vulnerability assessment. These runs preserve tool errors and evidence gaps as failures or inconclusive outcomes rather than passes.

All native API trials restored `~/.lmstudio/mcp.json` to SHA-256
`b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c`.
The UI refresh and result-presentation checks could not be exercised because
the computer-use host was unavailable; no profile swap was made during that
attempt. This was recorded as a blocker before the user's scope clarification
in §13 and is now historical evidence only.

## 13. Qualification scope update and repeated LFM3 API pair

On 2026-10-31, the user clarified that M003 qualifies small local models'
ability to use Eggsearch tools appropriately while the opt-in profiles reduce
context. LM Studio is only the available local inference/API host here,
including for MLX support. Its UI refresh, chat presentation, and internal
choice between MCP structured content and text fallback are not acceptance
criteria. This scope supersedes the earlier UI-related remaining-work wording
above; the recorded UI automation failure is retained as history, not as an
M003 blocker.

LM Studio's native `POST /api/v1/chat` API supports configured MCP plugins and
returns tool calls/results plus usage counters. Its documented
`stats.input_tokens` includes formatting and tool definitions, making paired
full/compact requests a direct context-cost measure for the selected local
model. Trials below used LM Studio 0.4.26+4, `lfm3-2b-omni-abliter8`,
24,000-token context, temperature 0, `max_output_tokens=1000`, `store=false`,
the same prompt and system instruction, and only `security_search` allowed.
The candidate executable was `target/release/eggsearch`, SHA-256
`52385c8454808a0c784f980dd12e79542ac8ab10a6133ace87396c875c4814dc`. Full
used `schema_profile="full"`; compact used `schema_profile="compact"` and
`brief_max_chars=8000`. The exact original `mcp.json` hash was restored after
the trials.

The task asked whether Apache Log4j 2.14.1 is affected by CVE-2021-44228 and
required an evidence-grounded applicability conclusion with failures retained.
The primary Apache security page lists affected Log4j versions as
`[2.0-beta9, 2.15.0)`, which includes 2.14.1, and states that `log4j-core` is
the affected component: https://logging.apache.org/security.html.

| Profile / trial | API input / output tokens | `security_search` result characters | Model disposition |
|---|---:|---:|---|
| Full, trial 1 | 682 / 450 | 32,659 | Valid call. Answer said applicability was not confirmed despite citing an NVD description whose affected range includes 2.14.1; it preserved provider failures. Incorrect/internally inconsistent conclusion. |
| Compact, trial 1 | 454 / 386 | 7,277 | Valid call with CVE query. Correctly said affected and cited NVD and vendor advisory; retained Yahoo 500 and unavailable native OSV lookup. |
| Full, repeat | 682 / 824 | 46,665 | Valid call. Answer again treated applicability as unconfirmed based on weak/low-confidence results; it noted Yahoo failures. Incorrect conclusion against the authoritative affected-version range. |
| Compact, repeat | 454 / 480 | 7,334 | Valid call with CVE query. Correctly said affected and cited the NVD and vendor advisory; retained Yahoo 500 and unavailable OSV lookup. |

Across these paired turns, compact input usage was 33.4% lower. Brief result
length was 77.7% and 84.3% lower in the two repeats. Combined input plus
output usage was 25.8% and 38.0% lower per corresponding trial. Compact
preserved the decisive advisory passage and provider-failure uncertainty, and
the model applied the version range correctly in both runs; full produced a
longer result but failed to apply the evidence in both runs. These are two
paired observations for one model/task, not a general model-quality claim.

A paired `web_search` task was not scored: full returned HTTP 500 with an
MCP structured-content/output-schema validation error (`query` and `results`
missing), while compact returned HTTP 500 `Plugin unloaded`. Neither provided
a model answer or usage counters, so these are recorded as API/tool-path
failures rather than model outcomes.

The expanded paired task results are recorded in §14. They cover direct HTML
fetch, web search, research search, security applicability, and PDF extraction
on two small models. Outcomes remain mixed: compact consistently reduced
measured input tokens, but it did not preserve adequate quality in every task.
At the time of the M007 trials, the compact schema did not advertise optional
`render`; M008 adds that argument. The paired API requalification in §15 shows
both profiles can extract the requested browser page, though compact's brief
result omits transport metadata and the model missed its `output_truncated`
warning. Repeat research and browser tasks before a broader quality claim.
M003 remains active, not blocked on the UI. The current acceptance objective and method are in
`plans/implementation/standalone-mcp-local-inference/003-lm-studio-consumer-qualification-and-docs.md`.

### Gemma E2B paired security trial

A second small model, `google/gemma-4-e2b`, was run through the same configured
MCP API path with the identical Log4j applicability task and system prompt.
The run used LM Studio 0.4.26+4, 10,000-token context, temperature 0,
`max_output_tokens=1200`, `store=false`, and only `security_search` allowed.
The same candidate binary and full/compact profile TOML settings as the LFM3
pair above were used. The original `mcp.json` hash was restored and verified
after the pair.

| Profile | API input / output tokens | `security_search` result characters | Model disposition |
|---|---:|---:|---|
| Full | 615 / 391 | 46,241 | Valid call with CVE, package, and query. It focused on Yahoo 500 failures and declined to conclude applicability, although the returned diagnostic result included advisory evidence. |
| Compact | 356 / 534 | 7,758 | Valid call with CVE, package, query, goal, and version. Correctly applied the affected range 2.0-beta9–2.15.0 to version 2.14.1 and cited the NVD source. |

Compact used 42.1% fewer input tokens and returned 83.2% fewer result
characters. Combined input and output tokens were 11.5% lower (890 versus
1,006); the compact turn used more completion tokens to give the correct
evidence-based conclusion. This matches the LFM3 direction on the same task,
but remains one pair for this model and one security case.

### All-tools palette pair after M007 schema correction

The M003 task was repeated with no `allowed_tools` filter, exposing the full
ten-tool palette to `lfm3-2b-omni-abliter8`. The candidate executable included
the M007 fix (SHA-256
`ff98098566a89f0603d1c07ad3cd88e5d36fae830bc8d6bde65cebc3456a71ea`). Both
requests used a 24,000-token context, temperature 0, the same Log4j/CVE task,
the same system instruction, and `store=false`.

| Profile / trial | API input / prompt tokens | Tool outcome | Model disposition |
|---|---:|---|---|
| Full, all tools | 34,779 prompt tokens; rejected against 24,064 available | No tool call; context overrun | No answer generated; cannot score model tool use. |
| Compact, all tools, after full request | No usage; HTTP 500 `Plugin unloaded` | No tool call | Host/plugin failure, not scored as a model outcome. |
| Compact, all tools, fresh compact-first request | 5,365 input tokens; 362 output tokens | Valid `security_search`; arguments included applicability intent, Log4j package, and CVE query; result was 7,215 characters | Correctly applied the NVD affected range to 2.14.1, cited the advisory, and retained Yahoo's 500 failure. |

The compact-first request passed the schema validation point that had failed
before M007: all ten tools were exposed and the API completed the specialist
call. Full exceeded this model's 24k context by 10,715 tokens. The 5,365 input
counter is the successful API response's usage value; the full value is the
inference engine's rejected prompt count. Treat this as evidence that the
compact palette fit and was usable in this task, not as a general percentage
savings or proof that every compact tool schema is accepted by every host.
The earlier post-full `Plugin unloaded` response remains recorded as a host
failure; the exact original MCP config was restored byte-for-byte after each
request block (SHA-256
`b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c`).

## 14. Expanded small-model task matrix on M007 candidate

The native LM Studio REST API was used as a local inference harness; its UI
behavior is not part of this qualification. LM Studio 0.4.26+4 invoked the
release executable built from the M007 candidate (SHA-256
`ff98098566a89f0603d1c07ad3cd88e5d36fae830bc8d6bde65cebc3456a71ea`). The
temporary configured-server entries exposed the full Eggsearch palette under
both profiles, without an `allowed_tools` filter for the matrix tasks. Each
pair used a fresh `store=false` request, temperature 0, the same model,
prompt, context size, and source URL. LFM3 used Q4_K_M GGUF at a 128,000-token
request context; Gemma E2B used 4-bit MLX at 32,000 tokens. The profile
settings were `full + diagnostic` and `compact + brief`, with PDF and browser
features enabled in the temporary Eggsearch config. API/plugin errors were
kept separate from model outcomes.

| Model / task | Full input / output tokens; result chars | Compact input / output tokens; result chars | Tool and task outcome |
|---|---:|---:|---|
| LFM3 2B — direct fetch, official Rust 1.90 announcement | 18,846 / 578; 21,120 | 5,378 / 596; 5,706 | Both made one valid `web_fetch` and correctly identified LLD and workspace publishing with source evidence. |
| LFM3 2B — web search, same release task | 18,821 / 994; 55,193 | 5,353 / 770; 18,973 | Full selected `security_search` for a release query and used secondary evidence. Compact selected `web_search`, repaired an excessive `excerpt_count`, fetched official Rust sources, and answered the requested changes correctly. |
| LFM3 2B — `research_search`, workspace publishing | 18,825 / 1,216; 53,140 | 5,357 / 1,175; 28,581 | Both invoked research. Full leaned on weak/secondary evidence; compact added official Rust release and Cargo sources and better substantiated the claim. |
| LFM3 2B — security applicability, Log4j 2.14.1 | 18,803 / 566; 53,020 | 5,335 / 610; 12,099 | Full made several calls, including repair attempts, and gave an imprecise upper range. Compact used `security_search` then fetched NVD/OSV; it gave the full affected range and reported Yahoo's 500. |
| LFM3 2B — PDF extraction, NIST FIPS 197 | 18,860 / 905; 50,732 | 5,392 / 574; 5,084 | Both fetched the exact PDF. Full incorrectly assigned 192- and 256-bit block sizes; compact correctly reported 128-bit blocks and 128/192/256-bit keys, noting truncation after the needed evidence. |
| Gemma E2B — direct fetch, official Rust 1.90 announcement | 4,612 / 1,030; 21,120 | 1,825 / 1,162; 5,706 | Both made one valid `web_fetch` and identified the two requested release changes. |
| Gemma E2B — web search, same release task | 4,585 / 1,971; 71,023 | 1,798 / 1,659; 13,129 | Full fetched the general GitHub releases page and answered with unrelated examples. Compact searched, fetched the official Rust blog, and gave the requested changes. |
| Gemma E2B — `research_search`, workspace publishing | 4,586 / 1,861; 55,062 | 1,799 / 1,731; 7,414 | Both invoked research but neither established the Rust 1.90-specific feature from primary evidence. Compact appropriately stated the evidence was inconclusive; full overstated a generic workspace result. |
| Gemma E2B — security applicability, Log4j 2.14.1 | 4,589 / 2,280; 140,916 | 1,802 / 1,184; 5,535 | Both concluded 2.14.1 is affected and cited advisories. Both gave an incomplete upper range; full also made extra `provider_status` and repair calls. |
| Gemma E2B — PDF extraction, NIST FIPS 197 | 4,634 / 947; 50,732 | 1,847 / 877; 5,084 | Both fetched the same PDF and correctly separated key length from the 128-bit block size. |

Compact reduced API-reported input tokens by 71.4–71.6% for the scored LFM3
pairs and 60.1–60.8% for the Gemma pairs. Result-character reduction varied
by task (approximately 46–90% for LFM3 research/PDF and 73–90% for the direct
fetch/PDF cases; approximately 73–96% for Gemma direct-fetch/security/PDF).
These are per-task measurements, not a universal savings estimate. Across
models, compact improved the release-search and PDF answers, while research
and exact security-range quality remained mixed. Fewer result characters did
not guarantee correctness.

### Browser-rendered extraction disposition

The React `useState` page was paired with the same question. Full-profile
requests explicitly using `render="browser"` timed out or returned HTTP errors
in the native API; a successful compact request fetched the page over HTTP
instead. Its result metadata reported `browser_escalated=false`, and the model
made incorrect claims about initializer functions and immediate state updates.
A separate `example.com` call with `render="browser"` returned HTTP 200, but
does not qualify browser extraction quality. The compact schema on this M007
candidate omitted the optional `render` parameter. M008 adds it and is
recorded in §15. The successful full-profile call and compact launch failure
are not counted as a successful paired browser extraction.

### Candidate and cleanup

The original `~/.lmstudio/mcp.json` was restored byte-for-byte after testing;
SHA-256 `b8cb90299d832914cba5f92dbb259684a084045cc503fd186fea4b4cee3fec7c`.
The local LM Studio server was restarted with that original registration.
This M007 candidate passed local `make check` and `make docs-check`;
hosted CI, release drift, and Python native qualification passed on commit
`fe677ff45a4c69feee50907cfbd451b6ae2a1fd4`. The separate hosted egress
qualification run `41220450274` passed its Linux, x86_64 macOS, Windows, MSRV,
and ARMv7 jobs; the Apple Silicon macOS job was cancelled by a scheduled
runner-image brownout. This egress-only cancellation did not block M007 or M008
closure. M003 remains active:
the matrix now demonstrates consistent context reduction on these tasks, but
tool choice, evidence quality, failure handling, and browser reachability still
vary by model and profile. Additional repeats and a clean browser pair are
needed before a release-quality recommendation.

## 15. M008 compact browser-render reachability corrective

M008 adds the runtime-supported `render` argument to compact `web_fetch`, with
the accepted enum values `http_only`, `auto`, and `browser`. The source change
is in `src/mcp/server.rs`; `tests/mcp_schema_slimming.rs` now checks that the
compact schema preserves all three values. The exact local candidate release
binary SHA-256 is
`507724a14a1926488913a50b1864543f1e899d8a3df6e1b44d1a90dcc4110b43`.

The LFM3 2B pair used the same `example.com` extraction prompt, all ten MCP
tools, and the M008 release executable. The local `/v1/chat/completions`
inference harness supplied each profile's actual MCP schemas, executed the
model's MCP call over stdio, and returned its result for synthesis. Full used
68,736 schema bytes, 18,813 first-turn input tokens, and 40,415 total input /
176 output tokens across the tool and answer turns. Its valid call requested
`render="browser"`, returned HTTP 200 with `transport="browser"`, and the
model accurately gave the heading, paragraph, URL, and external-content
warning. The tool result was 9,901 serialized characters.

Compact used 19,339 schema bytes, 5,382 first-turn input tokens, and 11,904
total input / 216 output tokens. Its valid call requested `render="browser"`
and returned HTTP 200; the brief result did not expose the `transport` field.
The model correctly gave the same heading, paragraph, URL, and external-content
warning. The tool result was 3,817 serialized characters with
`output_truncated=true`; the model did not report that outer-output warning,
although the extracted page content was complete. The brief output's
transport-field omission and warning miss are recorded as quality limits, not
as a failed browser call. Compact reduced schema bytes by 71.9%, total input
tokens by 70.5%, and result characters by 61.4% for this one task pair. The
React reference page remains unqualified.

The compact-profile browser call succeeds on the exact M008 executable.
M008 is closed at source commit `772a073fd40e9f044017b26bae18a6efd0724b1d`
after hosted Linux/macOS/Windows CI. The full local `make check` attempt
stalled in `bounded_command`, while the focused 20-test schema suite, release
build, formatting, diff, and documentation checks passed. M003 remains active
because research/security evidence quality is mixed and key specialist/browser
tasks need repeats. Existing measured token reductions are per task and do not
establish a general quality or reliability improvement.
