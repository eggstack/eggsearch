# M057 — M056 Audit Follow-Up Correctives

Status: closed
Date: 2026-11-27
Subsystem: agentic retrieval integrity
Milestone class: invariant (corrective to M056)
Baseline: commit `99b2532` (M056 implementation and closure commit)
Corrects: M056, `plans/implementation/agentic-retrieval-integrity/056-live-surface-diagnostic-honesty-correctives.md`
Closure: `plans/closure/agentic-retrieval-integrity/056-status.md`
Audit: post-closure read-only audit of M056 at `99b2532` (code, tests, guards, and live-probe retention).

Decision authority: M056 WP4 (reserved-versus-private hostname distinction) and its closure record; `plans/003-planning-process.md` §7 (corrective passes are new plans). Approved scope decisions for this plan: null is absent; fragment-only documentation guard; trailing-dot normalization inside WP1; one shared hostname classifier.

## Objective

Close the five defects that the post-closure audit of M056 confirmed. M056 correct
diagnostics were applied at one seam each, but two of those seams are shared, and the
audit found that the browser interception path never received the reserved-hostname
classification, that trailing-dot fully-qualified names bypass every hostname predicate,
that the redirect wrapper reintroduces the misclassification M056 removed, that a
present-but-null cutoff still rejects, and that two in-repository documentation anchors
do not resolve. Keep M056 closed; this plan is a separate corrective milestone and
authorizes no code changes by itself.

## Evidence and corrective rationale

| Finding | Current evidence | Why M056 verification missed it |
|---|---|---|
| F1 (P2) Browser interception does not apply the reserved-hostname classifier before DNS. `is_private_host` covers `localhost`, `.localhost`, loopback literals, the metadata endpoints, and `is_private_hostname`, but never consults `is_reserved_hostname`, so `*.invalid` and `*.test` reach resolution in the browser path. Fetch validation rejects those names pre-DNS. | `src/fetch/browser/intercept.rs:94-110` with the classifier call at `:104`; reserved suffixes in `src/fetch/limits.rs:464-465`; `tests/proxy_route_feasibility.rs:17,668` shows `.invalid` resolving through a pinned resolver, so the browser guarantee depends on DNS rather than on the name. | M056 WP4 tested the fetch validator and a live `web_fetch` probe; no test drives a reserved name through browser interception, and the interception suite covers only `.internal`/`.private`/`.local` (`intercept.rs:185-193`). |
| F2 (P2) A present-but-null `cutoff` is rejected. The near-miss scan treats any occurrence of `cutoff`, `knowledge_cutoff`, or `cutoff_date` as a mistake, so `{"cutoff": null}` — the ordinary encoding of an unset optional field — fails before the override path is reached. | `src/mcp/tools/knowledge_brief.rs:80-83` (scan) and `:210-216` (rejection naming `cutoff_override`). | M056 WP1 added the scan to catch non-null strings; it did not consider JSON null, which serializers emit for unset optional members. |
| F3 (P3) Trailing-dot fully-qualified names bypass every hostname predicate. The URL parser preserves a terminal dot, `host_lower == "localhost"` fails for `localhost.`, and suffix predicates fail for `foo.invalid.`, `foo.test.`, and `service.internal.`. Both validators and the browser path use the preserved form. | `src/fetch/limits.rs:274-275` and `:355-356` (lowercase only, with the `localhost` equality immediately after and no normalization); suffix predicates at `:464-482`; `intercept.rs:94-110`. Audit probe: `http://foo.invalid./` yields host `foo.invalid.` and `http://localhost./` yields `localhost.`; `127.0.0.1.` is normalized by the parser, so only hostname forms are affected. | Pre-existing, not introduced by M056. Neither M056 nor the earlier SSRF suites carried a trailing-dot case, and `tests/property_fetch_limits.rs:12` documents the suffix list without a canonical-form requirement. |
| F4 (P3) The redirect wrapper re-adds the prefix M056 removed. A blocked redirect target is mapped to `RedirectTargetBlocked(format!("private network: {reason}"))`, so a reserved name reports `private network: reserved hostname can never resolve: …`. | `src/fetch/client.rs:2287-2289`; the existing redirect tests assert the error kind, not the message, so nothing pins the prefix. | M056 WP4 changed the reason text at the validator; the redirect mapping lives in the transport layer and was outside its work package. |
| F5 (P3) Two in-repository documentation links do not resolve. `architecture/engines.md:234` targets `#keyless-tier-degradation`; the heading is `### Keyless-tier degradation runbook` (`docs/provider-setup.md:24`), so the slug is `keyless-tier-degradation-runbook`. `docs/config.md:245` targets `tool-matrix.md#compatibility-aliases`; the heading is `## Legacy call compatibility and web field aliases` (`docs/tool-matrix.md:42`). | Both anchors re-read at the baseline. `make docs-check` is rustdoc-only (`cargo doc -D warnings`) and no test inspects Markdown fragments. | M056 WP5 added the runbook and linked it from `architecture/engines.md`; a new heading slug is not exercised by any gate, so the link was never resolved. The `docs/config.md` anchor predates M056. |
| F6 (P4) The test inventory is stale. It records 6669 `--all-features` tests and 30 ignored; the audit measured 6692 and 30 at `99b2532`. | `docs/test-inventory.md:9-10`; measured with `cargo test --all-features -- --list`. | Drift predates M056 and accumulates between closures; no gate derives the table from the build. |

