# ADR-0021: Optional Local Knowledge-Delta Evidence Index

Status: accepted
Date: 2026-11-19
Accepted: 2026-11-20
Revised: 2026-11-20 — accepted. Engine is bundled rusqlite. This record does not implement the index.
Decision owners: project maintainers
Baseline: `0f52c8fc812c99e8aee75f515f1b1d8dd32a9789` on `feat/auto-20261117-044c4885`

Related: ADR-0002 (eggfetch transport), ADR-0006 (deterministic retrieval),
ADR-0020 (accepted knowledge-brief tool).
Affected specification: `plans/000-long-term-specification.md` §§3–4, 6–8.
Affected subsystem: `plans/subsystems/knowledge-delta-roadmap.md`.
Implementation: `plans/implementation/knowledge-delta/002-local-evidence-index.md`.

## Context

The existing process-local search and fetch caches are short-lived
performance infrastructure, not durable records of changed external facts.
A local knowledge-delta index would preserve dated source evidence across
sessions so a later milestone can find a related change the caller did not
name. Persistence creates retention, locking, migration, corruption,
authorization, and release-footprint obligations. The canonical
specification excludes recursive crawling, autonomous browser interaction,
mandatory vector indexing, and multiple binary SKUs.

Fetch cache already partitions browser-profile bodies by an opaque
`prof_` ID. `should_cache_response` refuses `private` and `no-store`
bodies in the anonymous scope. Config lives under
`$XDG_CONFIG_HOME/eggsearch/`. There is no general data directory.

## Alternatives

1. No persistent index; always search live. Lowest complexity but no
   historical evidence recall. This remains the default.
2. Append-only JSON files plus a lexical scan. Simple to ship but weaker
   for atomic updates, constrained queries, deduplication, and crash
   recovery.
3. **Accepted:** opt-in SQLite with FTS5, bounded document and event
   metadata, and deterministic lexical indexing, with a versioned migration
   path. The engine is bundled rusqlite. The spike did not complete the
   original precondition; selection is the acceptance below.
4. Vector DB or embedded LLM extraction. Rejected as a required dependency.
   Neither solves source dating or veracity.

## Acceptance precondition

The original precondition was a recorded footprint spike that qualifies
bundled versus system SQLite under Rust 1.89 on the default release
targets (Linux, macOS, and Windows). It was to measure added stripped
binary size, startup resident memory, and schema-init time against the
current default-binary budget. The experimental ARMv7 wheel is not a
hard gate; mark that physical run unrun. The spike is not M002 work. If
the spike misses a written budget, the decision would have been
alternative 2, the JSON store. No second SKU and no new eggsearch Cargo
feature. The 2026-11-20 spike did not meet this precondition. The
acceptance below waives the unmet Linux and Windows runs. It does not
invent a budget score, and it does not start M002.

## Proposed decision (requires acceptance)

- The index is optional infrastructure, disabled by default, and never
  required for `knowledge_brief` to perform bounded live discovery.
  Enabling it does not change a `live` call. `live` neither reads nor
  writes the index.
- Additive modes, rejected by M001 until this ADR is accepted:
  `retain` writes this call's already selected public evidence and does
  not change retrieval; `prefer_live` runs live search and then unions
  index hits by stable ID, labeling `live_checked` or `index_only`;
  `index` reads the index only and says freshness is unverified.
  `retain` is a boolean, not a `refresh` value. `prefer_live` is
  the union mode and `index` is the local-only mode; both are
  `refresh` values, and neither is the default. An operator setting
  cannot write on a call the caller did not ask to retain.
- Store document identity, canonical URL, content hash, timestamps with
  the ADR-0020 basis and precision, evidence excerpts and spans, the M001
  `kdelta_` ID, source provenance, and schema version. Do not store user
  prompts, credentials, session cookies, or personal browsing history.
  Do not add a feed-subscription table.
- Index a document only when the fetch used `CacheScope::Anonymous`,
  `browser_profile` was absent, and `should_cache_response` would
  accept the response. Refuse `private`, `no-store`, pass-cookie
  replays, and profile-scoped bodies.
- The path is explicit, or `<config_dir>/knowledge-delta/<partition>/`.
  `partition` is `anonymous` or the opaque `prof_<fnv>` ID, never
  the display name. Do not copy fetch-cache contents, and do not treat
  fetch-cache TTL as event freshness.
- This decision does not authorize feed crawling, a refresh command, an
  import daemon, or a scheduler. M002 persists only evidence a live
  `knowledge_brief` already selected. A later milestone must not add
  host discovery from page links under this ADR.
- Bound index size, document count, excerpt bytes, and time. Evict
  deterministically. On disk full, corrupt records, or lock contention,
  report the degraded state. Never report a failed read as a fresh search.
- Use a versioned schema, transactional writes, one writer, and read-safe
  access. On an incompatible schema, fail the index closed and preserve
  existing data. A `live` call is unchanged.
- File permissions are restrictive. No network listener beyond the
  existing loopback-only MCP transport. Document deletion and disable.
- Content retrieved from the index stays external-untrusted. Persistence
  does not promote trust. The index is not a crawler, an answer generator,
  or an authority on what a model knows.

## Dependency and scope

M002 has a hard dependency on M001 closure and on acceptance of this ADR,
which includes the spike record. M002 implements the named engine. It does
not choose the engine. No canonical long-term amendment is required for an
index that remains optional, deterministic, and non-crawling. If
implementation needs recursive traversal, a storage network service, or a
mandatory runtime dependency that breaks the release constraints, stop
for a new decision.

## Verification and consequences

M002 must prove schema migration and recovery, lexical recall, preserved
date precision, retention and eviction, no write on `live`, refusal of
authenticated bodies, concurrent use, offline semantics, no-network tests,
and default-target package sizes. Unrun targets are named. Operational
docs cover path, backup, delete, disk use, and disable.

