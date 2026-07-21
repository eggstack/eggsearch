# Final Closure Pass: Production Readiness

Status: implementation handoff

Baseline reviewed: `70edcff866608e9a68d5da4e7d1f501ff483a059`

Scope: close the remaining production blockers in remote forge retrieval, repository provenance, local workspace execution and freshness, symlink-safe file access, and workflow-aware evidence output.

This is a closure pass, not another architecture phase. The repository already has the intended major components. The purpose of this plan is to make their safety and correctness claims true at the actual I/O boundaries, complete the remaining Phase 5 integration, and produce release evidence sufficient for a production-readiness decision.

---

## 1. Outcome

At completion, eggsearch must be suitable for long-running use as the retrieval substrate behind codegg and other coding agents.

The release candidate must satisfy all of the following:

1. Every forge response body, including metadata and error responses, is read through a hard byte limit.
2. Redirects cannot move credential-bearing requests to an unvalidated origin.
3. Custom forge endpoints are classified using both the configured hostname and resolved addresses.
4. Repository references, commits, trees, and entry object identifiers are represented separately and used correctly.
5. Generated repository URLs resolve to the requested repository state and do not treat blob or tree object IDs as commit IDs.
6. Git subprocesses cannot hang indefinitely, allocate unbounded output, deadlock on full pipes, or leave descendant processes behind.
7. Newly created untracked files and linked-worktree changes become visible without waiting for the five-minute inventory TTL.
8. Local content is read through a handle that preserves the validated file identity and cannot be redirected through a symlink race.
9. Evidence roles are present on returned cards rather than existing only in aggregate summaries.
10. Workflow coverage is computed from the actual requested workflow and actual retrieval outcomes.
11. Conflict metadata is emitted only between comparable evidence for the same canonical entity.
12. Next actions are derived from identified evidence gaps and contain valid tool arguments.
13. Deterministic CI, hardening, fuzz smoke, live-smoke, and performance evidence are captured for the final head.

---

## 2. Existing strengths to preserve

The implementation must preserve the behavior already delivered by Phases 1 through 5:

- bounded provider fan-out and request deadlines;
- deterministic source identifiers and ordering;
- prompt-injection framing and trust markers;
- partial-result behavior on provider failures;
- native repository tree retrieval for GitHub, GitLab, Gitea, Forgejo, and Codeberg;
- additive `repo_map.entries` alongside backward-compatible `root_entries`;
- local inventory-first search with native-walk fallback;
- tracked and untracked file enumeration;
- existing evidence-role, coverage, conflict, and retrieval-status public types;
- stable MCP tool names and additive response schema evolution;
- no network dependency in ordinary unit and integration tests.

Do not remove compatibility fields solely to simplify this pass.

---

## 3. Non-goals

The following are explicitly outside this closure pass:

- adding new search providers;
- adding a database-backed index;
- replacing regex symbol extraction with a full language server or parser stack;
- introducing a general plugin SDK;
- adding an HTTP server transport;
- redesigning the MCP tool catalog;
- adding model-based ranking or model-based conflict judgment;
- building a filesystem watcher service;
- changing codegg orchestration architecture;
- optimizing every large-monorepo case before correctness is established.

Any newly discovered broad feature request should be recorded separately and must not delay this closure unless it violates a release invariant in this plan.

---

## 4. Release invariants

These invariants are normative. Tests and documentation should use the same language.

### 4.1 Remote transport invariants

- No response body is consumed with unbounded `text()`, `bytes()`, `json()`, or equivalent helpers.
- The byte limit is enforced while streaming, before appending beyond the configured cap.
- UTF-8 validation occurs after bounded byte collection or through a stateful decoder; arbitrary network chunk boundaries cannot invalidate otherwise valid UTF-8.
- Redirects are disabled by default for forge API clients.
- If redirects are supported explicitly, every hop is revalidated before credentials or the next request are sent.
- Credentials are never sent over plaintext HTTP.
- Credentials are never forwarded cross-origin.
- Address classification covers literal IPv4, literal IPv6, DNS-resolved IPv4, DNS-resolved IPv6, IPv4-mapped IPv6, loopback, link-local, private, documentation, multicast, unspecified, carrier-grade NAT, and reserved ranges.
- Resolution failure produces a structured configuration or network failure; it does not silently bypass address policy.

### 4.2 Repository identity invariants

- `requested_ref` means the caller-supplied branch, tag, commit, or symbolic ref.
- `resolved_commit_sha` means a commit object identifier resolved by the provider.
- `tree_sha` means the root tree object identifier associated with that commit where available.
- `entry_object_sha` means the blob, tree, submodule, or symlink object identifier returned for one entry.
- A field named `commit_sha` must contain an actual commit SHA or be absent.
- A browser or raw permalink advertised as immutable must use `resolved_commit_sha`, not `entry_object_sha`.
- Provider fallback requests use the same requested or resolved ref as the primary request.
- Slash-containing refs are encoded as data, not interpreted as extra URL path segments.

### 4.3 Local execution invariants

- Subprocess timeout covers spawn, stdout drain, stderr drain, and process exit.
- Stdout and stderr are drained concurrently.
- Readers stop at their byte caps rather than reading to completion and truncating afterward.
- Exceeding a cap terminates and reaps the process.
- A timeout terminates the process and its relevant descendants.
- No auxiliary Git invocation bypasses the bounded runner.
- Git-path discovery supports ordinary repositories and linked worktrees.

### 4.4 Local file invariants

- A path validated under a workspace root is read from the same file identity.
- Intermediate path components cannot be swapped to symlinks between validation and open.
- The final path cannot be swapped to a symlink between validation and open.
- The file descriptor or handle is checked for regular-file type and size before reading.
- Content reads remain capped even if the file grows after inventory creation.
- Failure to obtain a race-resistant handle degrades safely; it does not fall back to an unsafe reopen.

### 4.5 Evidence invariants

- Every returned `SourceCard` has a deterministic evidence role when a role can be inferred.
- Coverage uses the workflow requested by the caller, not a hardcoded generic model.
- Retrieval success with zero matches is distinct from provider failure, timeout, policy skip, and not-applicable.
- A provider is not labeled as implementation evidence merely because it returned a result.
- Conflicts require at least two distinct sources or records for the same canonical entity and field.
- Multiple values inside one advisory are not automatically treated as cross-source conflicts.
- Next actions identify the evidence gap they address and explain why the proposed tool call is useful.

---

## 5. Delivery sequence

Implement in the following order. Later workstreams depend on the contracts established by earlier ones.

