# M057 Closure — M056 Audit Follow-Up Correctives

Status: closed
Date: 2026-11-27
Candidate: commit `99b2532` plus the M057 working tree (all five work packages + live probe), landed as one closure commit on `feat/auto-20261126-82ca75e6`
Plan: `plans/implementation/agentic-retrieval-integrity/057-m056-audit-follow-up-correctives.md` (now `closed`; this record closes it)
Baseline: commit `99b2532` (`feat/auto-20261126-82ca75e6`, macOS arm64)
Corrects: M056. M056 remains `closed`; its plan and `plans/closure/agentic-retrieval-integrity/056-status.md` were not edited.

## Requirement-to-evidence matrix

| WP | Requirement | Evidence | Result |
|---|---|---|---|
| WP1 | One classifier owns the reserved and private hostname classes; trailing-dot names classify like their canonical spelling in fetch validation and browser interception; no reserved name reaches DNS in the browser path | `src/fetch/limits.rs` adds `HostBlock { Reserved, Private }`, `normalize_hostname` (lowercase, strip one bracket pair, drop one terminal dot), and `blocked_hostname` with reserved-first precedence; `hostname_block_error` is a formatter over `blocked_hostname`; both `host_lower` derivations normalize before the `localhost` equality branch while the IP-literal parse keeps the raw host. `src/fetch/browser/intercept.rs::is_private_host` consumes `blocked_hostname` plus its literal and metadata cases and carries no second suffix list; `is_private_hostname` became module-private. Tests: `fetch::limits::tests` 53/53 including both `*_classifies_hostname_spellings_identically` tables over `foo.invalid`, `foo.test`, `foo.localhost`, `service.internal`, `service.local`, `service.corp`, `localhost`, `host-gateway` in canonical, trailing-dot, uppercase, and uppercase-plus-dot forms; `browser::intercept` 31/31 with new `.invalid`/`.test`/`.localhost` and trailing-dot cases; `property_fetch_limits` 14/14 with the terminal-dot property over the documented SSRF suffix list. The zone-ID loopback test still rejects. Supplemental live identifiers `p1-trailing-dot` and `p2-browser-reserved` (retained 2026-11-27). | Pass |
| WP2 | A present-but-null `cutoff` is absent; every non-null near-miss value is still rejected | `src/mcp/tools/knowledge_brief.rs` scan is `value.get(field).is_some_and(|value| !value.is_null())`. Tests in `tests/knowledge_delta_brief.rs`: `null_cutoff_fields_are_treated_as_absent` (all three keys, no `invalid_semantic_value`), `null_cutoff_with_override_applies_the_override`, and `non_null_cutoff_near_misses_still_fail_closed` over `"2024"`, `""`, `2024`, `[]`, `{}` with the `cutoff_override` repair hint. Suite 22/22. | Pass |
| WP3 | A blocked redirect preserves the validator's reason verbatim under `RedirectTargetBlocked`; other arms unchanged | `src/fetch/client.rs::map_redirect_validation_error` maps `PrivateNetworkBlocked(reason)` to `RedirectTargetBlocked(reason)`. Four tests in `src/fetch/client_tests.rs`: reserved reason, private reason, unchanged `credentials: ` prefix, and an end-to-end mock redirect to `never-resolves.invalid` that reports `reserved hostname can never resolve: never-resolves.invalid` under `FetchErrorKind::RedirectTargetBlocked`. `fetch::client` 53/53. Supplemental live identifier `p3-redirect-reserved` (retained 2026-11-27). | Pass |
| WP4 | Two fragments resolve; a fragment-only guard fails on a broken fragment and passes on the candidate | `architecture/engines.md` now targets `#keyless-tier-degradation-runbook`; `docs/config.md` targets `#legacy-call-compatibility-and-web-field-aliases`. `tests/docs_links.rs` walks every Markdown file except `bindings/`, `target/`, and `.git/`, skips fenced code, inline code, absolute URLs, and fragment-less links, and resolves fragments with GitHub-style slugs and duplicate counters. 7/7 pass. Negative control: reverting the `engines.md` fragment makes `in_repository_markdown_fragments_resolve` FAIL with `architecture/engines.md -> ../docs/provider-setup.md#keyless-tier-degradation: missing fragment`; restoring it returns 7/7. | Pass |
| WP5 | Inventory counts match the candidate; the retention rule exists once with a pointer; M056's record is untouched | `docs/test-inventory.md` records `--all-features` 6711 / 30 ignored and `--features mock` 6647 / 6 ignored, measured with `cargo test -- --list` and `--list --ignored` on this candidate. `plans/003-planning-process.md` §2.5 carries the retention rule; `skills/eggsearch-planning/SKILL.md` § Status Vocabulary points to it in one `closed` requirement line. M056's plan and closure record are unchanged. | Pass |

