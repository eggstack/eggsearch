# Code Duplication Consolidation — Integrated Gate

Status: passed

Candidate: the exact tree on branch `feat/auto-20261117-044c4885` based on parent
`fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64`. The 72 modified tracked files are identified by
the diff fingerprint below; the 24 added files are identified by their SHA-256 entries (this gate
record is excluded). Verification below was run against this candidate.

## Canonical gate

`RUSTUP_TOOLCHAIN=1.89.0 make check` exited 0:

- `cargo fmt --check`: clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- no-default-features library check: passed.
- 94 all-features test targets: **6,489 passed, 0 failed, 30 ignored**.
- `make hygiene`: `repo-hygiene: ok`.
- `make dependency-policy`: advisories, bans, licenses, and sources ok.
- `make packaging-check`: packaged assets, planning consistency, Eggpack qualifier contract,
  and the egress target contract ok.

## Documentation and planning gates

- `make docs-check`: exit 0.
- `make hygiene`: `repo-hygiene: ok`.
- `make packaging-check`: exit 0 (`planning-consistency: ok`).
- `python3 packaging/check-planning-consistency.py`: `planning-consistency: ok`.

## Review findings and disposition

- P2 response-contract regression: `SymbolKind::TypeAlias` now uses the canonical `type_alias` spelling; the structured repo-search response regression test pins it.
- P2 candidate-evidence mismatch: refreshed to 72 modified tracked files and 24 added files; the canonical gate and docs check were rerun on this corrected tree.
- P3 ADR status drift: the roadmap now records ADR-0023 as accepted and M007 as closed under it.
- P3 helper guard coverage: the guard now recursively checks Rust files under `tests/`, excluding the shared `tests/support/mod.rs`.
- The gate exposed a scheduling-sensitive 10 ms cache-expiry test. It now sets the expiry instant deterministically; the isolated test and full gate pass.
- Independent review findings are resolved; none remain open.

## Live MCP protocol qualification

- `RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features live-mcp-smoke --test mcp_http live_canonical_tools_and_aliases_work_over_mcp_http -- --ignored --nocapture` — **1 passed** in 99.47 s.
- The live test kept default `pdf` and `browser` features enabled, compiled without `mock`, used no API credentials, and returned the keyless defaults `brave`, `duckduckgo`, and `yahoo`. It exercised nine advertised tools, three callable aliases, PDF extraction, and browser rendering.

## Mutation checks

- M003: changing the `github_code` expected first title made
  `conversion_cases_parse_expected_rows` fail with `left: "src/lib.rs - test/repo"` versus
  `right: "src/lib.rs - mutated"`; restored.
- M006: changing the shared character-limit message made
  `core::query::tests::validate_rejects_oversized_query` fail; restored.

## Dependency note

`RUSTSEC-2026-0592` (stack overflow in `Url::origin()` on nested `blob:` URLs) affected
`url 2.5.8`. The lockfile now pins `url 2.5.9` and `form_urlencoded 1.2.3`;
`cargo deny check advisories` reports ok. No dependency was added or removed.

## Exact candidate fingerprint

- `tracked_diff_sha256=74ab3843052293f7b5e17b274b37f6a350b45bcf44eddc455f2fdeb714b34e4c`
  over 72 modified tracked files (`git diff | shasum -a 256`).

Added files relative to the parent (sha256; this gate record excluded):

