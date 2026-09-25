# Optional Outbound Routing Roadmap

Status: closed

Long-term references:

- `plans/000-long-term-specification.md#4`
- `plans/001-terminology-and-domain-model.md#4`
- `plans/002-long-term-roadmap.md`

Related ADRs:

- `plans/adrs/ADR-0002-eggfetch-transport-ownership.md`
- `plans/adrs/ADR-0003-outcome-b-optional-outbound-routing.md`

## 1. Purpose and ownership boundary

Add an opt-in listener-free HTTP/SOCKS proxy-chain route for provider
upstreams only, without weakening the resolved-address/SSRF contract.
Owns `src/fetch/egress.rs` (`EggressDialer`), provider route construction,
and the egress qualification contract. Does not own destination TLS,
pooling, decompression, redirects, or deadlines (eggfetch retains those).

## 2. Work classification

### Invariants

- Outcome B narrow route; dynamic `FetchClient` targets stay direct and
  pinned; forge/updater/loopback/rmcp/browser stay direct.
- Base protocols `http`/`socks4`/`socks5` only; credentials env-indirected
  and redacted; malformed-proxy fail-closed.
- Single SKU; no published egress-enabled binary; source-build opt-in only.

### Capabilities

- Operator-configured proxy chain for fixed/operator-owned provider upstreams.

### Infrastructure

- `EggressDialer` beneath the eggfetch Dialer seam; routed-redirect
  ownership; pool reuse above the dialer; stalled-handshake bounds.

### Polish

- Qualification-contract hardening and test-hygiene cleanup.

## 3. Non-goals

eggress-embed, pproxy compatibility, SSH, QUIC/H3, UDP, legacy crypto,
insecure TLS, service listeners, direct-fallback behavior, second SKU.

## 4. Current state

Closed. Outcome B implemented (phase 25), corrective closure and
qualification (phase 26, candidate `8cfe9d873b55c1cda176c9c8f2af0182714ae431`,
egress lane run `35806105807`, default release run `35806121182`),
maintenance closure and contract hardening (phase 27, candidate `6414a72`,
hardened run `35810222447`), and hygiene cleanup (phase 28, tests/docs only).
No egress-enabled binary is published.

## 5. Target architecture

Achieved: Eggress limited to physical TCP hop establishment; eggfetch
remains the HTTP/pooling/TLS/deadline owner; egress qualify matrix in exact
set equality with `packaging/release-targets.txt`.

## 6. Dependency graph

```text
M001 optional proxy-chain integration (hard: eggfetch 0.2.0)
    |
    `--> M002 corrective closure + qualification (hard, operational)
              |
              `--> M003 maintenance closure + contract hardening (hard)
                        |
                        `--> M004 documentation + test hygiene (hard, non-functional)
```

## 7. Milestones

### M001 — Eggress 1.0.8 optional outbound proxy-chain integration

Historical plan:
`plans/archive/phase-25-eggress-1.0.8-optional-outbound-proxy-chain-integration.md`.

### M002 — Corrective closure and qualification

Historical plan:
`plans/archive/phase-26-egress-corrective-closure-and-qualification.md`.

### M003 — Maintenance closure and qualification-contract hardening

Historical plan:
`plans/archive/phase-27-egress-maintenance-closure-and-qualification-contract-hardening.md`.
Terminal maintenance/qualification closure baseline.

### M004 — Documentation and test-hygiene cleanup

Historical plan:
`plans/archive/phase-28-egress-documentation-and-test-hygiene-cleanup.md`.
Tests/planning/docs only; no production, packaging, or qualification change.

## 8. Cross-cutting requirements

IPv6 proxy-hop validation; connection-pool reuse proof; deterministic
destination TLS/SNI through CONNECT; Brotli+gzip limits; authenticated-path
credential non-forwarding; expanded workflow path triggers on the
provider-route seam.

## 9. Verification strategy

Routed and excluded-path guards, authenticated/malformed proxy regressions,
`egress-feature-qualify` lane, default seven-target release qualification,
and `packaging/check-egress-qualify-contract.sh` plus
`tests/egress_qualify_contract.rs`.

## 10. Risks and decision points

None open. Optional follow-on work is closed; new egress scope requires a new
roadmap and ADR review.

## 11. Completion definition

Closed: Outcome B architecture with exact-matrix qualification and no
egress-enabled published binary.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 | closed | `plans/archive/phase-25-eggress-1.0.8-optional-outbound-proxy-chain-integration.md` | `plans/closure/optional-outbound-routing/001-status.md` | — |
| M002 | closed | `plans/archive/phase-26-egress-corrective-closure-and-qualification.md` | `plans/closure/optional-outbound-routing/001-status.md` | — |
| M003 | closed | `plans/archive/phase-27-egress-maintenance-closure-and-qualification-contract-hardening.md` | `plans/closure/optional-outbound-routing/001-status.md` | — |
| M004 | closed | `plans/archive/phase-28-egress-documentation-and-test-hygiene-cleanup.md` | `plans/closure/optional-outbound-routing/001-status.md` | — |