## Message-quoting deviation (WP1)

The plan's work-package prose said the messages keep quoting the original `host_str`, while its acceptance rule requires canonical, trailing-dot, and uppercase spellings to report the same message. Quoting the raw host cannot satisfy both, because `foo.invalid` and `foo.invalid.` differ. The implementation quotes the normalized host in the reserved, private, and localhost messages. Canonical lowercase messages are byte-identical to the baseline; uppercase input now renders lowercase in the message. The plan sentence was corrected in the same change so the next reader does not revert the messages.

## Verification run

- `make check` (Rust 1.89, canonical gate): exit 0. 100 test-binary results reported `ok`, 0 failed; includes `fmt --check`, clippy `--all-targets --all-features -D warnings`, the no-default-features check, all-features tests, hygiene, dependency-policy, and packaging checks (including `packaging/check-contract.sh`, packaged-assets, egress qualification, and `planning-consistency: ok`).
- `make docs-check`: exit 0.
- Focused suites: `fetch::limits::tests` 53/53; `browser::intercept` 31/31; `property_fetch_limits` 14/14; `knowledge_delta_brief` 22/22; `fetch::client` 53/53; `web_fetch_integration` 136 passed, 1 ignored; `docs_links` 7/7; `mcp_tool_contract` 23/23; `static_guards` 69/69.
- `git diff --check`: clean.
- Live probe (supplemental, not a gate): retained at `/tmp/m057-live-probe/` with identifiers `initialize`, `p1-trailing-dot`, `p2-browser-reserved`, and `p3-redirect-reserved`, date 2026-11-27. Each identifier returned the fail-closed outcome it targets: the trailing-dot spelling was rejected as a reserved hostname, the browser path was denied before DNS, and the loopback redirect reported the reserved-hostname reason under the redirect kind. Under `plans/003-planning-process.md` §2.5 the retained path, these identifiers, and the date are the citation; no transcript is reproduced here. The deterministic tests above remain the gate.

## Invariants held

- The advertised surface is unchanged: `mcp_tool_contract` still matches fingerprint `d7968115e378f708`; no tool, alias, argument, schema, provider, default set, or trust-framing change.
- No new `FetchErrorKind` and no `PolicyViolation` variant; reserved and private hosts keep `FetchErrorKind::PrivateNetworkBlocked` with their two existing messages, and the browser path keeps `PolicyViolation::PrivateNetworkTarget`.
- Fail-closed ordering is preserved. The only new rejections are the audited trailing-dot spellings; `{"cutoff": null}` is the only new acceptance.
- `allow_private_network` still disables both hostname blocks, and `allow_localhost` still governs the `localhost` branch.
- M056 remains `closed` with its plan and closure record untouched.

## Known limitations

- Quoting the normalized host means an uppercase input renders lowercase in the error message. This is the consequence of the one-message-per-class acceptance rule.
- `tests/docs_links.rs` checks inline Markdown links only. Reference-style link definitions and setext headings are not scanned; links without a fragment and external URLs are out of scope by design.
- Exactly one terminal dot is dropped, so a double-dot name is not normalized.
- The candidate is a working tree on `99b2532`. This record, the plan, roadmap, and registry status flips land in one closure commit; the probe evidence stays outside the repository and is not committed.

## Recommendation

Closed. M057 is `closed` in the plan, `plans/registry.md`, and the roadmap, committed together with this record. M056 remains `closed` with its plan and closure record untouched.