1. Workstream A — forge transport trust boundary.
2. Workstream B — repository provenance and URL correctness.
3. Workstream C — bounded Git process execution.
4. Workstream D — inventory freshness and race-resistant file access.
5. Workstream E — workflow-aware evidence integration.
6. Workstream F — verification, documentation, and release evidence.

Each workstream should land in an independently reviewable commit. Do not combine all work into one large closure commit.

---

# Workstream A — Forge Transport Trust Boundary

## A.1 Centralize forge HTTP client construction

Refactor forge API client construction into one policy-aware constructor.

Recommended location:

- `src/meta/forge_adapter.rs`, or
- a focused `src/meta/forge_transport.rs` module if the adapter becomes too large.

The constructor must define explicitly:

- connect timeout;
- whole-request timeout;
- redirect policy;
- user agent;
- proxy behavior, if inherited from the environment;
- DNS-resolution policy assumptions;
- TLS behavior;
- maximum idle connections where relevant.

Do not rely on reqwest defaults for redirect behavior.

### Required default

Use `Policy::none()` or equivalent for the forge client.

The provider API endpoints used by eggsearch do not require browser-style redirect following for normal operation. Treat redirects as a failure unless a provider-specific, tested exception is demonstrated.

### Structured redirect failure

Map HTTP 3xx responses to a stable forge failure class such as:

- `redirect_rejected`;
- `cross_origin_redirect_rejected` if the location can be parsed and is cross-origin;
- `redirect_target_invalid` for malformed locations.

Do not include credential material in error strings.

## A.2 Define one endpoint-policy model

Replace ad hoc hostname checks with one explicit policy input.

Suggested type:

```rust
pub struct ForgeEndpointPolicy {
    pub allow_loopback: bool,
    pub allow_private_network: bool,
    pub require_https: bool,
}
```

The exact name may vary, but policy must be explicit and testable.

Recommended production defaults:

- `allow_loopback = false`;
- `allow_private_network = false`;
- `require_https = true`.

Development and self-hosted forge use may opt in through configuration. Do not infer permission merely because the URL is localhost.

### Configuration requirements

Add or reuse clearly named configuration fields. Avoid silently overloading unrelated fetch settings unless the same semantics and threat model are genuinely intended.

Potential configuration shape:

```toml
[search.forge]
allow_loopback = false
allow_private_network = false
require_https = true
```

If the project already has an appropriate network-policy type, reuse it rather than creating another incomplete classifier.

### Credential rule

Even when loopback or private-network access is permitted, credential-bearing HTTP remains rejected. HTTPS is mandatory whenever an API key is configured.

Document any exception only if there is a concrete supported deployment that requires it and an explicit high-risk opt-in is added. The preferred closure is no credential-bearing plaintext exception.

## A.3 Resolve and classify DNS addresses

Before issuing a request to a custom forge base URL:

1. Parse and normalize the URL.
2. Reject embedded username and password fields.
3. Require a host.
4. Classify literal IP hosts directly.
5. Resolve DNS names using a bounded resolution timeout.
6. Classify every resolved address.
7. Reject the endpoint if any resolved address violates policy.
8. Record the normalized origin used for later checks.

The implementation must not accept a DNS name merely because its text is not `localhost`.

### DNS rebinding boundary

A preflight DNS check alone is not a perfect DNS-pinning mechanism. Choose and document one of these closure strategies:

Preferred:

- resolve once;
- validate all addresses;
- construct the request so it connects only to the validated addresses while preserving the original hostname for TLS SNI and Host headers.

Acceptable if reqwest constraints make pinning disproportionate for this release:

- disable redirects;
- validate all preflight addresses;
- revalidate the connected remote address if exposed by the transport;
- document residual DNS rebinding risk explicitly;
- prohibit credentials to custom endpoints unless address pinning is active.

Do not claim DNS-rebinding resistance unless the implementation enforces it.

## A.4 Implement one byte-oriented bounded response reader

Replace the current string-building reader with a byte-oriented implementation.

Suggested contract:

```rust
pub struct BoundedBody {
    pub bytes: Vec<u8>,
    pub observed_bytes: usize,
}

pub async fn read_bounded_body(
    response: reqwest::Response,
    per_response_cap: usize,
    aggregate_counter: &mut usize,
    aggregate_cap: usize,
) -> Result<BoundedBody, ForgeTransportError>;
```

Requirements:

- reject honest `Content-Length` values over the remaining allowed budget before reading;
- stream chunks;
- check prospective length before extending the buffer;
- never allocate more than a small bounded overhead beyond the cap;
- apply both per-response and aggregate-request budgets;
- return typed errors for response cap and aggregate cap;
- preserve raw bytes until decoding or JSON parsing;
- do not decode each network chunk independently.

### UTF-8 behavior

For JSON APIs, decode once after bounded collection with `std::str::from_utf8` or parse JSON directly from bytes.

Add a regression test where one valid multibyte character is split across two transport chunks. The response must parse successfully.

## A.5 Apply bounds to every body path

Audit all forge adapter response handling.

The bounded reader must cover:

- successful tree responses;
- default-branch or repository metadata responses;
- commit/ref resolution responses;
- GitHub Contents fallback responses;
- provider error-body previews;
- rate-limit detection bodies;
- redirect bodies, if read at all.

Remove or forbid direct uses of:

- `response.text().await`;
- `response.bytes().await`;
- `response.json().await`;

inside the forge transport boundary.

### Error-body preview

Provider error messages may include a small body preview, but it must use a separate low cap, for example 8–32 KiB.

Sanitize control characters and credentials before returning the preview.

## A.6 Normalize and encode URL components

Centralize URL assembly with `Url` path-segment and query APIs rather than string interpolation for user-controlled values.

Encode:

- owner and nested namespace components;
- repository name;
- branch, tag, and arbitrary ref values;
- file paths;
- API query parameters.

Add fixtures for:

- branch `feature/foo`;
- tag `release/2026.07`;
- owner namespace `group/subgroup`;
- repository names containing spaces or percent-encoded characters where hosts allow them;
- paths containing `#`, `?`, spaces, Unicode, and percent characters.

## A.7 Transport telemetry

Extend telemetry or structured warnings with bounded, non-secret fields:

- endpoint origin;
- redirect rejected;
- response bytes observed;
- response cap applied;
- DNS policy class;
- aggregate byte cap reached.

Do not expose API keys, authorization headers, or full internal endpoint URLs when diagnostics are configured for untrusted clients.

## A.8 Workstream A tests

Add deterministic tests for:

