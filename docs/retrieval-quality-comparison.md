# Retrieval Quality and Comparative Qualification

This report records M006 comparative evidence from candidate
`10b06ab76434983cd9942e81bf2090fc9d6e31cc`. It includes controlled fixture
results, optional live observations, feature builds, browser execution, and the
platform qualification outcome. The initial candidate exposed two macOS test
failures; repository-hardening M011 corrected and requalified them at candidate
`99316f31caf37ad2165cd5e822316942faf20c6f`. M006 is fully closed with that
corrective evidence.

## Method and scope

The offline evaluator remains `retrieval-quality-evaluator-v1`, using the
M001 corpus fingerprint `fnv1a64:b882ce3d844e8ace`: 15 self-authored cases,
9 training and 6 held out. Its judgments are useful for deterministic
regression checks, but are not independent relevance truth. M004 therefore
made no ranking change: its small held-out ceiling scores did not establish a
reliable lift.

The live lane is diagnostic. It used three keyless providers
(`duckduckgo,startpage,yahoo`), three cases, two trials, `en-US`, and no paid
provider allowlist. Its upper bound was nine provider-query attempts per trial,
18 total, with zero billable requests. Snapshots were captured on 2026-10-29
at approximately 13:58:34 and 13:58:38 UTC. A live snapshot is not a durable
provider guarantee.

## Quality and live provider observations

| Measure | Offline / fixture evidence | Live observations |
|---|---|---|
| Retrieval relevance | M001 baseline: 14/15 relevant documents retrieved in the frozen fixture result lists; recall 14/15. M004 held-out relevance subset: recall@5, MRR@5, nDCG@5, passage hit and status correctness each 1.0 over four relevance cases, five relevant documents and five passages; precision@5 0.25 over 20 slots. | Both aggregate trials scored precision, recall, MRR and nDCG at 0; no live result matched a judged document. The synthetic judged corpus is not a credible live relevance benchmark. |
| Provider contribution | M003 fixture arithmetic: DuckDuckGo 1/3, Startpage 2/3, Yahoo 2/3 individual relevant hits; combined 3/3, with two overlapping hits. These are synthetic assignments. | All 15 ranked cards came from DuckDuckGo. Startpage returned no cards and no failure. Yahoo had `http_status` failures on all three tasks in both trials. No judged document matched. |
| Empty / failure status | Fixture distinguishes empty success from provider failure. | Expected-absence status was correct in 3/3 tasks in trial 1 and 2/3 in trial 2; DuckDuckGo returned cards for the expected-absence task in trial 2. Results were not mislabeled as failures. |
| Live latency | Not a relevance-quality measure. | Combined-task p50/p95: 1,050/1,162 ms in trial 1 and 1,044/1,133 ms in trial 2. The cases and trial count are too small for general latency claims. |

No independent live gold set, paid API comparison, or configured SearXNG
instance was available. No universal provider superiority, marginal-recall,
or cost-effectiveness claim follows from these observations. Logs retained
response and canonical-source hashes and provider attribution, not response
bodies or raw source URLs.

## Feature and runtime comparison

All four release feature arms built on the exact candidate on macOS x86_64.
Sizes are exact executable bytes; the short `--version` timings and process
RSS are host observations, not fetch or browser latency.

| Feature arm | Release executable | `--version` elapsed | Max RSS | Qualification |
|---|---:|---:|---:|---|
| Default | 19,310,288 bytes | 2.37 s | 6,152,192 bytes | Build and CLI smoke passed |
| `pdf` | 20,659,400 bytes | 2.51 s | 6,209,536 bytes | Build and CLI smoke passed |
| `browser` | 23,871,472 bytes | 3.32 s | 6,324,224 bytes | Build and CLI smoke passed |
| `pdf browser` | 25,216,456 bytes | 3.46 s | 6,316,032 bytes | Build and CLI smoke passed |

The process is x86_64 on macOS. These measurements do not represent other
architectures and do not include an isolated Chrome process measurement.
Python's published `eggsearch==0.4.4` macOS x86_64 abi3 wheel was installed in
an isolated temporary environment; `AsyncEggsearch().provider_status(probe=False)`
returned the expected structure and ten tool names without credentials or
provider calls.