The M056 closure record states that the reserved/private distinction is message-only
under the existing `PrivateNetworkBlocked` kind. That decision stands; this plan fixes
the seams that decision did not reach.

## Contract and invariants

- Keep the nine advertised tools, twelve callable names, aliases, accepted argument
  sets, error codes, providers, default sets, retrieval behavior, source IDs, and
  trust framing unchanged.
- Do not add a `FetchErrorKind`. Reserved and private hosts keep
  `FetchErrorKind::PrivateNetworkBlocked` with their two existing messages; the
  browser path keeps `PolicyViolation::PrivateNetworkTarget`.
- Do not re-pin the advertised fingerprint; `d7968115e378f708` stays valid because no
  advertised schema or description changes.
- Keep fail-closed ordering. Every host rejected at the baseline is still rejected.
  The only acceptance change is the one this plan is named for: a trailing-dot form of
  an already-rejected name is rejected too, and `{"cutoff": null}` stops being an
  error. No non-null near-miss value may become acceptable.
- One classifier owns the hostname classes. Do not leave a second suffix list in the
  browser path, and do not make the browser path depend on resolution to reach a
  reserved-name decision.
- Tests stay keyless and network-independent. No new dependency is required for either
  the classifier change or the documentation guard.

## Expected production changes

| File | Change |
|---|---|
| `src/fetch/limits.rs` | Add `pub(crate) enum HostBlock { Reserved, Private }`, `pub(crate) fn normalize_hostname`, and `pub(crate) fn blocked_hostname` with reserved-first precedence. Rewire `hostname_block_error` as a message formatter over `blocked_hostname`; normalize `host_lower` at both derivation sites before the `localhost` equality branch. |
| `src/fetch/browser/intercept.rs` | `is_private_host` consumes `blocked_hostname` for the name classes and keeps its literal-address and metadata-endpoint cases; normalization happens before the localhost comparisons. |
| `src/mcp/tools/knowledge_brief.rs` | The near-miss scan treats a JSON null as absent and still rejects every non-null near-miss value. |
| `src/fetch/client.rs` | `PrivateNetworkBlocked(reason)` maps to `RedirectTargetBlocked(reason)` unchanged; the embedded-credentials arm is untouched. |
| `tests/docs_links.rs` (new) | Fragment-only Markdown link guard for tracked in-repository documentation. |
| `architecture/engines.md`, `docs/config.md` | Correct the two fragments. |

No CLI, configuration, MCP schema, provider, or packaging change is expected.

## Work packages

### WP1 — One hostname classifier with trailing-dot normalization (M)

Introduce `HostBlock { Reserved, Private }`, `normalize_hostname` (lowercase, strip
brackets, drop one terminal dot), and `blocked_hostname` (reserved first, then
private) in `src/fetch/limits.rs`. `hostname_block_error` becomes a formatter over
`blocked_hostname`, so the reserved and private messages keep their current wording
and precedence. `is_reserved_hostname` and `is_private_hostname` are either folded
into the classifier or kept as thin predicates over it; `is_private_hostname` stays
`pub(crate)` only if the browser path still needs it, and no suffix list may appear
twice.

Normalize at both `host_lower` derivations (`src/fetch/limits.rs:274`, `:355`) before the `== "localhost"` branch, and
pass the normalized value to the classifier and to the reserved/private message
format, so every spelling of a blocked name reports one message; canonical
spellings keep the wording they already emit. In `intercept.rs`, `is_private_host` returns true for any
`Some(HostBlock)` plus its existing literal and metadata cases, with normalization
applied before the localhost checks. Both classes stay
`PolicyViolation::PrivateNetworkTarget`; the browser contract is allow/deny, and the
message distinction is a fetch-error property.

Tests:

- Extend `fetch::limits::tests` with a table over
  `foo.invalid`, `foo.test`, `foo.localhost`, `service.internal`,
  `service.local`, `service.corp`, `localhost`, and `host-gateway`, each in canonical,
  trailing-dot, uppercase, and uppercase-plus-dot form; assert the same class and the
  same message for canonical and trailing-dot spellings in both validators.