1. 200 response under cap.
2. Honest `Content-Length` over cap.
3. Chunked response crossing cap.
4. Aggregate page budget crossing cap.
5. Error body crossing the error-preview cap.
6. Valid UTF-8 split across chunks.
7. Invalid UTF-8 rejected deterministically.
8. Same-origin redirect rejected by default.
9. Cross-origin redirect rejected before credential forwarding.
10. Redirect from public to loopback rejected.
11. DNS name resolving to loopback rejected.
12. DNS name resolving to private IPv4 rejected.
13. DNS name resolving to private IPv6 rejected.
14. Mixed public and private DNS answers rejected.
15. Explicit private-network policy allows an HTTPS internal forge.
16. Credential-bearing HTTP rejected under every policy.
17. Embedded credentials rejected.
18. Missing host rejected.
19. Unsupported scheme rejected.
20. Slash-containing refs encoded correctly.

Use a small purpose-built local HTTP server where `httpmock` cannot control chunk boundaries or redirect behavior precisely enough.

## A.9 Workstream A acceptance criteria

- No direct unbounded body helper remains in forge code.
- Forge clients do not follow redirects automatically.
- Tests prove credentials are not sent to redirect targets.
- Tests prove DNS-resolved private addresses are rejected by default.
- Valid split-chunk UTF-8 succeeds.
- Error bodies are bounded.
- Documentation describes the implemented endpoint policy exactly.

---

# Workstream B — Repository Provenance and URL Correctness

## B.1 Introduce an explicit resolved identity type

Replace overloaded `resolved_ref` semantics with a provider-neutral identity structure.

Suggested shape:

```rust
pub struct ResolvedRepositoryIdentity {
    pub requested_ref: Option<String>,
    pub resolved_ref_name: Option<String>,
    pub resolved_commit_sha: Option<String>,
    pub tree_sha: Option<String>,
    pub default_branch: Option<String>,
}
```

`ForgeTreeResponse` should contain this structure or equivalent fields.

`ForgeRawEntry.sha` should be renamed or documented unambiguously as `object_sha`.

Prefer an additive migration where necessary, but eliminate internal ambiguity immediately.

## B.2 Resolve refs before fetching trees

For each provider family, resolve the requested ref to an actual commit when the API supports it.

### GitHub

Resolve using a commit endpoint or equivalent provider response before generating immutable links.

Expected sequence:

1. Determine the requested ref: caller ref or default branch.
2. Resolve it to a commit SHA.
3. Obtain the commit's tree SHA.
4. Fetch the recursive tree by tree SHA or resolved commit where the endpoint contract allows it.
5. Store the commit SHA and tree SHA separately.

Do not treat the Git Trees API response SHA as a commit without confirming the endpoint response semantics for the exact request form.

### GitLab

Use the repository commit/ref endpoint to obtain the commit ID for the requested branch, tag, or SHA.

The response-level `commit_sha` must not be `HEAD` or an unresolved branch string.

### Gitea, Forgejo, and Codeberg

Use the appropriate commit/ref resolution endpoint supported by the shared API family.

If a host version does not provide the required endpoint:

- leave `resolved_commit_sha` absent;
- retain the requested ref separately;
- generate mutable ref URLs rather than claiming immutable provenance;
- emit a structured warning such as `commit_resolution_unavailable`.

Never serialize a ref name into a field called `commit_sha`.

## B.3 Correct entry URL construction

Change URL construction to accept repository identity and entry object identity separately.

Suggested function inputs:

```rust
fn build_entry_urls(
    host: CodeHost,
    owner: &str,
    repo: &str,
    identity: &ResolvedRepositoryIdentity,
    entry: &ForgeRawEntry,
    instance_root: Option<&Url>,
) -> EntryUrls;
```

Rules:

- GitHub immutable browser and raw URLs use `resolved_commit_sha`.
- GitLab immutable URLs use `resolved_commit_sha` when supported by the browser/raw URL scheme.
- Gitea/Forgejo/Codeberg use resolved commit identity when supported; otherwise use the requested ref and mark provenance mutable.
- `entry.object_sha` is retained for diagnostics or future blob fetches, not substituted for commit identity.
- Directory entries do not receive raw-file URLs.
- Submodules must identify the submodule commit separately if surfaced.

## B.4 Extend response schema additively

Review `RepoMapResponse` and `RepoMapEntry`.

Recommended additive fields:

At response level:

- `requested_ref`;
- `resolved_ref_name`;
- `commit_sha`;
- `tree_sha`;
- `provenance_pinned: bool` or an equivalent enum.

At entry level:

- `object_sha`;
- `url`;
- `raw_url`;
- optional `provenance` if needed to distinguish pinned and mutable links.

Keep existing fields where clients may depend on them, but correct their semantics.

Update schemas, examples, and compatibility tests.

## B.5 Correct fallback behavior

GitHub Contents fallback must include the requested or resolved ref in its query.

The fallback must not silently read the default branch when the caller requested another ref.

If the primary tree response is truncated:

- preserve the original identity;
- request fallback content at the same state;
- merge entries deterministically;
- never combine entries from different commits;
- mark the result incomplete when fallback covers only the root.

## B.6 Provenance tests

Add fixtures for all supported host families.

Required cases:

1. Branch ref resolves to commit SHA.
2. Tag ref resolves to commit SHA.
3. Direct commit SHA remains the same.
4. Slash-containing branch resolves correctly.
5. Missing ref produces structured `ref_not_found` behavior.
6. GitHub tree SHA differs from commit SHA and both are retained correctly.
7. Entry blob SHA differs from commit SHA and URL uses commit SHA.
8. Directory tree SHA is not used as a permalink revision.
9. GitLab response never places `HEAD` in `commit_sha`.
10. Gitea/Forgejo unavailable commit-resolution endpoint degrades to mutable provenance without lying.
11. GitHub Contents fallback sends the requested ref.
12. Fallback entries and primary entries share one resolved commit identity.
13. Directory entries omit raw URLs.
14. Unicode and reserved path characters generate valid URLs.
15. Serialization remains additive-compatible with previous clients.

Delete or rewrite tests that currently assert entry object SHAs in commit-pinned URLs.

## B.7 Workstream B acceptance criteria

- No field named `commit_sha` contains a branch name, `HEAD`, tree SHA, or blob SHA.
- GitHub entry URLs use resolved commit SHA.
- Fallback paths preserve the requested repository state.
- Tests explicitly use different commit, tree, and blob SHAs.
- Mutable provenance is represented honestly when commit resolution is unavailable.
- Documentation defines every SHA field.

---

# Workstream C — Truly Bounded Git Process Execution

## C.1 Replace the current bounded runner

