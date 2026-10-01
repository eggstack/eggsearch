# Plan 003 — CodeGG Retirement Handoff and Qualification

Status: blocked

Source roadmap:

- `plans/subsystems/codegg-legacy-search-parity-corrective-addendum.md`

Milestone: M003 compatibility qualification and downstream handoff

Primary class: compatibility closure + integration handoff

Planning baseline: `d16a6c6adb04a5b6e2ccb8fec68987e80a452875`

Hard dependencies:

- M001 keyless/source-specific provider parity closed.
- M002 credentialed provider parity closed.

Operational downstream dependency:

- CodeGG must consume a tagged/published eggsearch release carrying the accepted
  M001/M002 surface rather than pinning an unqualified moving branch.

## 1. Objective

Prove that eggsearch now owns every legacy CodeGG external-search capability
that should survive, freeze the exact provider migration/disposition matrix,
and produce the release/contract evidence needed for CodeGG to delete its
in-tree external-search backend.

M003 does not edit CodeGG. It establishes the upstream contract that the
separate CodeGG retirement workstream may consume.

## 2. Required migration matrix

The closure record must contain a final matrix derived from real implemented
provider IDs, not this planning expectation.

Expected mapping if M001/M002 land as planned:

| CodeGG historical hint | Eggsearch disposition |
|---|---|
| `auto` | no explicit provider list; normal eggsearch routing |
| `duckduckgo` | `duckduckgo` |
| `mojeek` | `mojeek` |
| `wikipedia` | `wikipedia` |
| `arxiv` | `arxiv` |
| `openalex` | `openalex` |
| `pubmed` | `pubmed` |
| `hn_algolia` | `hn_algolia` |
| `github` | `github_repositories` |
| `exa` | `exa` |
| `tavily` | `tavily` |
| `brave` / `brave_api` | `brave_api` for API-key semantics; document `brave` HTML separately |
| `serpapi` | `serpapi` if M002 landed |
| `kagi` | `kagi` if M002 terms gate passed; otherwise deliberate retirement |
| `google_news` | no exact provider; migrate to `intent="news"` and a supported news provider when explicit source selection is required |

The matrix must distinguish exact source preservation from capability
preservation. It must never claim that an unrelated general provider is an
exact replacement for a removed source.

## 3. Explicit-provider failure semantics

Qualification must prove that an explicit provider request is never silently
converted into automatic routing because the provider is unknown, disabled, or
missing credentials.

For a supplied `providers` list:

- unknown provider -> actionable provider-level validation/skip evidence;
- disabled provider -> typed disabled skip;
- missing required credential -> typed missing-key skip;
- unavailable capability -> typed capability-skip evidence;
- no selected provider able to execute -> bounded degraded/error response that
  preserves the reason.

Automatic routing remains correct only when the caller intentionally omits
providers.

If current `web_search` behavior silently drops an unknown explicit provider
and proceeds with defaults, M003 must treat that as a correctness blocker and
create/execute the smallest corrective change before closure.

## 4. CodeGG integration contract

Update `docs/codegg-integration.md` and `architecture/codegg-contract.md`
with a downstream retirement section containing:

- exact provider migration matrix;
- minimum eggsearch version/tag carrying the parity contract;
- provider-status inventory expectations;
- explicit-provider failure behavior;
- Google News RSS non-migration guidance;
- Kagi disposition;
- stable ten-tool statement;
- statement that CodeGG native wrappers remain the normal model-facing API;
- statement that CodeGG may remove `backend="builtin"` and
  `fallback_to_builtin` once it pins this release.

Do not add CodeGG-specific fields to MCP requests/responses.

## 5. Qualification scope

M003 must re-run provider inventory/capability/documentation tests after all
new providers are present and verify at least these user journeys with mock
fixtures:

1. explicit Wikipedia search;
2. explicit arXiv scholarly search;
3. explicit PubMed scholarly search;
4. explicit HN search;
5. explicit GitHub repository discovery;
6. explicit SerpAPI with configured fake credential;
7. explicit Kagi with configured fake credential if shipped;
8. news-intent search using an actually supported news provider;
9. unknown provider request;
10. missing-credential provider request.

The fixtures must execute through the same adapter/tool surface used by MCP,
not instantiate engines as the only end-to-end evidence.

## 6. Release and version handoff

M003 implementation closure establishes a release candidate, but downstream
CodeGG retirement must wait for a tagged release containing it.

Required release handoff evidence:

- exact eggsearch commit SHA;
- final version number intended for release;
- `eggsearch --version` expected output;
- final `provider_status` inventory count/IDs;
- release note/changelog entry describing the CodeGG parity providers and
  Google News/Kagi dispositions.

If the normal eggsearch release is not cut during M003, close M003 only as
**conditionally closed** with one operational condition: tagged release
publication. CodeGG's M001 remains blocked until that condition clears.

Do not cut an otherwise-unwanted release merely to make the plan look closed.

## 7. Documentation reconciliation

Update factual provider-count/inventory statements wherever the implementation
changed them, including canonical planning documents whose numeric inventory
is now stale. This is a factual reconciliation, not a rewrite of historical
closure records.

At minimum inspect:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `architecture/engines.md`
- `architecture/meta.md`
- `architecture/config.md`
- `architecture/codegg-contract.md`
- `docs/provider-setup.md`
- `docs/codegg-integration.md`
- `docs/tool-matrix.md`
- `docs/test-inventory.md`
- `README.md`
- `AGENTS.md`

## 8. Required verification

Run on the exact qualification candidate:

```bash
cargo fmt --all -- --check
cargo test --locked --features mock --test web_search_integration
cargo test --locked --features mock --test provider_routing
cargo test --locked --all-features --test provider_capability_contract
cargo test --locked --all-features --test docs_provider_inventory
cargo test --locked --all-features --test dispatch_fault_injection
cargo test --locked --all-features --test static_guards
make docs-check
make check
make release-check
```

Run any existing CodeGG contract/mock integration suite owned by eggsearch.

A local live-provider smoke MAY be recorded for selected keyless providers but
is supplementary only.

## 9. Acceptance criteria

M003 reaches implementation closure when:

- every legacy CodeGG provider hint has an exact upstream mapping or explicit
  non-migration disposition;
- provider IDs/count/config/docs/tests agree;
- explicit unknown provider selection cannot silently become automatic search;
- Google News RSS is explicitly retired unless a separately accepted supported
  provider plan exists;
- Kagi disposition is final and documented;
- CodeGG integration docs define a minimum downstream version/tag;
- the ten-tool MCP surface is unchanged;
- full `make check` and `make release-check` pass.

M003 is fully closed only after the qualifying tagged eggsearch release exists.
Until then it is conditionally closed and downstream CodeGG parity adoption
remains blocked.

## 10. Stop conditions

Stop and create a corrective plan if:

- an explicit-provider request can bypass provider identity and route to
  defaults;
- a new provider breaks stable SourceCard/trust semantics;
- provider inventory updates expose a compatibility break beyond additive
  provider IDs/kinds;
- release qualification finds an unrelated medium-or-higher correctness issue;
- downstream compatibility would require a CodeGG-specific MCP schema.

## 11. Closure evidence required

Create `plans/closure/codegg-legacy-search-parity/003-status.md` with:

- M001/M002 closure links;
- exact final provider migration/disposition matrix;
- provider inventory/count;
- explicit-provider negative-path evidence;
- CodeGG integration documentation evidence;
- complete verification results;
- exact qualification candidate SHA;
- tagged release/version evidence, or the single named operational condition if
  publication is still pending;
- unresolved findings by severity;
- recommendation: closed, conditionally closed, or corrective pass required.