Positive: persistent recall of already selected public evidence without a
model. Costs: storage compatibility, migrations, and a privacy review.
Those are gates, not assumed benefits.

## Footprint spike record

Recorded 2026-11-20. This record does not accept the ADR. Linux and
Windows were not measured. No numeric default-binary budget was found,
so this record does not score a pass or a miss. If a later complete
spike misses a written budget, the decision remains alternative 2. No
budget number is invented here.

Host: macOS 26.6.2, `aarch64-apple-darwin`, `rustc 1.89.0
(29483883e 2025-08-04)`. The harness is a throwaway crate under `/tmp`,
not committed and not linked into eggsearch. `rusqlite` 0.37.0 and
`libsqlite3-sys` 0.35.0. Its release profile matched the crate: thin
LTO, `codegen-units = 1`, `strip = true`. The baseline binary prints
zeros. The spike opens a temporary database, creates one table and an
FTS5 virtual table, prints elapsed microseconds and
`sqlite_compileoption_used('ENABLE_FTS5')`, then deletes the directory.
Size is `stat` bytes. Resident memory is five `/usr/bin/time -l`
maximum resident set size samples. The first schema-init sample is cold.
The harness delta is not an eggsearch relink, so LTO against the real
crate can differ.

Native `aarch64-apple-darwin`. Bundled links only `libSystem` and
reports `fts5=1`. The system column is the OS library,
`/usr/lib/libsqlite3.dylib` (compatibility 9.0.0, current 382.0.0),
also `fts5=1`. That path is a dyld shared-cache install name. Reaching
it required an empty `PKG_CONFIG_LIBDIR`: default `pkg-config` on this
host selects MacPorts SQLite 3.51.2 at the absolute path
`/opt/local/lib/libsqlite3.0.dylib`. That MacPorts link is not a
relocatable release dependency. Both dynamic links have the same
stripped size.

| arm64 build | bytes | added | schema-init microseconds | maximum RSS |
|---|---:|---:|---|---|
| baseline | 333,568 | — | 0 | 1,523,712 ×5 |
| bundled | 2,049,200 | +1,715,632 | 1552, 1086, 1058, 1061, 1047 | 3,162,112; then 3,178,496 ×4 |
| system, OS | 370,544 | +36,976 | 5729, 1966, 1653, 1990, 1744 | 2,359,296; 2,359,296; 2,342,912; 2,359,296; 2,359,296 |

`x86_64-apple-darwin` was cross-compiled and executed under Rosetta 2
on this arm64 host. Size is the Mach-O size. RSS and schema-init are
not native x86_64 measurements. Bundled links only `libSystem`.
System links the same OS install name. Both report `fts5=1`.

| x86_64 build | bytes | added | schema-init microseconds | maximum RSS |
|---|---:|---:|---|---|
| baseline | 331,128 | — | 0 | 3,211,264; 3,203,072; 3,203,072; 3,211,264; 3,203,072 |
| bundled | 2,046,504 | +1,715,376 | 2026, 1796, 1408, 1414, 1558 | 4,931,584; 4,972,544; 4,874,240; 4,874,240; 4,915,200 |
| system, OS | 376,104 | +44,976 | 18011, 2444, 2267, 2618, 2649 | 3,788,800; 3,784,704; 3,801,088; 3,821,568; 3,801,088 |

The current default eggsearch binary, same toolchain, is not a SQLite
build. Command: `cargo +1.89.0 build --locked --release` with default
features `pdf` and `browser`, isolated target directory. The release
profile finished in 3m 38s. Stripped Mach-O arm64, 25,161,184 bytes.
It links CoreFoundation, libiconv, and libSystem. Five `eggsearch
--version` RSS samples: 8,847,360; 8,814,592; 8,749,056; 8,765,440;
8,765,440. That is `--version`, not an MCP serve startup, and it does
not include SQLite.

Unrun, and therefore not qualified:

- `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`: the
  rustup targets are installed. No `x86_64-linux-gnu-gcc`, no
  `aarch64-linux-gnu-gcc`, no container runtime, and no `qemu-x86_64`.
  This host cannot link or execute them. The host compiler is
  `/usr/bin/clang`.
- `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`: no MSVC
  linker, and this host cannot execute Windows binaries.
  `x86_64-w64-mingw32-gcc` is present and is not the release target.
- `armv7-unknown-linux-gnueabihf`: unrun. Not a hard gate. No
  `arm-linux-gnueabihf-gcc`.

`plans/000` §7, `architecture/packaging.md`, and the performance
roadmap state no numeric default-binary cap. Phase 22 records a
historical 18,744,624-byte default binary on Rust 1.98.1 /
`x86_64-apple-darwin`. The retrieval-quality matrix records 28,081,112
bytes for a PDF+browser build on another host. Neither figure is this
measurement, and neither is a cap.

These measurements do not by themselves accept this ADR. Acceptance is
the maintainer decision in the next section. M002 must not start.

## Acceptance

Accepted 2026-11-20 by maintainer decision. Alternative 3 is selected.
The engine is `rusqlite` with its bundled SQLite and FTS5, compiled into
the one default binary. There is no second SKU and no new eggsearch
Cargo feature. System SQLite is not the release link: Linux and Windows
system libraries were not measured, and the macOS `pkg-config` path was
not relocatable. The index stays disabled by default.

The macOS spike did not qualify Linux or Windows, and no numeric budget
was scored. The maintainer accepts bundled rusqlite anyway. Those unrun
targets remain an operational gate on calling M002 shippable. They do
not reopen this decision. This acceptance does not implement the index,
and M002 must not start until M001 is closed.