The current implementation reads pipes to completion and truncates afterward. Replace it with a runner that enforces caps during reads.

Suggested result type:

```rust
pub struct BoundedCommandOutput {
    pub status: Option<ExitStatus>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub killed: bool,
}
```

The runner must distinguish:

- spawn failure;
- timeout;
- stdout cap;
- stderr cap;
- non-zero exit;
- invalid UTF-8 where string parsing is required.

## C.2 Drain stdout and stderr concurrently

Use one of these patterns:

- `tokio::process::Command` with asynchronous capped readers inside the blocking search orchestration boundary; or
- independent reader threads for stdout and stderr with shared cancellation.

Do not read stdout fully before beginning stderr.

Each reader must:

- read fixed-size chunks;
- check the remaining cap before append;
- signal cancellation immediately on cap breach;
- return the bytes retained and truncation status.

## C.3 Terminate process groups safely

On Unix:

- start Git in a new process group;
- terminate the group on timeout or output-cap breach;
- wait for and reap the direct child.

On Windows, use the strongest available supported mechanism, such as a Job Object or explicit child termination, and document any limitation.

Avoid a detached watchdog thread that can outlive the command or race against process-ID reuse.

The timeout mechanism must not send a signal to a reused PID after the child has exited.

## C.4 Use the bounded runner for every Git operation

Audit local inventory and repository identity code.

All of these must be bounded:

- `git ls-files --cached`;
- `git ls-files --others --exclude-standard`;
- `git rev-parse --git-dir`;
- `git rev-parse --show-toplevel`;
- `git rev-parse HEAD` or equivalent;
- dirty/change-token commands;
- any future Git metadata command in the inventory path.

No `.output()` or unbounded `read_to_end()` should remain in the local Git boundary.

## C.5 Enumerate tracked and untracked files cleanly

Prefer two bounded NUL-delimited commands:

1. tracked files: `git ls-files -z --cached`;
2. untracked files: `git ls-files -z --others --exclude-standard`.

Then:

- parse both bounded outputs;
- count untracked files from the second output;
- merge and deduplicate deterministically;
- apply centralized path eligibility to both sets;
- preserve truncation and timeout diagnostics separately.

This avoids a second unbounded command solely for telemetry.

If one command fails:

- do not poison the cache with a partial inventory presented as complete;
- either use the safe partial inventory with explicit truncation metadata or fall back to the bounded native walker according to a documented rule.

## C.6 Resolve ordinary repositories and linked worktrees

Use bounded Git commands to resolve:

- worktree root;
- actual Git directory;
- common Git directory where needed;
- index path;
- HEAD identity.

Do not assume `.git` is a directory.

Support:

- ordinary `.git/` directories;
- linked worktrees where `.git` is a file;
- bare repositories only if local workspace search intentionally supports them; otherwise reject them explicitly.

## C.7 Command-runner tests

Add deterministic helper binaries or shell fixtures that simulate:

1. small stdout and stderr.
2. stdout over cap.
3. stderr over cap.
4. both pipes producing enough data to deadlock a sequential reader.
5. process sleeping past timeout.
6. child spawning a descendant that keeps a pipe open.
7. non-zero exit with bounded diagnostic output.
8. invalid UTF-8 paths.
9. exit immediately before timeout.
10. repeated timeout runs without PID-race failures.
11. tracked and untracked command merge.
12. one command failing without cache poisoning.
13. linked-worktree Git-directory resolution.

The tests must complete under a strict wall-clock limit.

## C.8 Workstream C acceptance criteria

- No local Git operation uses unbounded `.output()`.
- Pipe readers are concurrent and cap before append.
- A pipe-filling fixture cannot deadlock the test suite.
- Timeout and output-cap breach terminate and reap the process.
- Untracked count is derived from bounded output.
- Linked worktrees resolve the correct index and HEAD paths.

---

# Workstream D — Inventory Freshness and Race-Resistant Local Reads

## D.1 Introduce an explicit workspace change token

The current TTL, HEAD, and index-mtime checks do not detect newly created untracked files promptly.

Add a bounded change token representing the state relevant to inventory membership.

A practical Git-backed token may include:

- resolved Git directory;
- HEAD commit;
- index metadata or checksum;
- bounded hash of `git status --porcelain=v2 -z --untracked-files=normal` or equivalent;
- relevant exclude and ignore metadata where not represented by status output.

The command used to produce the token must use the Workstream C bounded runner.

The token must change when:

- a tracked file is staged or removed;
- an untracked non-ignored file is created or deleted;
- a file changes from ignored to eligible due to ignore-rule edits;
- the worktree switches branches;
- the linked-worktree index changes.

## D.2 Balance freshness and latency

Checking a full dirty token before every search may be expensive in very large repositories.

Implement a bounded policy:

- use a short freshness probe interval, separate from full inventory TTL;
- run the change-token probe after the short interval;
- hard-limit probe time and output;
- rebuild when the token changes;
- if the probe times out, mark freshness indeterminate and choose a safe documented behavior.

Recommended behavior on indeterminate freshness:

- do not claim `High` confidence;
- either rebuild through the bounded inventory path or serve cached results with a structured warning and `Low` confidence;
- for explicit file/path queries, perform a bounded direct eligibility check so newly created named files can still be found.

Set defaults based on measured cost rather than the existing five-minute TTL alone.

## D.3 Store freshness inputs in inventory metadata

Extend `RootInventory` with fields such as:

- resolved worktree root;
- resolved Git directory;
- index path;
- change token;
- change-token observation time;
- freshness confidence;
- last probe outcome;
- partial-build state.

Do not expose sensitive absolute paths in untrusted MCP responses unless existing local telemetry already permits them.

## D.4 Make cache publication atomic

Build new inventories outside the cache lock.

Publish only after:

- all roots have a well-defined completion state;
- partial or truncated state is recorded;
- the configuration fingerprint and change tokens are attached;
- sorting and deduplication are complete.

Concurrent cold searches should result in:

- one shared build; or
- bounded duplicate work with a deterministic winner.

A failed build must not replace a valid prior inventory with an empty or malformed cache entry.

## D.5 Implement capability-style local file opening

Replace validate-then-reopen path reads for inventory candidates and direct workspace fetches.

Preferred Unix implementation:

1. Open the workspace root directory handle.
2. Walk each relative path component using `openat`-style calls.
3. Reject `..`, absolute prefixes, and empty components before system calls.
4. Open intermediate directories with no-follow semantics.
5. Open the final file with no-follow semantics.
6. Inspect the opened handle with `fstat`.
7. Require a regular file.
8. Enforce the current size cap from the opened handle.
9. Read at most the configured byte cap from that handle.

