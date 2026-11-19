# M038 — Security-Search Native Advisory Routing

Status: closing
Date: 2026-11-19
Subsystem: agentic retrieval integrity
Milestone class: capability (corrective)
Decision authority: existing contracts — ADR-0006 keyless retrieval, M006 provider routing, and the
native advisory capability already implemented by the OSV provider. No new ADR.

References: `plans/000-long-term-specification.md` §3 (retrieval architecture) and §5 (provider
model); `architecture/security.md`; `tests/provider_capability_contract.rs` (the native
enforcement matrix).

## Objective

Make `security_search` use the enabled native advisory provider for identifier-bearing queries by
default, and make its warnings describe what actually happened. A caller asking for
`CVE-2024-3094` should receive the advisory, its severity, and a bounded summary in one call
instead of an empty result set and an instruction to enable a provider that is already enabled.

## Evidence

Measured on 2026-11-19 against candidate `55180790c6dc5dbb2ce0d94c186ef88cd7584f48`:

1. `security_search` with `cve_id: "CVE-2024-3094"` and the shipped provider map (OSV enabled,
   `default_providers` = DuckDuckGo, Brave, Yahoo) selected exactly the three generic engines,
   returned **0 vulnerabilities**, and warned
   `native_advisory_search_unavailable: only generic web search was used; enable the 'osv'
   provider for native advisory lookups`. It also returned 6 `suggested_fetches`, including
   `https://osv.dev/vulnerability/CVE-2024-3094`.
2. The same request with the hidden `providers: ["osv"]` selected OSV, completed 4/4 jobs, and
   returned one vulnerability: severity `critical`, `cvss_score: 10.0`, CVSS 3.1 vector,
   GHSA/OSV IDs, affected ranges, and vulnerable versions. The result carried **no summary or
   title**, and the response still emitted
   `severity_unavailable: severity levels may not be available from generic web search results;
   use native advisory providers for severity data`.
3. `web_search` with `providers: ["osv"]` works keyless: one OSV card, no provider failures.
4. `security_search` decides `has_native_advisory` from the **selected** providers
   (`advisory_provider_capabilities(&effective_providers)`), not from enabled providers. With no
   explicit `providers`, routing selects `default_providers`, so OSV is never considered.
5. `severity_unavailable` is pushed unconditionally after the generic lane
   (`src/meta/security_search.rs`).
6. `VulnerabilityMetadata` has IDs, ranges, severity, CVSS, KEV, timestamps, references, and
   source, but no summary/title/description field, so a native result cannot answer "what is this
   advisory" without a second fetch.
7. `tests/security_workflow.rs` asserts the empty case but not the native-selected case;
   `tests/repo_workflow.rs` pins a `severity_unavailable` occurrence;
   `tests/web_search_integration.rs` pins the separate `web_search` intent=security warning.

## Scope

In scope:

- `security_search` provider selection for identifier-bearing requests;
- the native-advisory and severity warning conditions and messages;
- an additive advisory `summary` on native results;
- tests and documentation for those contracts.

Out of scope:

- enabling or disabling any provider, credentials, or new advisory sources;
- the `web_search` intent=security warning path;
- severity normalization, CVSS scoring, or applicability logic;
- OSV request shape beyond mapping its returned summary;
- new dependencies or a new tool/argument.

## Invariants

- OSV stays keyless and no new dependency is introduced.
- The plan selects only providers already enabled by configuration; it never enables one.
- Advisory content remains `external_untrusted` and is sanitized and bounded.
- Explicit `providers` behavior is unchanged and still authoritative.
- At most one native lookup per identifier, inside the existing timeout and operation budget.
- The change is additive on the wire: one optional `summary` field, omitted when absent.
- Failure semantics are unchanged: provider failures are reported, and `has_failures` is not
  masked by a native success.
- No secret, credential, or advisory body beyond the bounded summary enters the response.
- No new comment beyond required doc comments; `cargo fmt` clean; ordinary files under
  1,600 lines / 80 KB.

## Expected production changes

- `src/meta/security_search.rs`: selected-provider set for identifier queries and the two warning
  conditions.
- `src/core/security.rs`: the optional `summary` on `VulnerabilityMetadata`.
- The OSV provider's normalization: map the advisory summary.
- `tests/security_workflow.rs`, `tests/native_security_attempts.rs`: regression coverage.

## Compatibility and migration

- One additive, optional field (`summary`) on `VulnerabilityMetadata`; it is omitted when absent
  and does not change existing fields.
