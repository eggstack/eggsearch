# Dependency security policy

`make dependency-policy` runs cargo-deny 0.20.2 against the complete feature
graph. Known vulnerability and unsound advisories fail the gate; yanked
versions, unapproved registries, git sources, and licenses outside the
allowlist fail it. Duplicate versions are reported for review and do not fail
by themselves.

The license allowlist in `deny.toml` is based on the current all-features graph.
It contains permissive licenses used by direct and transitive packages, plus
MPL-2.0 and CDLA-Permissive-2.0. New license families require review before
they are added. A package offering multiple license alternatives is accepted
when at least one listed license applies.

## Scoped unmaintained notices

These exact RustSec notices are temporarily ignored. Each exception is limited
to one notice, names its dependency path and capability context, and must be
reviewed by 2026-11-27.

| Advisory | Dependency path and context | Reason | Review by |
|---|---|---|---|
| RUSTSEC-2025-0052 | `eggsearch` dev dependency -> `httpmock` -> `async-std`; tests only | `httpmock` is used by deterministic integration tests and has no compatible replacement already in the tree. No production binary includes this path. | 2026-11-27 |
| RUSTSEC-2025-0057 | `eggsearch` -> `scraper` -> `selectors` -> `fxhash`; default runtime | `scraper` owns this transitive dependency. Replacing the parser or maintaining a fork would enlarge this bounded hardening change; track scraper upstream and review an upgrade path by the stated date. | 2026-11-27 |
| RUSTSEC-2026-0192 | `eggsearch` optional `pdf` feature -> `lopdf` -> `ttf-parser`; not in the default binary | Removing `lopdf` would remove supported PDF extraction, and no patched `ttf-parser` release exists. Track the upstream `ttf-parser` issue and review the dependency path by the stated date. | 2026-11-27 |
| RUSTSEC-2026-0441 | `eggsearch` dev dependency -> `httpmock` -> `async-std` -> `pin-utils`; tests only | The upstream repository was archived after 0.1.1 and no safe upgrade exists. `pin-utils` is reached only through the test-only `httpmock` harness, so no shipped binary includes this path. Track `httpmock`/`async-std` for a replacement and review by the stated date. | 2026-12-31 |

The waiver guard in `packaging/check-dependency-policy.py` requires every
ignored advisory to appear here with a future review date no more than 60 days
away. Reachable vulnerability advisories are never accepted through this
mechanism. Do not add wildcard or crate-wide ignores.

## Remediated lockfile findings

- `quick-xml` is constrained to `>=0.41` for the checked-attribute parser fix.
- `rustls` resolves to `0.23.46` or newer in the 0.23 line.
- The yanked `chacha20 0.10.1` lock entry was replaced by `0.10.2`.
- `rmcp` no longer enables its reqwest HTTP client feature. Chromiumoxide is
  the only all-features reqwest dependency until its upstream optional
  HTTP-discovery seam is released.
- `rmcp` resolves to `3.9.0` (from `3.2.0`), clearing the OAuth SSRF filter,
  unbounded tool schema cache, and unbounded response body advisories. The
  deprecated `ServerInfo` alias is replaced by `ServerConfig`; feature names and
  MCP 2026-07-28 behaviour are unchanged.
- The yanked `async-executor 1.14.0` dev-dependency lock entry was replaced by
  `1.14.1`.