Use `rustix`, `cap-std`, or carefully reviewed platform APIs rather than path revalidation followed by `std::fs::read`.

The implementation must work on macOS and Linux, the primary development and deployment targets.

For unsupported platforms:

- provide a conservative implementation;
- document residual limitations;
- do not silently weaken policy while reporting race-safe behavior.

## D.6 Reuse safe-open in all local content paths

Use the same helper for:

- inventory search content scoring;
- snippet extraction;
- symbol extraction;
- direct local `repo_fetch`;
- local `repo_map` important-file probes;
- any dependency-file reading initiated through user-supplied paths, where the same workspace policy applies.

Avoid reading the same file once through a safe helper and again through an unsafe path helper.

Return a bounded content object containing:

- bytes or text;
- observed size;
- truncation status;
- stable metadata from the opened handle;
- failure reason.

## D.7 Strengthen eligibility consistency

The native walker, Git path enumeration, direct fetch, and safe-open helper must use one component policy.

Consolidate:

- hidden component handling;
- `SKIP_DIRS` handling;
- binary extension policy;
- symlink policy;
- file size policy;
- root containment;
- ignored-file policy where applicable.

Add a table-driven parity test that sends the same path corpus through every eligibility surface and asserts compatible decisions.

## D.8 Freshness and safe-open tests

Required tests:

1. Cold search builds and publishes inventory.
2. Second unchanged search reuses inventory.
3. New untracked file becomes visible after the bounded freshness probe, without five-minute delay.
4. Deleted untracked file disappears.
5. Staged file changes invalidate inventory.
6. `.gitignore` change affects membership.
7. Branch switch invalidates inventory.
8. Linked-worktree index change invalidates inventory.
9. Change-token timeout lowers freshness confidence and emits a warning.
10. Failed rebuild preserves prior valid inventory.
11. Concurrent cold searches do not publish partial state.
12. Final component symlink substitution is rejected.
13. Intermediate directory symlink substitution is rejected.
14. File growth after inventory creation remains byte-capped.
15. File deletion between candidate selection and open is handled as a soft stale-entry failure.
16. File replaced by directory is rejected.
17. Hard link behavior is documented and tested according to policy.
18. Native, Git, direct-fetch, and safe-open eligibility decisions remain consistent.

For race tests, use synchronization barriers to force substitution between candidate selection and open. Avoid probabilistic sleep-only tests.

## D.9 Workstream D acceptance criteria

- A newly created untracked source file is discoverable promptly.
- Linked worktrees use the real Git directory and index.
- Cache publication cannot replace a valid inventory with a failed partial build.
- Inventory and direct-fetch reads use a race-resistant handle.
- No local content path reopens a previously validated pathname unsafely.
- Freshness confidence reflects probe outcomes rather than age alone.

---

# Workstream E — Complete Workflow-Aware Evidence Integration

## E.1 Materialize evidence roles on returned cards

Postprocessing currently infers roles for summaries without necessarily writing them onto cards.

Add a deterministic mutating pass before final grouping or serialization:

```rust
pub fn materialize_evidence_roles(cards: &mut [SourceCard]);
```

Rules:

- preserve explicit provider-derived roles;
- infer only when absent;
- ensure the inferred role is stable under result ordering;
- recompute quality metadata only if evidence role participates in quality scoring;
- apply to `web_search`, `repo_search`, `security_search`, and `research_search` result cards.

Tests must assert the serialized `SourceCard.metadata.evidence_role`, not merely aggregate counts.

## E.2 Select workflow models from requests

Create one function per tool or one shared resolver mapping request context to a coverage model.

Examples:

### `repo_search`

- coding profile → implementation, interface/API definition, tests/examples, and documentation expectations;
- exact-error mode → implementation or issue evidence plus version/release context;
- security profile → authoritative advisory, implementation, patch/release, and defensive guidance expectations;
- research profile → primary, official, counterpoint, and design/academic expectations.

### `research_search`

Map every `ResearchWorkflow` variant to the existing `WorkflowCoverageModel` for that workflow.

### `security_search`

Use a security-review model that distinguishes authoritative advisory evidence from generic discussion and requires version/applicability evidence when the request includes package and version context.

### `web_search`

Coverage may remain absent when no workflow is implied. Do not force a coding-oriented generic model onto ordinary web queries.

Pass the selected model into postprocessing rather than `None`.

## E.3 Represent actual retrieval outcomes

Introduce or reuse a provider/subquery outcome model that distinguishes:

- queried and returned results;
- queried successfully with zero results;
- failed;
- timed out;
- rate limited;
- skipped by operator policy;
- skipped because capability was unavailable;
- not applicable;
- interrupted by global deadline;
- truncated after partial success.

Do not derive all outcomes solely from final cards and `providers_failed`.

Capture outcomes at dispatch time while the information is available.

Suggested dimensions:

```rust
pub struct RetrievalAttempt {
    pub provider_id: String,
    pub subquery_id: Option<String>,
    pub intended_roles: Vec<EvidenceRole>,
    pub outcome: RetrievalAttemptOutcome,
    pub result_count: usize,
}
```

## E.4 Map providers and subqueries to evidence roles correctly

A successful generic provider is not automatically primary implementation evidence.

Use:

- native provider capability;
- result `SourceKind` and metadata;
- subquery purpose;
- research source type;
- security tier;
- repository result role.

A retrieval dimension may target multiple possible evidence roles. Keep the mapping deterministic and documented.

Examples:

- `github_code` source subquery → primary implementation or interface definition;
- `github_issues` issue subquery → issue/incident discussion;
- release provider → release note/changelog;
- OSV/NVD/RustSec/CISA → authoritative security advisory or exploitation-status evidence;
- generic web provider for a benchmark subquery → benchmark evidence only if returned cards classify as benchmark evidence.

## E.5 Compute coverage from the active model and outcomes

Coverage must distinguish:

- missing after successful retrieval;
- indeterminate because relevant providers failed or timed out;
- unsupported because no selected provider could retrieve that role;
- present but weak;
- present with sufficient evidence.

Populate `workflow_coverage` on applicable responses.

Ensure the status vocabulary used by code and documentation is identical. Remove stale names such as `covered` versus `sufficient` if both remain from earlier iterations.

## E.6 Refine retrieval summaries

Correct known semantic issues:

- rate limiting is a provider failure or throttling outcome, not a policy skip;
- successful zero-result retrieval is evidence absence, not a provider skip;
- a provider not selected is different from a selected provider that failed;
- partial provider success should retain both success and failure information;
- deadline interruption should be explicit;
- truncation should be explicit.