| sha256 | Path |
|---|---|
| `ad250e0d7ea475350324c16d8c8446e6dc9a602e48391f32cbf1def69b7ea35b` | `plans/adrs/ADR-0023-one-repository-target-field-set.md` |
| `455265b44d76ff364acfbf5c598f31df96cb7ab997a7fbde85db79b955c94eae` | `plans/closure/code-duplication-consolidation/001-status.md` |
| `8fe0331f7bd1d7ca014ed4ceab07198b796315981810b8bcdd447db5e35586d9` | `plans/closure/code-duplication-consolidation/002-status.md` |
| `b465cece9eea905b86eb98c58045c9f76136aac20d357eab9b35ded8e75e1b1e` | `plans/closure/code-duplication-consolidation/003-status.md` |
| `34ece437a39d1bfc25be9b5ced58df6445e6e5ee5a8fc02ace7425ae26e78ac6` | `plans/closure/code-duplication-consolidation/004-status.md` |
| `eee27fbf39fef453d584199e58d9056a3cb3534fc534a06fa9c2a0909fe6d3d9` | `plans/closure/code-duplication-consolidation/005-status.md` |
| `6862e616957e6400fb62a07d3138d2c0f57c876d22701d6f3f2c62a10f4ba232` | `plans/closure/code-duplication-consolidation/006-status.md` |
| `94814e73eb6fa0204095cb5d98fbd40e39dfc043624b9799e227ace18bd77166` | `plans/closure/code-duplication-consolidation/007-status.md` |
| `cd09850a9c9bfe0f08205c959b582c42c162d0f351c49798d4988dd7961bcd75` | `plans/closure/code-duplication-consolidation/008-status.md` |
| `46dba3f5130734bf8f7f6d6062d1d706a095133b0862e773524357b881225923` | `plans/archive/implementation/code-duplication-consolidation/001-test-support-harness-and-default-arguments.md` |
| `fc087711d034ec26571f3a74d1db0076c52f93c1fe1fc856cc10c4465770637e` | `plans/archive/implementation/code-duplication-consolidation/002-provider-capability-declaration-compaction.md` |
| `0a0aeb0733d1e701e66a0d1a64d4d137759263d8890b9cdd1031a514edd7d4cc` | `plans/archive/implementation/code-duplication-consolidation/003-engine-api-conformance-harness.md` |
| `8c864441be69d7eb59b46350c444c0037235c6d1b8446a508937f94ffe0f28cc` | `plans/archive/implementation/code-duplication-consolidation/004-shared-engine-json-transport.md` |
| `2d4524f1a9ee9459ea85f8e7e374d5c7dd6e27b3537166e8d2aee1715d632b8a` | `plans/archive/implementation/code-duplication-consolidation/005-shared-fetch-escalation-seam.md` |
| `a1f9c229788157018003f36c0d5e613b87f45fceaabffc20c6ab3a17a81616f0` | `plans/archive/implementation/code-duplication-consolidation/006-core-request-validation-helpers.md` |
| `2119bdfe571d440b59229905a50f9f876a2281889814596950c0602649bf939c` | `plans/archive/implementation/code-duplication-consolidation/007-one-repository-target-field-set.md` |
| `4bd33765e112b9c80b8a29617a63953dbe5b03f2de84be36279cc1efe6707877` | `plans/archive/implementation/code-duplication-consolidation/008-searchengine-implementation-macro.md` |
| `037bd5ecd260c9fe454ca2e321df449189a99df39f350321fc02192ae6ef6193` | `plans/subsystems/code-duplication-consolidation-roadmap.md` |
| `ab1d9e55980750dcf22711acba2d90e9eaccf17c7b2dbcd07203a9981e965f7e` | `src/core/repo_target.rs` |
| `c399cb4135e3b7ee202520b51a487326a3c923dfb687ddf21013476a9ae7daa2` | `src/core/validation.rs` |
| `8c33dad334e60ff514ecab9acb0c5d122a30b0fe3032aa7a623eff0cb4221ea1` | `src/fetch/browser/escalation.rs` |
| `431480b842fa932739d8e4a665994d00ac029abe802b2f1c00c9d805ca3c9ddb` | `src/meta/engines/api.rs` |
| `25d8842e14650becb504e68d2c392ffa6e5e0f9b03f026edecd5bae31286586e` | `tests/engine_api_conformance.rs` |
| `fab001ed2bda4073242ead9023bd3858329edba22acc6c9d112b56ddd6efbd0d` | `tests/support/mod.rs` |

This gate record is excluded from its own content-hash inventory.
