# Dynamic Fetch Proxy Routing M003 — Qualification, Packaging, and Operator Documentation — Status

Status: closed
Date: 2026-11-24
Milestone: M003 — Qualification, Packaging, and Operator Documentation
Decision authority: ADR-0025 (accepted); ADR-0002; ADR-0014
Baseline: `d932115d1f86af51710747d243455792ff63a6fb`

## Candidate and changes

Local candidate: the baseline plus the M001–M003 change set. This milestone adds
no capability; it qualifies M002 and completes the operator contract.

- `docs/features.md` § Dynamic-Fetch Proxy Routing: supported and refused
  combinations, policy semantics, credentials, determinism, fail-closed errors,
  cache isolation, challenge coupling, diagnostics, and the distinct scope of
  `[egress]` versus `[fetch.proxy]`.
- `architecture/fetch.md` §8: the native eggfetch proxy route, redirect
  re-pinning, and the cache identity rule.
- `architecture/build.md`: `proxy` moved from the forbidden eggfetch feature
  list to the enabled budget, with the ADR-0025 rationale; the pinned dependency
  version corrected to 0.2.2.
- `architecture/testing.md`, `docs/test-inventory.md`,
  `skills/eggsearch-dev/SKILL.md`: `fetch_proxy_route`, `site_map`, and the
  extended `proxy_route_feasibility` inventory.
- `tests/static_guards.rs`: the eggfetch feature budget now requires `proxy`;
  the `src/core/config.rs` ratchet moved to 2,035 lines.
- `src/mcp/tools/web_fetch_options.rs` (new): the render/route validation and
  error helpers extracted from `web_fetch.rs` (1,729 → 1,545 lines) so the
  tool module stays under the ordinary 1,600-line ceiling.
- `tests/mcp_schema_slimming.rs`, `tests/mcp_tool_contract.rs`,
  `tests/tool_surface_live.rs`: measured schema and advertisement pins
  refreshed for the new item type.

## Requirement-to-evidence matrix

| Criterion | Evidence | Outcome |
|---|---|---|
| Failure matrix at the handler level | `tests/fetch_proxy_route.rs` (7) and handler cases in `tests/proxy_route_feasibility.rs` | pass |
| Pin preservation | CONNECT authority is the pinned `IP:port`; SOCKS5 target is `atyp: 1`; origin sees the logical hostname | pass |
| Fail-closed behavior | `proxy_route_unconfigured`, `proxy_scheme_unsupported`, `proxy_credentials_missing`, `proxy_authentication_failed`, `proxy_connect_failed`, `proxy_target_unsupported`, `proxy_route_changed_by_redirect` all asserted | pass |
| Cache isolation | Raw and derived key tests; `cache_only` under a proxy route | pass |
| `repo_fetch` and provider routing unchanged | Sole `with_proxy_route` call site; no proxy references outside web fetch | pass |
| Packaging: no new SKU or artifact | `make packaging-check` passes; the feature set is unchanged and no artifact is added | pass |
| Operator documentation | `docs/features.md`, `docs/config.md`, `architecture/fetch.md`, CodeGG guidance, and the test inventories | pass |
| Cost recorded | Final release binary measured against the baseline and feature-only builds | pass |
| `egress` lane unchanged | `egress-feature-qualify` contract and `tests/egress_routing.rs` unchanged and passing | pass |

## Size measurements (local macOS arm64 release build)

| Build | Bytes |
|---|---|
| Web work only, `proxy` feature off | 27,214,144 |
| `proxy` feature on, before M002 | 28,194,960 |
| Final candidate (all milestones) | 28,345,248 |

The feature-attributable delta measured in isolation in M001 is +980,816 bytes
(+3.60%). The additional +150,288 bytes cover the site-discovery and
proxy-routing code added afterwards.

## Measured pins refreshed

| Pin | Before | After |
|---|---|---|
| `fetch` definition bytes | 9,013 | 10,837 |
| `fetch` input schema bytes | 8,109 | 9,866 |
| `fetch` schema definitions | 12 | 13 |
| Full+full advertisement fingerprint | `c98316a5061399d6` | `01f2a66c4f3ace23` |
| Full+full advertisement bytes | 31,509 | 33,345 |
| Full+compact advertisement fingerprint | `bcc94e8b6857744f` | `9f62436a05fcf31e` |
| Full+compact advertisement bytes | 19,282 | 20,871 |
| Tool-surface byte reduction | -46,550 | -44,714 |
| Tool-surface token reduction | -11,638 | -11,179 |
| `src/core/config.rs` ratchet lines | 1,910 | 2,035 |

The schema growth is structural: a third `fetch` item type with five
properties plus the selector and attribute definitions. The total advertisement
still satisfies the ten-tool baseline slimming test.

## Verification

| Command | Outcome |
|---|---|
| `RUSTUP_TOOLCHAIN=1.89 make check` | pass: fmt, clippy, no-default check, all-features tests, hygiene, dependency policy, packaging |
| `make docs-check` | pass |
| `make packaging-check` | pass (workflow pins, packaged assets, planning consistency, eggpack qualifier, egress contract) |
| `cargo test --locked --all-features --test fetch_proxy_route` | pass: 7 |
| `cargo test --locked --all-features --test proxy_route_feasibility` | pass: 12 |
| `cargo test --locked --features mock --test config_validation` | pass: 100 |

A bare `make check` under the machine's default Rust 1.100 reports seven
newer-lint findings in untouched modules (`browser/intercept.rs`,
`classify.rs`, `challenge/altcha.rs`, `challenge/anubis.rs`,
`meta/local/scan.rs`). Under the repository-pinned 1.89 toolchain the same
gate passes with no findings, so this is toolchain drift, not a change defect.

## Limitations

- All proxy evidence is loopback-only; no hosted proxy, PAC, or real
  credentials are exercised.
- Size numbers come from one local macOS arm64 release build.
- An operator needing a proxy chain must run the loopback relay themselves; the
  relay is outside the trust boundary and outside this repository.