Add summary counts and flags only when they can be computed from actual attempts.

## E.7 Scope conflict detection to canonical entities

Refactor conflict detection around a comparison key.

Suggested key:

```rust
pub struct ConflictEntityKey {
    pub entity_type: ConflictEntityType,
    pub canonical_id: String,
    pub field: String,
}
```

Examples:

- vulnerability + CVE/GHSA/OSV identity + affected range;
- package + normalized ecosystem/name/version + release date;
- benchmark + normalized model/task/hardware context + score;
- repository file + canonical repo/commit/path + mutable-versus-pinned status.

Requirements:

- compare values from at least two distinct source records;
- do not compare two list elements within the same record as a source conflict;
- normalize values before comparison;
- preserve source IDs and URLs used in the comparison;
- include a reason explaining comparability;
- classify confidence and severity conservatively;
- cap output deterministically.

### Version ranges

Use ecosystem-aware normalization where available. Different but equivalent range syntax should not produce a conflict.

### Dates

Normalize timestamps and date-only values. Distinguish publication date, modification date, disclosure date, and release date rather than comparing unrelated fields.

### Mutable versus pinned

Only report mutable-versus-pinned tension when records refer to the same canonical repository entity or claim. Do not compare every mutable card against every pinned card globally.

## E.8 Generate gap-driven next actions

Replace generic source-ID-only actions where workflow evidence is available.

For each missing or indeterminate required/recommended role:

- select the most productive MCP tool;
- populate a valid input template;
- attach the evidence role or evidence gap;
- attach a concise rationale;
- include relevant source IDs or repository hints;
- avoid repeating a tool call already known to have failed unless the action changes provider, scope, or query;
- cap and order actions deterministically.

Examples:

- missing official documentation → targeted `web_search` or `repo_search` docs intent;
- missing release notes → `repo_search` with releases enabled;
- missing implementation → native repository search or `repo_map` followed by `repo_fetch`;
- missing authoritative advisory → `security_search` with native advisory providers;
- unresolved conflict → fetch the highest-authority conflicting sources;
- indeterminate due to timeout → retry with a narrower provider/subquery budget rather than asserting absence.

Validate action templates against actual MCP argument schemas in tests.

## E.9 Response schema completion

Review all response types:

- `WebSearchResponse`;
- `RepoSearchResponse`;
- `ResearchSearchResponse`;
- `SecuritySearchResponse`.

Where applicable, expose additively:

- materialized per-card evidence role;
- workflow coverage;
- retrieval summary;
- conflict metadata;
- evidence-role summary;
- gap-driven next actions.

Security responses should not omit workflow coverage and conflict metadata merely because earlier integration added only retrieval and role summaries.

Do not emit empty placeholder fields if `skip_serializing_if` can preserve compact output.

## E.10 Evidence integration tests

Required deterministic tests:

1. Returned cards serialize an inferred evidence role.
2. Explicit provider role is preserved.
3. Role assignment is deterministic under randomized input order.
4. Research workflow selects the correct model.
5. Repo coding profile selects the correct model.
6. Exact-error mode selects the correct model.
7. Security package/version request selects applicability-aware requirements.
8. Successful zero results produce absence, not failure.
9. Timeout produces indeterminate coverage.
10. Rate limit is not a policy skip.
11. Unselected provider is not reported as failed.
12. Partial provider success preserves partial outcome.
13. Required role missing after successful retrieval produces insufficient coverage.
14. Required role blocked by failure produces indeterminate coverage.
15. Two equivalent version ranges do not conflict.
16. Two values from one advisory do not produce cross-source conflict.
17. Two distinct advisories with incompatible normalized ranges do conflict.
18. Unrelated mutable and pinned sources do not conflict.
19. Same canonical repository entity with mutable and pinned evidence can produce scoped metadata.
20. Gap-driven next action identifies the missing role.
21. Next-action template keys match the target tool schema.
22. Failed retrieval is not immediately recommended unchanged.
23. Output ordering and caps remain deterministic.
24. Empty ordinary web search does not receive an irrelevant coding coverage model.

Add end-to-end MCP serialization fixtures for codegg consumption, not only unit tests of helper functions.

## E.11 Workstream E acceptance criteria

- Applicable responses contain non-empty workflow coverage using the requested workflow.
- Cards contain evidence roles directly.
- Retrieval summary distinguishes success-zero, failure, timeout, rate limit, policy skip, deadline, and truncation.
- Conflict tests use at least two distinct source records.
- Next actions are driven by gaps and validate against MCP schemas.
- Security, repository, and research outputs all expose the intended additive metadata.

---

# Workstream F — Verification, Documentation, and Release Evidence

## F.1 Correct documentation before expanding it

Audit these files and any related skills:

- `AGENTS.md`;
- `.opencode/skills/eggsearch-architecture/SKILL.md`;
- `.opencode/skills/eggsearch-dev/SKILL.md`;
- `.opencode/skills/eggsearch-mcp/SKILL.md`;
- `.opencode/skills/eggsearch-release/SKILL.md`;
- `docs/architecture/core.md`;
- `docs/architecture/meta.md`;
- `docs/architecture/hardening.md`;
- `docs/architecture/overview.md`;
- `docs/config.md`;
- `docs/safety.md`;
- `docs/agent-workflows.md`;
- `docs/test-inventory.md`;
- `docs/release.md`.

Remove or correct claims that are stronger than the implementation, especially:

- DNS-resolved address validation;
- redirect rejection;
- universally bounded forge responses;
- commit SHA semantics;
- bounded Git output;
- immediate untracked freshness;
- safe local file opening;
- workflow coverage population.

After implementation, restore precise claims with tests named nearby.

## F.2 Add architecture decision notes

Document the final decisions for:

- custom forge endpoint policy;
- redirect handling;
- DNS pinning or residual rebinding risk;
- repository identity field semantics;
- subprocess execution model;
- workspace change-token strategy;
- race-resistant local open strategy;
- evidence workflow selection and conflict scoping.

These may be incorporated into existing architecture docs rather than separate ADR files, but the rationale must be retained.

## F.3 Deterministic verification matrix

