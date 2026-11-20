# Local Workspace Deep Dive

**Path:** `src/core/local.rs`, `src/meta/local_backend.rs`, `src/meta/local_symbols.rs`, `src/meta/local_inventory.rs`, `src/meta/local_inventory_cache.rs`, `src/meta/local_ignore.rs`, `src/meta/safe_open.rs`, `src/meta/repo_mapper.rs`, `src/meta/local/mod.rs`
**Purpose:** Bounded filesystem search with a git-aware cached inventory, race-resistant file opening, deterministic structured symbols, and repo-map enrichment.

---

## Overview

CodeGG-managed Eggsearch 0.4.4+ can receive an authenticated private request
scope over stdio. For repository-oriented calls, including repo items in
`batch_fetch`, CodeGG supplies one authorized workspace root or an explicit
`root: null` local-disabled scope.
Eggsearch authenticates that metadata with the per-process
`CODEGG_EGGSEARCH_HOST_SCOPE_TOKEN`; missing or invalid authentication is an
error and never falls back to configured roots. The selected backend and
inventory cache are isolated by canonical root and full indexing configuration
in a bounded eight-entry in-memory LRU. Standalone calls with no host metadata
continue to use `[local].roots`. The metadata is not part of a model-facing
tool schema. This private extension is for a trusted locally spawned CodeGG
child only; it does not establish authority for HTTP or remote MCP clients.

The local workspace backend searches operator-configured filesystem roots
without cloning or indexing services. Each call ensures a cached inventory,
selects a bounded deterministic candidate set, reads files through
race-resistant opening, and emits `SourceCard`s with
`TrustLevel::LocalTrusted`. A query, path/file, or symbol match is required to
admit a file; source-role bonuses only rank candidates, while language and
repository identity are filters or boosts, never admission signals. The
`src/meta/local/mod.rs` facade records the ownership split: orchestration in
`local_backend`, git discovery in `local_inventory`, caching and the git runner
in `local_inventory_cache`, parsing in `local_symbols`, ignore matching in
`local_ignore`, safe opening in `safe_open`. No workspace code is executed
and no native plugins are loaded.

---

## Configuration (`src/core/local.rs`)

`LocalConfig` (`[local]`, disabled by default, roots canonicalized at
startup):

- `enabled`, `roots`
- `max_file_bytes` (default 1_048_576) — per-file read cap for non-PDF workspace fetches
- `pdf_max_file_bytes` (default 16_777_216, range 1..=268_435_456) — separate cap for local PDF reads
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
   scores each viable entry once, returning the unchanged bounded deterministic
   first-pass set (budget `max_results * 2`).
3. Validate and read candidates through `safe_read_file()` under
   `max_file_bytes`, then score path, text, language, and symbol matches.
4. If fewer than `max_results` matches were admitted, the request has not
   timed out, and `files_scanned < max_indexed_files`, run one second pass over
   the remaining inventory in deterministic relative-path order. It skips
   `(root_index, relative_path)` entries already read. The first-pass matches
   and ordering are retained; second-pass matches fill the remaining result slots.
5. The second pass is bounded by 512 files read, 8 MiB of file contents,
   1.5 seconds, the request deadline, `max_indexed_files`, and exhaustion.
   Query and symbol hints participate in candidate filtering; symbol results
   still use the structured parser first and regex fallback for provenance.
6. Convert to `SourceCard`s (`trust: LocalTrusted`, `local_repo_match`
   with root path, remote host/owner/repo, branch, commit, dirty state,
   plus file classification and deterministic `workspace_id`).
7. Report `InventoryTelemetry`, including `freshness_confidence`
   (`FreshnessConfidence`: `High`, `Medium`, `Low`).

`local_result_budget()` caps local merging at half the effective max
results. Internally, `ScanOutcome::Exhausted` means the eligible inventory was
completed; `ScanOutcome::CutShort` means a scan bound stopped it.
`local_search_truncated` is reported only for a cut-short scan, dropped returned
matches, or inventory truncation. A completed scan with zero cards reports
`local_search_completed_no_matches`; timeout remains a separate warning and
prevents the no-match warning. Bounded first-pass candidate selection alone is
not truncation when the second pass completes. The `SymbolBackend` trait
defines `find_symbols()`, `capabilities()`, `find_definition()`,
`find_references()` (defaulting to `local_symbols::find_references()`), and
`find_enclosing()` / `find_implementors()` where supported.
`RegexSymbolBackend` preserves compiled-pattern behavior;
`StructuredSymbolBackend` runs the deterministic parser first and degrades to
regex when parsing is disabled, unsupported, fails, or exceeds budget. There is
no persistent symbol index; recall remains bounded by the scan ceilings.

