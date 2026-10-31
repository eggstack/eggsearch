# Standalone MCP Local-Inference M003 — Operational Status

Status: blocked (native API retrieval works for a controlled direct-fetch pair, but specialist-task outcomes are mixed and LM Studio UI refresh/result-shape behavior remains uncharacterized)

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
reliability improvement; LM Studio UI refresh and result-shape acceptance
remain open.

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
| LM Studio tool visibility and interactive tool use | Prior full/compact qualification chats called `web_search`; compact also called `security_search`, `research_search`, and `web_fetch`. The app rendered a `web_fetch mcp/eggsearch` result card. | partial | Verify refresh/new-chat behavior against the final candidate; determine whether the app consumes structured content, text fallback, or both |
| Paired small/larger model comparison | Gemma E2B and E4B native API trials plus earlier Gemma E2B/Bonsai 27B and LFM3 2B task pairs; see §12 | partial / mixed | Repeat larger-model specialist tasks with a refreshed MCP plugin and enough output budget to synthesize results |
| Tool choice, argument validity, repairs, evidence and failure truthfulness | Earlier interactive findings plus §12; includes no-call, invalid-argument, plugin-unloaded, and truthful evidence-gap outcomes | partial / quality concern | Re-run a balanced research/security matrix after LM Studio plugin refresh; verify source authority and answer citations |
| Exact tokenizer and prompt-token attribution | Controlled native API pair in §12 reports 1,628 input / 89 output tokens for full and 872 / 105 for candidate compact; UI still exposes no tokenizer identity | partial | Keep host usage separate from tokenizer identity and characterize the UI context meter |
| Output schema / structuredContent vs text fallback | Offline protocol coverage plus a successful rendered UI tool-result card and native API tool call | partial | Isolate which result representation the UI consumes; compare a generic MCP inspector/client if available |
| Protected API MCP smoke | Authenticated native `/api/v1/chat` call through configured `mcp/eggsearch`, with only `web_fetch` allowed | pass | UI structured-content/text fallback and refresh behavior remain uncharacterized |
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
The remaining UI refresh and structured-content/text-fallback acceptance was
not executable through the available UI automation in this environment; the
LM Studio UI native pipe startup failed. A new `cua.getState()` attempt on
2026-10-31, after the user confirmed a client pause window, returned the same
native-pipe startup failure and reported that the browser access host was
unavailable; no profile swap was made. M003 remains blocked until those UI
checks and the larger-model specialist matrix are completed or explicitly
re-scoped.