The implementation commit is not complete until all of these pass on the final head:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo test --features mock
cargo test --features pdf
cargo test --test forge_adapter
cargo test --test recipes_next_actions
cargo test --test corpus_runner
```

Also run any Makefile release and hardening targets defined by the repository:

```bash
make hardening
make release-check
```

If target names differ, update this plan's implementation note and use the repository's canonical commands.

## F.4 Add static guard tests

Add source-contract tests that fail if forbidden patterns reappear in bounded boundaries.

Examples:

- direct `.text().await`, `.bytes().await`, or `.json().await` in forge transport files;
- unbounded `.output()` in local Git inventory files;
- path-based `std::fs::read` in race-resistant local retrieval paths;
- `entry.object_sha` passed as commit revision to URL builders;
- postprocessing invoked with `workflow_model = None` for requests that specify a workflow.

Use these tests narrowly to enforce architectural contracts without banning legitimate use elsewhere.

## F.5 Property and fuzz coverage

Extend existing hardening targets.

### Forge response fuzzing

Fuzz the byte-oriented bounded reader with:

- arbitrary chunk boundaries;
- valid and invalid UTF-8;
- JSON prefixes and truncation;
- cap values around boundary conditions;
- aggregate-budget transitions.

### URL and identity properties

Property-test:

- encoded refs never alter path structure;
- commit, tree, and object identifiers are never substituted across fields;
- URL generation is deterministic;
- parsed generated URLs retain owner/repo/ref/path semantics.

### Local command parsing

Fuzz or property-test NUL-delimited Git output parsing under caps.

### Conflict detection

Property-test order independence and the invariant that one source alone cannot produce a cross-source conflict.

## F.6 Live-smoke matrix

Run live smoke tests outside deterministic CI against public repositories.

Required cases:

### GitHub

- default branch;
- non-default slash-containing branch;
- nested repository tree;
- repository with a tree response large enough to exercise truncation behavior.

### GitLab

- nested namespace;
- non-default branch;
- nested entries;
- rate-limit or unauthenticated failure behavior where reproducible.

### Codeberg

- public repository map;
- nested entries;
- resolved provenance behavior supported by the API.

### Gitea/Forgejo

- public test instance or controlled integration instance;
- custom base URL policy;
- credentials only over HTTPS;
- redirect rejection.

Capture for every smoke:

- command or test invocation;
- timestamp;
- target repository and ref;
- observed commit SHA;
- entry URL validation sample;
- response byte telemetry;
- warnings;
- pass/fail result.

Do not store credentials in artifacts.

## F.7 Local workspace integration matrix

Test on both macOS and Linux:

- ordinary repository;
- linked worktree;
- dirty tracked file;
- newly created untracked file;
- ignored file;
- large file over cap;
- symlink final component;
- symlink intermediate component;
- repository with high stdout volume from Git enumeration;
- command timeout fixture;
- concurrent cold searches.

Record cold and warm latency distributions.

## F.8 Performance gates

Correctness is primary, but closure must avoid major regression.

Define and capture baselines for representative repositories:

- small: fewer than 1,000 files;
- medium: 10,000–50,000 files;
- large: near configured `max_indexed_files`.

Measure:

- cold inventory build time;
- warm search latency;
- freshness-probe latency;
- memory peak during Git enumeration;
- memory peak during maximum allowed forge response;
- safe-open overhead per candidate;
- evidence postprocessing time for maximum result count.

Suggested release thresholds:

- no unbounded memory growth under cap tests;
- warm local search p95 remains suitable for interactive code-agent use;
- freshness probe remains bounded by its configured deadline;
- evidence postprocessing remains negligible relative to network retrieval;
- no more than a documented, justified regression from the current baseline.

Do not optimize away safety checks to meet an arbitrary threshold. Adjust architecture or defaults with evidence.

## F.9 CI and release evidence

Ensure the final head has visible CI checks for:

- formatting;
- clippy;
- default tests;
- all-features tests;
- no-default-features tests;
- mock and PDF feature tests;
- documentation contract tests;
- schema/corpus tests;
- fuzz smoke where supported.

If GitHub status checks are not attached to direct pushes, run the same commands in a PR or documented release workflow so the final commit has auditable evidence.

Store a release verification note containing:

- final commit SHA;
- Rust toolchain version;
- operating systems tested;
- exact commands;
- test counts;
- live-smoke results;
- benchmark summary;
- known residual limitations.

## F.10 Workstream F acceptance criteria

- Documentation matches code behavior.
- Static guards prevent reintroduction of the key boundary violations.
- Full deterministic test matrix passes on the final head.
- Fuzz smoke passes for all affected targets.
- Live-smoke covers all supported forge families.
- macOS and Linux local-workspace tests pass.
- Performance and memory evidence are recorded.
- The final commit has auditable CI or equivalent release-check evidence.

---

## 6. Expected implementation surface

The exact file decomposition may change, but the following areas are expected to be touched.

### Remote forge retrieval

- `src/meta/forge_adapter.rs`
- optional new `src/meta/forge_transport.rs`
- `src/core/repo_map.rs`
- `src/core/repo_fetch.rs`
- `src/core/code_metadata.rs`
- configuration types and loaders
- `tests/forge_adapter.rs`
- additional transport fixture server tests
- forge fuzz target(s)

### Local workspace

- `src/meta/local_inventory_cache.rs`
- `src/meta/local_backend.rs`
- `src/meta/local_inventory.rs`
- `src/core/local.rs`
- local `repo_fetch` and `repo_map` paths
- optional new `src/meta/bounded_command.rs`
- optional new `src/meta/safe_local_file.rs`
- local inventory and filesystem property tests

### Evidence integration

- `src/core/evidence_postprocess.rs`
- `src/core/evidence_role.rs`
- `src/core/workflow_coverage.rs`
- `src/core/retrieval_status.rs`
- `src/core/conflict.rs`
- `src/core/workflow.rs`
- `src/core/repo_search.rs`
- `src/core/research.rs`
- `src/core/security.rs`
- `src/meta/adapter.rs`
- `src/meta/security_search.rs`
- `src/meta/research_evidence_analysis.rs`
- MCP tool orchestration
- `tests/recipes_next_actions.rs`
- end-to-end response fixtures

### Documentation and release

- architecture, safety, configuration, workflow, test inventory, and release documents
- `.opencode/skills` files
- `AGENTS.md`
- CI workflow files if coverage gaps exist

---

## 7. Recommended commit decomposition

Keep commits narrow enough to review and bisect.

### Commit 1 — forge transport safety

- redirect policy;
- endpoint policy;
- DNS classification or pinning;
- universal bounded byte reader;
- error-body bounds;
- UTF-8 chunk-boundary tests.

### Commit 2 — repository identity and URLs

- resolved identity type;
- host-specific commit resolution;
- correct URL generation;
- ref encoding;
- fallback ref propagation;
- provenance fixtures.

### Commit 3 — bounded Git execution

- concurrent capped pipe readers;
- process-group termination;
- bounded tracked/untracked commands;
- linked-worktree Git-directory resolution;
- command-runner tests.

### Commit 4 — inventory freshness and safe open

- change token;
- prompt untracked invalidation;
- atomic cache publication;
- race-resistant local file handle;
- parity and race tests.

### Commit 5 — workflow-aware evidence completion

- per-card role materialization;
- workflow model resolution;
- attempt/outcome semantics;
- coverage population;
- scoped conflicts;
- gap-driven next actions;
- MCP fixtures.

### Commit 6 — release verification and documentation

- corrected docs and skills;
- source-contract guards;
- fuzz/property additions;
- test inventory update;
- release verification record.

Do not use the documentation commit to hide implementation changes.

---

## 8. Cross-workstream test scenarios

The following scenarios exercise multiple closure contracts and must be represented in integration tests.

### Scenario 1 — non-default GitHub branch

Given a GitHub repository and branch `feature/foo`:

- the ref is URL-encoded;
- the branch resolves to commit `C`;
- the tree resolves to tree `T`;
- file entry has blob `B`;
- response `commit_sha == C`;
- response `tree_sha == T`;
- entry `object_sha == B`;
- browser and raw URLs use `C`;
- fallback requests use the same branch or `C`;
- all response bodies are bounded.

### Scenario 2 — custom internal Forgejo

Given an internal Forgejo HTTPS endpoint:

- default policy rejects its private resolved address;
- explicit private-network policy permits it;
- plaintext HTTP with credentials remains rejected;
- redirects are rejected;
- credentials are never sent cross-origin;
- response and error bodies are bounded.

### Scenario 3 — new codegg-created file

Given an already warm local inventory:

- codegg creates a new untracked, non-ignored source file;
- the next eligible freshness probe detects the change;
- inventory rebuild includes the file;
- a query for its symbol finds it;
- telemetry does not claim stale `High` confidence before detection;
- no five-minute wait is required.

### Scenario 4 — local symlink race

Given a valid candidate path:

- another thread replaces an intermediate directory or final file with a symlink after candidate selection;
- safe-open rejects the substitution;
- no bytes outside the configured root are read;
- response contains a bounded stale/race warning rather than a panic.

### Scenario 5 — research workflow with provider timeout

Given an architecture-decision research workflow:

- primary and official sources are returned;
- counterpoint retrieval times out;
- cards contain evidence roles;
- coverage marks counterpoint evidence indeterminate rather than absent;
- retrieval summary records timeout;
- next action proposes a narrower or alternate counterpoint retrieval;
- no unrelated mutable-versus-pinned conflict is emitted.

### Scenario 6 — security range disagreement

Given two distinct authoritative advisories for the same vulnerability:

- equivalent ranges normalize without conflict;
- incompatible ranges produce one scoped conflict;
- source IDs and normalized values are included;
- multiple patched versions inside one advisory do not create a false conflict;
- remediation actions remain based on authoritative/applicability evidence.

---

## 9. Definition of done

This closure pass is complete only when every item below is true.

### Forge safety

- [ ] Redirect following is disabled or every redirect hop is revalidated.
- [ ] Credentials cannot cross origins.
- [ ] DNS-resolved addresses are classified under explicit policy.
- [ ] Credential-bearing HTTP is rejected without exception.
- [ ] Successful, metadata, fallback, and error bodies are hard-bounded.
- [ ] Split-chunk valid UTF-8 succeeds.
- [ ] No forbidden unbounded body helper remains in forge code.

### Provenance

- [ ] Requested ref, resolved ref name, commit SHA, tree SHA, and object SHA have distinct semantics.
- [ ] Every `commit_sha` is an actual commit or absent.
- [ ] Immutable entry URLs use commit SHA.
- [ ] Slash-containing refs are encoded.
- [ ] Fallback requests preserve repository state.
- [ ] Tests use intentionally different commit/tree/blob IDs.

### Git execution

- [ ] Stdout and stderr are drained concurrently.
- [ ] Output is capped during read.
- [ ] Timeouts and cap breaches terminate and reap the process.
- [ ] No unbounded Git `.output()` remains.
- [ ] Tracked and untracked outputs are both bounded.
- [ ] Linked worktrees resolve correctly.

### Local freshness and file safety

- [ ] New untracked files invalidate or refresh inventory promptly.
- [ ] Index, HEAD, ignore, and linked-worktree changes are detected.
- [ ] Failed rebuilds do not poison valid cache state.
- [ ] Local reads use race-resistant file handles.
- [ ] Intermediate and final symlink races are tested.
- [ ] Freshness confidence reflects actual probe state.

### Evidence workflows

- [ ] Returned cards contain evidence roles.
- [ ] Applicable requests select an actual workflow model.
- [ ] Coverage is populated for repo, research, and security workflows.
- [ ] Retrieval outcomes distinguish zero results, failure, timeout, rate limit, skip, deadline, and truncation.
- [ ] Conflicts are scoped to canonical entities and distinct sources.
- [ ] Gap-driven next actions include valid templates and rationale.
- [ ] End-to-end MCP fixtures cover codegg consumption.

### Release evidence

- [ ] Formatting and clippy pass.
- [ ] All feature test matrices pass.
- [ ] Hardening and schema/corpus tests pass.
- [ ] Affected fuzz targets pass smoke campaigns.
- [ ] Live-smoke covers GitHub, GitLab, Codeberg, and Gitea/Forgejo.
- [ ] macOS and Linux local-workspace matrices pass.
- [ ] Performance and memory evidence are recorded.
- [ ] Documentation matches implementation.
- [ ] Final commit has auditable CI or equivalent release-check output.

No item may be closed solely because a commit message says it was implemented.

---

## 10. Release decision

After implementation, perform one final review against this plan.

The appropriate outcomes are:

### Release candidate

Use this classification only when all safety and correctness gates pass, deterministic CI is green, live-smoke evidence is captured, and no known issue can cause credential disclosure, unbounded memory/process behavior, provenance misrepresentation, workspace escape, or materially misleading evidence semantics.

### Late beta

Use this classification if core behavior works but one or more non-exploitable correctness or observability gaps remain. Document each gap and its user impact.

### Not releasable

Use this classification if any of the following remains:

- credential forwarding to an unvalidated endpoint;
- unbounded response or subprocess output;
- commit/blob/tree identity confusion;
- local workspace escape or symlink race;
- cache behavior that routinely hides newly created agent files;
- workflow metadata that asserts absence when retrieval failed;
- unscoped false conflict generation.

The expected result of this pass is a credible release candidate without another broad architecture cycle.