---

## Inventory Cache and Git-Aware Fast Path

`InventoryEntry { root_index, relative_path, absolute_path, size, language,
role, is_binary, mtime_secs, fingerprint }` values are grouped per root in
`RootInventory { root_index, root_path, entries, built_at, … }` inside the
shared `WorkspaceInventory`. `build_inventory()` prefers the git fast path
— `git ls-files` via `run_bounded_command()` — and falls back to the native
walker for non-git roots, timeouts, or cap breaches.

`run_bounded_command()` — a thin `local_inventory_cache` wrapper over
`src/process.rs` — enforces the 5s `GIT_OP_TIMEOUT` (from
`local_inventory.rs`), 16MB stdout cap (`GIT_STDOUT_CAP`), 64KB stderr cap
(`GIT_STDERR_CAP`), concurrent drainage, and process-group kill
(`ProcessTerminationController`, now owned by `src/process.rs` and reused by
startup and integration probes); `CommandTermination` is `Exited`, `TimedOut`,
`StdoutLimitExceeded`, `StderrLimitExceeded`, `SpawnFailed`, or `Signaled`.

The second-pass literal hint is built with bounded `git grep -l -z -i -F
--untracked`, the configured `--exclude-standard` or
`--no-exclude-standard`, and `-e` patterns for the lowercased query, query
tokens (up to eight patterns), and symbol hint. Its 2s configured maximum is
further capped to half of the 1.5s second-pass budget for each hint, with a
4 MiB stdout cap. NUL-separated paths are only a skip filter: every admitted entry
still passes `validate_entry()` and `safe_read_file()`. A path outside the hint
set remains eligible if a query token appears in its lowercased relative path
or a path/file hint was provided. Non-git roots, spawn/status/timeout/cap
failures produce no hint set and walk every remaining candidate. If no hint set
is available, symbol-only searches also walk the remaining inventory; there is
no persistent symbol index.

Freshness uses two clocks: `FRESHNESS_PROBE_INTERVAL` (30s status-hash
probe via `probe_needs_rebuild()`) and `INVENTORY_REBUILD_TTL` (300s full
rebuild via `needs_rebuild()`). Warm searches share the cached inventory
through `Arc` ownership. `validate_entry()` rejects deleted files (mtime
mismatch), oversized files, and disallowed symlinks; `score_inventory_entry()`
ranks candidates with filename-aware adjustments.

`local_inventory_cache.rs` is the one file here above the ordinary 1,600-line
/ 80 KB ceiling: it carries a ratchet in `maintenance.md` (1,800 lines /
60,000 bytes), currently ~1,695 lines / 53 KB. Its next slice is cache ownership
moving under `local/`; bounded process mechanics already moved to
`src/process.rs`.

---

## Git Worktree Discovery (`src/meta/local_inventory.rs`)

Identity resolution reads Git config files directly without invoking Git for
identity and normalizes remotes with `normalize_remote_url()` into
`NormalizedRepoId` (HTTPS, SSH, SCP, and git forms; `parse_url_form()` /
`parse_scp_form()`), enumerated by `read_remotes_from_config()`. Resolution is
deterministic: a `.git` directory uses its own `config`; a `.git` file first
resolves the named gitdir and uses its `config` when present; if that
per-worktree config is absent, its `commondir` file resolves the shared
configuration directory (relative to the per-worktree gitdir), and the shared
`config` supplies remote identity. If shared config resolution is unavailable,
lookup falls back to the per-worktree gitdir config and then to no remotes. The
small gitfile, commondir, and config reads are bounded; paths are canonicalized
before following indirections. `resolve_git_dir()` locates the git directory,
and three narrow readers report worktree state: `read_head_branch()`,
`read_head_commit()` (HEAD SHA, handling detached HEAD, packed-refs, and
gitfile worktrees), and `detect_dirty_state()`. The inventory therefore knows
which worktree each root belongs to without shelling out for identity on every
search.

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
- Windows (`follow_symlinks=false`): every component is
  `symlink_metadata`-checked at walk time and links are rejected with
  `SymlinkDetected`; the root itself is followed, matching the unix root
  policy.

