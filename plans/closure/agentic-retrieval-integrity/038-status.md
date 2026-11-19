# M038 Security-Search Native Advisory Routing — Closure Evidence

Status: closing (implementation and evidence complete; the closure commit is pending)

Source plan: `plans/implementation/agentic-retrieval-integrity/038-security-search-native-advisory-routing.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Decision authority: existing contracts — ADR-0006 keyless retrieval, M006 provider routing, and the
native advisory capability already implemented by the OSV provider. No new ADR.

## Scope

M038 makes `security_search` use the enabled native advisory provider for identifier-bearing
queries and makes its warnings describe what actually happened:

1. The workflow decided native availability from the *selected* providers, and with no explicit
   `providers` the wrapper passed `default_providers` (DuckDuckGo, Brave, Yahoo). An enabled OSV
   provider was never considered, so a CVE query returned zero vulnerabilities and told the operator
   to enable a provider that was already enabled.
2. `severity_unavailable` was pushed unconditionally after the generic lane, even when a native
   result carried severity.
3. `VulnerabilityMetadata` had no summary, so a native result could not say what the advisory was
   without a second fetch.

The paper-first contract is in `architecture/security.md` (identifier routing, warning conditions,
optional summary).

## Exact code candidate

Parent commit `614634198bfb6f145f7c223d5a330cc8924f677d`; this work is the uncommitted
working tree on top of it. The implementation fingerprint is
`103dbdf22a28f3bed06ff0209fc8fbabacf691be13d76e5bce072bd90fc89c7d` (SHA-256 of
`git diff -- src tests docs architecture CHANGELOG.md`, 2026-11-19T21:21:54Z). The registry,
roadmap, plan headers, and this record are outside that fingerprint and carry `closing` until
the closure commit.

## Requirement-to-evidence mapping

| # | Acceptance criterion | Evidence |
|---|---|---|
| 1 | With the shipped provider map, an identifier query selects OSV by default | The MCP wrapper unions the routing decision with `MetadataSearchAdapter::enabled_native_advisory_provider_ids()` only when `args.providers` is empty and the request is identifier-bearing (`cve_id`, `ghsa_id`, `osv_id`, `rustsec_id`, `package`). `tests/security_workflow.rs` covers the default CVE path with a fake OSV engine. Live probe: `selected = [duckduckgo, brave, yahoo, osv]`. |
| 2 | No provider is enabled by the code; `default_providers` unchanged | The accessor reads only instantiated engines. `src/core/config.rs` is untouched. A config with `osv = false` selects generic providers only. Live probe: 0 vulnerabilities and `native_advisory_search_unavailable` naming the disabled provider. |
| 3 | Explicit `providers` stays authoritative | The union is inside `args.providers.is_empty()`. Live probe with `providers: ["duckduckgo"]` and OSV enabled: `selected = [duckduckgo]`, 0 native vulnerabilities, no native-advisory warning. |
| 4 | `severity_unavailable` only when no returned vulnerability has severity | The warning moved after final assembly and checks `vulnerabilities.iter().any(|v| v.severity.is_some())`. Live default probe (critical, CVSS 10.0) emits no severity warning; the explicit generic probe emits it. |
| 5 | Optional bounded sanitized summary | `VulnerabilityMetadata.summary: Option<String>` with `skip_serializing_if`, merged first-available. OSV maps its `summary` through `sanitize_summary`: control characters stripped, empty becomes `None`, bounded to 500 characters. Live: `Xz: malicious code in distributed source`. `tests/mcp_projection.rs` retains it in compact/standard responses. |
| 6 | Truthful availability and budget warnings | `native_advisory_search_unavailable` fires only when no enabled provider advertises `lookup_by_id`/`query_by_package`; its message now says the OSV provider is disabled or not instantiated. The provider-operation cap warning names `MAX_NATIVE_ADVISORY_PROVIDER_OPERATIONS` (64). |
| 7 | Failures stay visible | The native success does not mask generic provider failures; `has_failures` and `providers_failed` are unchanged. Live default probe still reports the DuckDuckGo parse error, Brave 429, and Yahoo 500. |
| 8 | Broad gate | `make check` exit 0: 85 suites, 6,445 passed, 0 failed, 28 ignored. `make docs-check` exit 0. |

## Live before and after

All probes used MCP `stdio` against the candidate. The disabled-provider config explicitly set
`osv = false`; the default-config probe used the shipped provider map with `osv = true`.

| Probe | Before (review evidence) | After |
|---|---|---|
| `security_search` `cve_id: "CVE-2024-3094"`, OSV enabled | Selected DuckDuckGo/Brave/Yahoo only, 0 vulnerabilities, warned to enable OSV although it was enabled, plus unconditional `severity_unavailable` | Selected `[duckduckgo, brave, yahoo, osv]`, **1** vulnerability, severity `critical`, CVSS `10.0`, summary `Xz: malicious code in distributed source`; no `native_advisory_search_unavailable`, no `severity_unavailable`; generic engine failures still reported |
| Same request, `osv = false` | Generic-only lane | 0 vulnerabilities; warning `no enabled native advisory provider is available; the 'osv' provider is disabled or not instantiated in the active provider set`; `severity_unavailable` present |
| `providers: ["duckduckgo"]`, OSV enabled | Not probed | `selected = [duckduckgo]`; 0 vulnerabilities; no native-advisory warning; `severity_unavailable` present |

The second row is the key truthfulness check: the message now describes the actual configuration
(disabled), not an instruction to enable something already enabled.

## Verification

Using the pinned Rust 1.89 toolchain on the candidate:

- `make check`: exit 0 — **85 suites, 6,445 passed, 0 failed, 28 ignored**.
- `make docs-check`: exit 0.
- `cargo test --locked --all-features --test security_workflow`: 20 passed, 0 failed.
- `cargo test --locked --all-features --test native_security_attempts`: 18 passed, 0 failed.
- `cargo test --locked --all-features --test security_applicability_contract`: 68 passed, 0 failed.
- `cargo test --locked --all-features --test web_search_integration`: 78 passed, 0 failed.
- `cargo test --locked --all-features --test mcp_projection`: 34 passed, 0 failed.
- `cargo test --locked --all-features --test static_guards`: 67 passed, 0 failed.
- `cargo fmt --all -- --check` and `git diff --check`: clean.

The additive field grows the advertised `security_search` output schema by 37 bytes. The pinned
full advertisement moved from 69,895 to 69,932 bytes and its fingerprint from
`e74f91978f5b8b0a` to `517d279f73742d36`; the compact advertisement is unchanged
(`e540f73dd2a392b8`, 18,847 bytes). The live-report pins moved from -8,176 to -8,139 definition
bytes and from -2,044 to -2,035 estimated tokens. Those pin updates are part of this change.

## Known limitations and residual risk

1. **A keyword-incapable advisory provider joins the generic lane.** The union adds every enabled
   provider that advertises native advisory lookup, and the same selected set drives the generic
   keyword subqueries. An operator who enables `rustsec` therefore gets its native ID lookup plus
   its expected `keyword search not supported` failures in the generic lane, exactly as if they
   had listed it explicitly. The shipped default enables only `osv`, which supports keyword search,
   so the default path is unaffected.
2. **The union lives at the MCP wrapper.** Both the MCP tool and the SDK delegate to
   `run_security_search`, so they inherit it. A future caller that invokes
   `run_security_search_plan` directly with an empty provider list would not inherit the union;
   the wrapper is the documented routing seam.
3. **A summary is optional by design.** OSV advisories without a `summary` keep `None`; no text is
   invented. The 500-character bound may clip a long upstream summary.
4. **No provider enablement or default change.** `default_providers`, `[search.providers]`, and
   credential requirements are untouched; the change only routes among engines the configuration
   already instantiated.
5. **The warning-code taxonomy is unchanged.** `native_advisory_search_unavailable` and
   `severity_unavailable` keep their codes and prefixes; only the conditions and the availability
   sentence changed.
6. No new dependency and no new tool or argument.

## Compatibility and migration

One additive, optional field (`summary`) on `VulnerabilityMetadata`, omitted when absent, and one
default-selection change limited to identifier-bearing security requests among already-enabled
providers. No storage, configuration, or dependency change. The advertised schema grows by 37 bytes
and the pinned surface was updated accordingly.

## Recommendation

Accept M038 on the closure commit. The acceptance criteria are met by deterministic tests and by
the three live probes above, including the previously misleading enabled-but-not-selected case; the
remaining risks are configuration-dependent interactions recorded rather than hidden.