- Default provider selection changes only for identifier-bearing security requests, and only among
  providers that configuration already enables. `default_providers` and enablement flags are
  unchanged.
- No storage, configuration, or dependency change. No migration.

## Dependencies

- Hard: none (the OSV provider and the native lane are shipped).
- Interface: M006 routing (closed), `architecture/security.md`, and the provider capability
  contract in `tests/provider_capability_contract.rs`.
- Soft: none.

## Work packages

1. **Paper first.** Update `architecture/security.md` (identifier routing, native lane, warning
   conditions, and the optional summary) and `architecture/engines.md` if the OSV capability note
   needs it. This lands before code.
2. **Select the native lane.** For identifier-bearing requests (`cve_id`, `ghsa_id`, `osv_id`,
   `rustsec_id`, or `package`) with no explicit `providers`, add every **enabled** provider that
   advertises `lookup_by_id` or `query_by_package` to the selected set, alongside the existing
   generic lane. Do not change `default_providers` and do not enable a provider.
3. **Truthful warnings.** Emit `native_advisory_search_unavailable` only when no native advisory
   provider is enabled; if one is enabled but was skipped for a bound, name the bound. Never
   instruct the operator to enable a provider that is already enabled.
4. **Conditional severity warning.** Emit `severity_unavailable` only when the assembled response
   contains no vulnerability with severity. Do not emit it when a native result carries severity.
5. **Advisory summary.** Add `summary: Option<String>` to `VulnerabilityMetadata` (additive,
   `skip_serializing_if = "Option::is_none"`), map the OSV `summary` field, sanitize and bound it
   (a small ceiling such as 500 characters), and verify it survives the compact and standard
   projections.
6. **Tests.** Extend `tests/security_workflow.rs` and `tests/native_security_attempts.rs`:
   - default config with OSV enabled and a CVE: OSV is selected, at least one vulnerability has
     severity and summary, and neither misleading warning appears;
   - OSV disabled: zero native results and a warning that names the disabled provider;
   - explicit `providers` unchanged;
   - generic-only lane with no severity: `severity_unavailable` still appears;
   - compact projection retains `summary`;
   - applicability and failure tests unchanged.
   Update `tests/repo_workflow.rs` and any schema/warning registry entries only where the
   contract genuinely changed; leave `web_search_integration`'s intent warning untouched.
7. **Documentation.** `architecture/security.md`, `docs/features.md`, `docs/tool-matrix.md`,
   `docs/provider-setup.md` (the native advisory selection note), `docs/test-inventory.md`, and a
   `CHANGELOG.md` entry under the existing unreleased section.

## Verification

```bash
cargo test --locked --all-features --test security_workflow
cargo test --locked --all-features --test native_security_attempts
cargo test --locked --all-features --test security_applicability_contract
cargo test --locked --all-features --test web_search_integration
cargo test --locked --all-features --test mcp_projection
cargo test --locked --all-features --test static_guards
make check
make docs-check
./packaging/check-repo-hygiene.sh
```

## Acceptance criteria

- With the shipped provider map (OSV enabled) and a `cve_id`, `security_search` queries OSV by
  default, returns at least one vulnerability with severity and a bounded summary, and emits
  neither `native_advisory_search_unavailable` nor `severity_unavailable`.
- OSV is still never enabled by the code; a disabled OSV yields zero native results and a warning
  that says it is disabled, not that it should be enabled.
- Explicit `providers` continues to be authoritative and unchanged.
- `severity_unavailable` appears only when no returned vulnerability carries severity.
- The optional `summary` is additive, bounded, sanitized, and visible in compact/standard
  responses; no existing field changes meaning.
- Provider failures remain visible and are not replaced by a native success.
- The broad gate passes with 0 failures.

## Stop conditions

Stop and record rather than working around any of these:

- the fix would enable a provider that configuration leaves disabled, or change a shipped default
  enablement (that needs its own measured acceptance);
- OSV leaves `summary` absent or unparseable for a material share of advisories; keep the field
  optional and record the limitation rather than inventing text;
- running the generic lane and the native lane cannot fit inside the existing deadline and budget;
  choose native-first for identifier queries and record the tradeoff;
- the change would require touching the `web_search` intent=security warning contract; defer that
  to its own plan.

## Required closure evidence

`plans/closure/agentic-retrieval-integrity/038-status.md` with the exact candidate SHA, a
requirement-to-evidence matrix, the before/after responses for the default and explicit-provider
probes, the suite results, the broad-gate counts, the summary mapping and its bound, and residual
risk (including any provider whose summary remains unavailable).