Pre-walk validation rejects empty, absolute, `..`-traversal, and
null-byte components plus hidden components unless `include_hidden` is set.
On Linux the final descriptor is `fstat`-checked for regular file type
and size limits, eliminating TOCTOU races between validation and open;
on Windows the walk-time link checks leave a documented check-to-open
swap window (no atomic no-follow open in std), contained by the
user-owned jail-root trust boundary. Intermediate links are rejected
fail-closed everywhere; Windows reports `SymlinkDetected`, macOS
reports `Io` (`ENOTDIR` from `O_NOFOLLOW`), and Linux rejects from the
untouched `openat2`/`openat` path with a platform-specific variant.
`SafeOpenError` covers `Empty`, `AbsolutePath`, `PathTraversal`,
`NullByte`, `NotFound`, `SymlinkDetected`, `NotAFile`, `FileTooLarge`,
`FileContentLimitExceeded`, `RootOpenFailed`,
`SafeSymlinkFollowingUnsupported`, and `Io`; `safe_read_file()` layers
bounded reads on top.

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
`Cargo.toml`, `package.json`, Python manifests, `go.mod`, JVM, `Gemfile` /
`*.gemspec`, `composer.json`, `mix.exs`),
`language_distribution`, top-level `modules`, entrypoint candidates
(`src/main.rs`, `src/lib.rs`, `main.py`, `index.ts`, `main.go`, …),
capped `top_symbols`, heuristic `test_relationships`, CI/Dockerfile/Makefile
`build_configs`, and `structure_truncated` on any cap breach. Caps come from
the `[local]` budgets; the scan never executes workspace code.

Local `repo_fetch` validates the requested path, then reads it with
`safe_read_file()` relative to the selected workspace root; it never reopens
the canonicalized pathname. With `prefer_local: true`, an enabled local backend
first resolves the requested host/owner/repository against local checkout
identities and fetches the path from a matching workspace. If no local checkout
matches, the remote fetch remains the fallback and the response warning states
that the local copy was skipped and why. An authorized host scope preserves its
existing validation error when its required local workspace or matching repo is
not available. It identifies PDFs by case-insensitive `.pdf` extension or a
safely read `%PDF-` prefix. PDF bytes use `pdf_max_file_bytes` and the existing
PDF feature plus `[fetch].pdf_enabled` gates, then flow through the shared
byte-slice extractor. `repo_fetch` and batch repo items do not accept PDF page
selection. PDF remains in `BINARY_EXTENSIONS`, so local indexing and repo-map
filtering are unchanged. Local `repo_map` structure enrichment uses the same
bounded opener for manifest, source, and test contents. Its structural walk
skips symlink entries even when local search permits following symlinks, so map
enrichment does not enumerate or read a linked tree outside the workspace.

---

## Cache Identity and Local Gating

`fetch::cache::CacheScope` is `Anonymous` or `Profile(ProfileId)`, where
`ProfileId::opaque(id)` is an identity constructor that *labels* an id the
caller has already made opaque — it performs no hashing or derivation. The
opaque property is a caller contract: callers pass the opaque ID, never a
display name (`web_fetch` builds
`CacheScope::Profile(ProfileId::opaque(id.clone()))`, and browser profile IDs
are already `prof_<16-hex>`). `build_raw_cache_key()`
scopes raw fetch entries so profiles never share cached bodies.

Local results enter mixed-provider flows only through the `include_local`
flag. For unscoped requests, an omitted value includes local results by
default; when resolved hints name a host, owner, repository, or organization
(including a `repo:owner/name` token), local search runs only when
`include_local: true` is explicit. Explicit `false` suppresses local search in
every scope, and the backend must also be enabled. The repo adapter merges
local cards inside `local_result_budget()`.

When `[search].mode = "off"`, `repo_search` may run only if the local backend
is enabled and local results were not explicitly disabled. The MCP endpoint
passes a call-scoped remote execution denial to the adapter; this is separate
from provider selection, where an empty provider list still means automatic
selection in live mode. Mode-Off requests skip provider engines, package
registry resolution, and package advisory lookup. Responses identify the
local workspace as queried, report remote sources skipped by policy, and keep
local source text marked untrusted. Missing, disabled, or opted-out local
search is rejected before adapter dispatch.

---

**Back to:** [overview.md](overview.md)
