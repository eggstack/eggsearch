# Local Workspace Deep Dive

**Path:** `src/core/local.rs`, `src/meta/local_backend.rs`, `src/meta/local_symbols.rs`, `src/meta/local_inventory.rs`, `src/meta/local_inventory_cache.rs`, `src/meta/local_ignore.rs`, `src/meta/safe_open.rs`, `src/meta/repo_mapper.rs`, `src/meta/local/mod.rs`
**Purpose:** Bounded filesystem search with a git-aware cached inventory, race-resistant file opening, deterministic structured symbols, and repo-map enrichment.

---

## Overview

The local workspace backend searches operator-configured filesystem roots
without cloning or indexing services. Each call ensures a cached inventory,
selects a bounded deterministic candidate set, reads files through
race-resistant opening, scores path/text/language/symbol matches, and emits
`SourceCard`s with `TrustLevel::LocalTrusted`. The `src/meta/local/mod.rs`
facade records the ownership split: orchestration in `local_backend`, git
discovery in `local_inventory`, caching and the git runner in
`local_inventory_cache`, parsing in `local_symbols`, ignore matching in
`local_ignore`, safe opening in `safe_open`. No workspace code is executed
and no native plugins are loaded.

---

## Configuration (`src/core/local.rs`)

`LocalConfig` (`[local]`, disabled by default, roots canonicalized at
startup):

- `enabled`, `roots`
- `max_file_bytes` (default 1_048_576) — per-file read cap
- `max_indexed_files` (default 50_000) — bounded scan
- `include_hidden` (default false), `respect_gitignore` (default true),
  `follow_symlinks` (default false)
- `structured_symbols` (default true)
- `max_parse_bytes` (default 262_144), `max_symbols_per_file` (default 256),
  `max_structured_files` (default 200), `max_total_symbols` (default 5_000)
- `repo_map_structure_cap` (default 500)

Path policy helpers: `should_skip_component` (`SKIP_DIRS` such as `target/`,
`node_modules/`, `.git/`, `__pycache__/`), `is_binary_extension`,
`language_from_extension`, `is_eligible_for_indexing`,
`is_git_path_eligible`.

`LocalSearchRequest { query, path, language, file, symbol, max_results,
timeout_ms }` narrows matching; `LocalFileEntry { path, relative_path,
root_index, size, language }` is the walk-level record.

---

## Filesystem Backend (`src/meta/local_backend.rs`)

Search flow per call:

1. Ensure the `WorkspaceInventory` is built (auto-build on cache miss).
2. `select_inventory_candidates()` filters by language/file/path hints and
   scores each viable entry once, returning a bounded deterministic top-K
   set (budget `max_results * 2`).
3. Read candidates through `safe_read_file()` under `max_file_bytes`.
4. Score path, text, language, and symbol matches; structured definitions
   outrank regex matches while preserving lexical fallback ordering.
5. Convert to `SourceCard`s (`trust: LocalTrusted`, `local_repo_match`
   with root path, remote host/owner/repo, branch, commit, dirty state,
   plus file classification and deterministic `workspace_id`).
6. Report `InventoryTelemetry`, including `freshness_confidence`
   (`FreshnessConfidence`: `High`, `Medium`, `Low`).

`local_result_budget()` caps local merging at half the effective max
results. The `SymbolBackend` trait defines `find_symbols()`,
`capabilities()`, `find_definition()`, `find_references()` (defaulting to
`local_symbols::find_references()`), and `find_enclosing()` /
`find_implementors()` where supported. `RegexSymbolBackend` preserves the
compiled-pattern behavior; `StructuredSymbolBackend` runs the deterministic
parser first and degrades to regex when parsing is disabled, unsupported,
fails, or exceeds budget.

---

## Inventory Cache and Git-Aware Fast Path

`InventoryEntry { root_index, relative_path, absolute_path, size, language,
role, is_binary, mtime_secs, fingerprint }` values are grouped per root in
`RootInventory { root_index, root_path, entries, built_at, … }` inside the
shared `WorkspaceInventory`. `build_inventory()` prefers the git fast path
— `git ls-files` via `run_bounded_command()` — and falls back to the native
walker for non-git roots, timeouts, or cap breaches.

`run_bounded_command()` enforces a 5s timeout, 16MB stdout cap
(`GIT_STDOUT_CAP`), 64KB stderr cap (`GIT_STDERR_CAP`), concurrent
drainage, and process-group kill (`ProcessTerminationController`);
`CommandTermination` is `Exited`, `TimedOut`, `StdoutLimitExceeded`,
`StderrLimitExceeded`, `SpawnFailed`, or `Signaled`.

Freshness uses two clocks: `FRESHNESS_PROBE_INTERVAL` (30s status-hash
probe via `probe_needs_rebuild()`) and `INVENTORY_REBUILD_TTL` (300s full
rebuild via `needs_rebuild()`). Warm searches share the cached inventory
through `Arc` ownership. `validate_entry()` rejects deleted files (mtime
mismatch), oversized files, and disallowed symlinks; `score_inventory_entry()`
ranks candidates with filename-aware adjustments.

---

## Git Worktree Discovery (`src/meta/local_inventory.rs`)

Identity resolution reads `.git/config` directly (no subprocess) and
normalizes remotes with `normalize_remote_url()` into `NormalizedRepoId`
(HTTPS, SSH, SCP, and git forms; `parse_url_form()` / `parse_scp_form()`),
enumerated by `read_remotes_from_config()`. `resolve_git_dir()` locates the
git directory and `read_head_commit()` reports branch, HEAD SHA, and dirty
state. The inventory therefore knows which worktree each root belongs to
without shelling out for identity on every search.