- Assert reserved names produce the reserved message and private names the private
  message, and that `allow_private_network` still disables the block.
- Extend the `intercept` test module with `*.invalid`, `*.test`, and trailing-dot
  cases, and keep the existing `.internal`/`.private`/`.local` assertions.
- Keep `tests/property_fetch_limits.rs`: its documented reserved-name list must remain
  accurate, and the property that a rejected name stays rejected under a terminal dot
  is added there.

Acceptance: one classifier owns both classes; a trailing-dot name classifies exactly
as its canonical form in fetch validation and in browser interception; no hostname
rejected at the baseline is accepted, no hostname accepted at the baseline outside the
audited trailing-dot spellings is rejected, and no reserved name reaches DNS.

### WP2 — Null-tolerant `cutoff` (S)

In the `knowledge_brief` deserializer, treat a present null as absent:
`value.get(field).is_some_and(|v| !v.is_null())`. `{"cutoff": null}`, alone or with a
valid `cutoff_override`, must deserialize as though the key were omitted, and the
override must resolve normally. Non-null values of any type remain rejected with the
existing message and the existing `cutoff_override` field name.

Tests, in `tests/knowledge_delta_brief.rs`:

- `cutoff: null`, `knowledge_cutoff: null`, and `cutoff_date: null` each reach the
  request path instead of failing.
- `cutoff: null` with a valid `cutoff_override` applies the override.
- `cutoff: "2024"`, `cutoff: ""`, `cutoff: 2024`, `cutoff: []`, and `cutoff: {}`
  still fail with the near-miss message.
- The response shape, timestamp warning, and cutoff reporting are unchanged for every
  pre-existing case.

Acceptance: null is absent; every other near-miss value keeps its current rejection;
no other argument semantics change.

### WP3 — Redirect reason pass-through (S)

Map `PrivateNetworkBlocked(reason)` to `RedirectTargetBlocked(reason)` with no added
prefix. Keep the `credentials: ` arm and the `UnsupportedScheme` arm as they are.

Tests, in the existing `fetch::client` test module:

- A redirect to a reserved name yields the reserved wording and no
  `private network: ` prefix.
- A redirect to a private name yields the private wording.
- A redirect carrying embedded credentials still yields its existing prefixed
  message, proving the arms were not collapsed.
- All three still report `FetchErrorKind::RedirectTargetBlocked`.

Acceptance: the validator's reason survives the redirect mapping verbatim; the kind
and the other redirect arms are unchanged.

### WP4 — Anchor repair and fragment-only documentation guard (S)

Correct `architecture/engines.md:234` to
`../docs/provider-setup.md#keyless-tier-degradation-runbook` and
`docs/config.md:245` to `tool-matrix.md#legacy-call-compatibility-and-web-field-aliases`.

Add `tests/docs_links.rs`: walk the tracked Markdown set under the repository root,
skipping `bindings/`, `target/`, and `.git/`; for each relative link that carries a
fragment, require the target file to exist relative to the linking file and the
fragment to match a heading slug in that file. Ignore fenced code blocks, inline code
spans, absolute URLs, and links without a fragment. GitHub-style slugging: strip the
leading markers and a trailing `#` sequence, drop punctuation except hyphens and
spaces, map spaces to hyphens, and keep a duplicate counter. No new dependency; the
scanner and slugger are local functions in the test file.

The guard must be able to fail. Include unit tests inside the file for a synthetic
broken fragment, an ignored fenced block, an ignored inline code span, and a correct
fragment, so a slugger bug cannot turn the guard into a no-op.

Docs sync at implementation: register the new suite in `docs/test-inventory.md` and
`architecture/testing.md`, and name it in `skills/eggsearch-dev/SKILL.md`. Do not edit
those files during registration.

Acceptance: `cargo test --locked --test docs_links` passes on the repaired tree and
fails when either repaired fragment is reverted; no tracked documentation outside
`bindings/` is exempt.

Stop condition: if the guard reports false positives on publish-relative or generated
documentation after the exclusions, drop the guard, keep the two manual repairs, and
record that decision in the closure record rather than widening the exclusions until
the guard proves nothing.

### WP5 — Inventory refresh and probe-evidence retention (S)

At closure, measure `--all-features` and `--features mock` test and ignored counts on
the closure candidate and update `docs/test-inventory.md`. Do not refresh counts
during registration; they are a snapshot of the candidate, not a stable claim.

Add one rule to `plans/003-planning-process.md` §2.5 (Closure records), mirrored as
one line in the `closed` requirement list under § Status Vocabulary in
`skills/eggsearch-planning/SKILL.md`: live-trial evidence cited by a closure record is
retained under a named untracked path with the probe identifiers enumerated, and any
observation that was not retained is written as unverified rather than cited as
evidence. Hygiene rules still forbid tracked transcripts, so the path stays outside
the repository and only the path, identifier, and date appear in the record.

