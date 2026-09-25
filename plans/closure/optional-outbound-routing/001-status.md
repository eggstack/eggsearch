# Optional Outbound Routing — Closure Status

Status: closed

Source implementation plans:

- `plans/archive/phase-25-eggress-1.0.8-optional-outbound-proxy-chain-integration.md`
- `plans/archive/phase-26-egress-corrective-closure-and-qualification.md`
- `plans/archive/phase-27-egress-maintenance-closure-and-qualification-contract-hardening.md`
- `plans/archive/phase-28-egress-documentation-and-test-hygiene-cleanup.md`

Source subsystem roadmap:

- `plans/subsystems/optional-outbound-routing-roadmap.md`

Repository baseline reviewed: `6414a72ead3d059e53205892f8200afacee2f5e4`
(terminal maintenance baseline; hardened qualify run `35810222447`)

Implementation commits or pull requests:

- M001 Outcome B implementation; M002 `8cfe9d873b55c1cda176c9c8f2af0182714ae431`
  (egress lane `35806105807`, default release `35806121182`); M003 `6414a72`
  (hardened run `35810222447`); M004 tests/planning/docs only.

## 1. Executive finding

The optional routing workstream is complete and closed: Outcome B narrow
route with source-build opt-in, corrective qualification, contract
hardening, and hygiene cleanup. No egress-enabled binary is published. Phase
26's record is the authoritative runtime closure evidence; phase 27 is the
terminal maintenance baseline; phase 28 changed no production behavior.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Narrow provider-only route | Route construction guards | pass | Dynamic fetch stays direct |
| Pool reuse above dialer | Reuse proof (1 accept / 2 requests) | pass | Phase-26 evidence |
| Handshake/deadline bounds | Cancellation regressions | pass | — |
| Destination TLS/SNI via CONNECT | Deterministic TLS tests | pass | Mismatch fails |
| Authenticated paths + redaction | Positive HTTP/SOCKS paths | pass | Non-forwarding proven |
| Malformed-proxy fail-closed | Negative regressions | pass | — |
| Exact-matrix qualify contract | `check-egress-qualify-contract.sh` + `egress_qualify_contract.rs` | pass | Set equality enforced |
| Seven-target + MSRV lanes | Runs `35806105807`, `35806121182`, `35810222447` | pass | SHA-specific |

## 3. Production implementation evidence

`EggressDialer` beneath the eggfetch seam; provider engines routed via
`build_http_client_with_egress`; excluded paths direct; IPv6 hop validation.

## 4. Verification executed

Egress qualify lane, default release qualification, `make release-check`
including strict publish dry-run, and host-grammar regressions under both
`--features mock` and `--all-features` via `enabled = false` fixtures.

## 5. Invariant review

Single SKU, default-feature policy, and phase-25/26 security architecture
unchanged by maintenance/hygiene passes.

## 6. Failure and recovery review

Stalled-handshake cancellation, routed-redirect ownership, and malformed-
proxy failure covered.

## 7. Migration and compatibility review

No migration; default binaries exclude the feature.

## 8. Security review

Credential indirection/redaction, excluded-path guards, and feature-budget
guards green.

## 9. Documentation and operations

Stale active labels corrected; host-syntax coverage decoupled from the
optional feature gate without weakening the enabled-route gate.

## 10. Unresolved findings

None.

## 11. Roadmap disposition

Milestones M001-M004 closed; workstream closed with no successor.

## 12. Registry updates

Covered by the planning-convention migration; roadmap marked closed.