---

## Ignore Rules (`src/meta/local_ignore.rs`)

`IgnoreStack` implements a minimal layered `.gitignore` matcher: comments,
negation (`!`), anchored patterns, directory-only patterns, `*` / `**` /
`?` wildcards, nested `.gitignore` files, and parent-to-child evaluation.
It applies only when `respect_gitignore` is true.

---

## Race-Resistant Opening (`src/meta/safe_open.rs`)

`safe_open_relative()` walks the relative path component-by-component from a
directory file descriptor:

- Linux, `follow_symlinks=false`: `openat2` with `RESOLVE_BENEATH |
  RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS`.
- Linux, `follow_symlinks=true`: `RESOLVE_BENEATH |
  RESOLVE_NO_MAGICLINKS`, letting the kernel enforce containment while
  allowing in-bounds symlinks.
- `openat` with `O_NOFOLLOW` only as a fallback when `openat2` is
  unavailable and following is disabled.
- Non-Linux with `follow_symlinks=true`: `SafeSymlinkFollowingUnsupported`
  — there is no race-safe containment primitive, so the backend reports
  instead of approximating.

Pre-walk validation rejects empty, absolute, `..`-traversal, and
null-byte components plus hidden components unless `include_hidden` is set.
The final descriptor is `fstat`-checked for regular file type and size
limits, eliminating TOCTOU races between validation and open.
`SafeOpenError` covers `Empty`, `AbsolutePath`, `PathTraversal`,
`NullByte`, `NotFound`, `RootOpenFailed`, and
`SafeSymlinkFollowingUnsupported`; `safe_read_file()` layers bounded reads
on top.

---

## Structured Symbols (`src/meta/local_symbols.rs`)

The dependency-free deterministic parser covers Rust, Python,
JavaScript/TypeScript, and Go behind the `SymbolBackend` abstraction,
extracting functions/methods, structs/classes/types, traits/interfaces,
impl relationships, modules/namespaces, imports, and test items with
bounded span estimates. `StructuredSymbol { name, kind, line_start,
line_end, container, visibility, is_test, relationship }` uses
`SymbolKind` from `core::code_evidence`; `SymbolProvenance` marks matches
as `structured` or `regex_fallback` via `as_str()`.

Budgets mirror `[local]`: `DEFAULT_MAX_PARSE_BYTES` (262_144),
`DEFAULT_MAX_SYMBOLS_PER_FILE` (256), `DEFAULT_MAX_STRUCTURED_FILES`
(200), `DEFAULT_MAX_TOTAL_SYMBOLS` (5_000),
`DEFAULT_MAX_SYMBOLS_PER_REPO_MAP_FILE` (32),
`DEFAULT_REPO_MAP_STRUCTURE_CAP` (500). Breaches degrade to
partial/regex evidence, never fatal failures. `SymbolBackendCapabilities`
reports `supports_definitions` / `supports_references` /
`supports_enclosing` / `supports_implementors`, the `structured_languages`
list, and `regex_fallback_available`.

`SymbolInventoryCache` keys records by path plus xxh3 content hash
(`SymbolRecord { path, language, name, kind, line_start, line_end,
container, visibility, relationship_hints, content_hash, version }`,
`SYMBOL_RECORD_VERSION = 1`); hash mismatches invalidate so stale offsets
are never trusted.

`related_test_hints()` emits deterministic heuristic source-to-test hints
ordered by `TestHintConfidence` (`Syntax` > `Path` > `NameReference` >
`Package`) with explicit reasons and no coverage claims.

---

## Repo-Map Enrichment (`src/meta/repo_mapper.rs`)

`populate_structure_from_local_checkout()` fills bounded additive
`RepoMapResponse` fields from `build_local_structure()`, returning
`LocalStructure { packages, language_distribution, modules, entrypoints,
top_symbols, test_relationships, build_configs, truncated }`:
manifest-derived `packages` (`manifest_ecosystem()` recognizes
`Cargo.toml`, `package.json`, Python manifests, `go.mod`, JVM, `Gemfile`),
`language_distribution`, top-level `modules`, entrypoint candidates
(`src/main.rs`, `src/lib.rs`, `main.py`, `index.ts`, `main.go`, …),
capped `top_symbols`, heuristic `test_relationships`, CI/Dockerfile/Makefile
`build_configs`, and `structure_truncated` on any cap breach. Caps come from
the `[local]` budgets; the scan never executes workspace code.

---

## Cache Identity and Local Gating

`fetch::cache::CacheScope` is `Anonymous` or `Profile(ProfileId)`, where
`ProfileId::opaque(id)` derives an opaque, non-reversible profile key —
callers pass the opaque id, never a display name (`web_fetch` builds
`CacheScope::Profile(ProfileId::opaque(id.clone()))`). `build_raw_cache_key()`
scopes raw fetch entries so profiles never share cached bodies.

Local results enter mixed-provider flows only through the `include_local`
flag: `RepoSearchRequest.include_local` (and `repo_search` tool arg
`include_local: Option<bool>`) defaults on via `include_local_enabled()`
(`unwrap_or(true)`), and the repo adapter merges local cards only when the
backend `is_enabled()` and `req.include_local_enabled()` both hold, inside
`local_result_budget()`.

---

**Back to:** [overview.md](overview.md)