Acceptance: the inventory table matches the closure candidate; the retention rule
exists in one authoritative place with a pointer from the other; the M056 closure
record is not edited to retrofit it.

## Ordering and dependencies

1. WP1 — classifier, normalization, and both consumers.
2. WP2 and WP3 — independent of WP1 and of each other.
3. WP4 — anchor repair first, then the guard, so the guard starts green.
4. WP5 — closure-time measurement and the process rule.

Dependencies:

- Hard: none.
- Interface: M056's closed reserved/private decision and its closure record.
- Operational: none. Network and live checks are supplemental; every gate named here is
  keyless.

M057 is dependency-ready. M049, M054, and M055 remain separate milestones and are not
prerequisites. M056 stays closed and its plan and closure record are not amended.

## Verification

Focused, on the candidate:

```bash
RUSTUP_TOOLCHAIN=1.89.0 cargo fmt --all
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib 'fetch::limits::tests'
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib 'browser::intercept'
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --lib 'fetch::client'
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test property_fetch_limits
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test knowledge_delta_brief
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test web_fetch_integration
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --test docs_links
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --all-features --test mcp_tool_contract
```

Canonical and documentation gates:

```bash
RUSTUP_TOOLCHAIN=1.89.0 make check
RUSTUP_TOOLCHAIN=1.89.0 make docs-check
RUSTUP_TOOLCHAIN=1.89.0 make packaging-check
RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --features mock --test static_guards
python3 packaging/check-planning-consistency.py
git diff --check
```

Supplemental only: a keyless live probe recording a trailing-dot reserved URL, a
browser-path reserved URL, and a redirect to a reserved name. Do not make network
access a CI requirement; if the probe cannot produce the shape, record it as
unverified and rely on the deterministic tests for the gate.

## Compatibility and migration

- Callers sending `cutoff: null` now succeed instead of failing; no caller must change
  an input.
- Error messages change only where they currently misdescribe the cause: redirects to
  reserved or private names stop carrying the `private network: ` prefix, and
  trailing-dot names are classified like their canonical spelling.
- No tool, alias, argument, response field, error kind, provider, default set,
  configuration key, or MCP fingerprint changes. Hosts that pinned either reserved or
  private message text see the same two messages the validator already emits.

## Acceptance criteria

- One shared classifier owns the reserved and private hostname classes, and neither the
  fetch validator nor browser interception carries a second suffix list.
- `foo.invalid.`, `foo.test.`, `localhost.`, and `service.internal.` are rejected
  pre-DNS with the same class and message as their canonical forms, in both paths.
- No hostname rejected at the baseline is accepted, and no reserved name reaches DNS
  in the browser path.
- `{"cutoff": null}` is accepted as absent, including alongside a valid
  `cutoff_override`; every non-null near-miss value is still rejected.
- A blocked redirect preserves the validator's reason verbatim under
  `RedirectTargetBlocked`.
- `architecture/engines.md` and `docs/config.md` fragments resolve, and
  `tests/docs_links.rs` fails when a fragment is broken and passes on the candidate.
- `docs/test-inventory.md` counts match the closure candidate; the retention rule is
  recorded and the M056 record is untouched.
- Escalated kind, alias, schema, provider, default-set, trust, and fingerprint claims
  remain unchanged; `d7968115e378f708` still matches.
- `make check`, `make docs-check`, `make packaging-check`, planning consistency, and
  `git diff --check` pass on the candidate.
- A closure record at
  `plans/closure/agentic-retrieval-integrity/057-status.md` records the candidate SHA,
  requirement-to-evidence mapping, tests and gates, unchanged-contract evidence,
  deviations, and residual risks. M056 remains closed.

## Stop conditions and non-goals

Stop and register a separate decision or plan if a repair requires a new
`FetchErrorKind` or `PolicyViolation` variant, a tool, alias, argument, response field,
provider, or default-set change, an advertised schema or description change, a
fingerprint re-pin, or a message change for any hostname class that is not the
reserved/private pair this plan reconciles.

Do not broaden the milestone into general URL canonicalization, DNS resolver policy,
IDNA or punycode handling, IP-literal spelling variants, browser navigation policy
beyond the reserved-name class, link checking beyond in-repository fragments (no
external URLs, no file-existence sweep, no `bindings/` changes), or a redesign of the
knowledge-brief cutoff contract. WP2 must not relax the near-miss scan for any
non-null value, and WP1 must not begin accepting a host that the current code rejects.

This plan repairs only the five audited defects. It does not reopen M056 and claims no
implementation, test, live, or closure evidence before those steps are completed on a
candidate.