### Browser execution

The ignored JavaScript-rendering smoke ran three times on this candidate
against the public [Scrapify JS-rendered practice page](https://scrapifydatalabs.com/playground/js-rendered).
The test verified browser transport and the inserted `product-card` and
`data-ready` content. All three runs passed. Elapsed times were 20.99 s,
8.85 s and 8.93 s (median 8.93 s; nearest-rank p95 20.99 s). `/usr/bin/time
-l` maximum process RSS was 122,671,104, 117,043,200 and 122,900,480 bytes.
These are Cargo/test-process figures, not isolated Chrome RSS; the first run
was an outlier. The page is a diagnostic external practice fixture, not a
rights-cleared quality corpus. No browser-rendered relevance score or paired
HTTP-vs-browser quality lift was measured.

## Context and prior decisions

M005 measured 72,939 serialized bytes for ten tool definitions and 14,008
bytes for the `web_search`, `web_fetch`, and `repo_search` core palette, an
80.8% schema-byte reduction. The 15-task synthetic trace retained the same 14
relevant result documents present in its fixture lists across diagnostic,
standard and compact projections. Token estimates use four bytes per token;
no model, tokenizer or real agent runtime was measured. Full details remain in
`docs/retrieval-quality-context-efficiency.md` and
`docs/retrieval-quality-codegg-handoff.md`.

M002 established the four compile-time feature arms and PDF fixture behavior;
M006 adds actual Chrome execution. M003's source attribution remains a fixture
validation, supplemented by the bounded live observations above. M004 remains
a no-change ranking decision. No ranking, provider routing, public schema,
default-feature, or release behavior changed for M006.

## Verification

Passed on exact candidate `10b06ab76434983cd9942e81bf2090fc9d6e31cc`:

- `cargo fmt --check`.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- `make bench-check`, `make docs-check`, `make packaging-check`.
- `make eval-retrieval-quality`: 8 passed; the ignored live test remained a
  separate opt-in lane.
- Focused fetch, browser, batch, provider-routing, dispatch, MCP contract,
  evidence handoff and Mode-Off zero-remote-call suites passed (see M006
  closure record for exact commands and counts).
- Default, PDF, browser and combined release builds passed; installed CLI and
  Python wheel smoke passed without API keys.
- Three exact-candidate JavaScript browser smoke runs passed.
- Two bounded keyless live provider trials completed; no paid requests were
  enabled.

The initial candidate's canonical `make check` did not pass on this macOS host. Across no-default,
PDF, browser, combined-feature and all-feature test runs, 3,348–3,484 tests
passed and the same two tests failed:

1. `mcp::state::tests::host_scopes_authenticate_and_cache_distinct_roots`
   compares `/private/var/...` with `/var/...` without accounting for the
   macOS temporary-directory alias.
2. `meta::repo_mapper::tests::structure_file_reader_rejects_symlink_escape_even_when_following_is_enabled`
   expected an outside-root symlink to be rejected, but it was followed on
   macOS when symlink following was enabled. This is a safety-boundary failure
   requiring corrective implementation and regression evidence.

M011 added an early `SafeSymlinkFollowingUnsupported` return on non-Linux Unix
when following is requested, and the root-scoping test now compares canonical
filesystem paths. See `plans/closure/repository-hardening/011-status.md` for
the corrected candidate and verification. The corrected candidate's
`make check` passed with 3,486 all-feature tests, and the local filesystem
suite passed under default, PDF, browser, and combined PDF/browser feature
arms. No ranking or provider-routing behavior changed.

## Compatibility, costs and limitations

There were no public API or default behavior changes and no release was
published. Default and optional feature builds remain available. Live provider
cost was zero under the selected keyless cohort. Network latency figures are
limited to the two observed trials. Browser timing used an external public
practice page. RSS is for the test command. No paired resource benchmark,
independent live relevance corpus, paid-provider comparison, or cross-OS green
canonical gate is claimed.

M006 is closed. Its diagnostic limitations remain: no independent live gold
set, paid-provider comparison, configured SearXNG instance, paired extraction
quality benchmark, or isolated Chrome RSS measurement was produced. No
downstream consumer implementation or release handoff is required by this
no-ranking-change result.
