# Binary Distribution and Deployment Roadmap

Status: closed

Long-term references:

- `plans/000-long-term-specification.md#6`
- `plans/001-terminology-and-domain-model.md#6`
- `plans/002-long-term-roadmap.md`

Related ADRs: none.

## 1. Purpose and ownership boundary

Ship installable, self-updating, supervised eggsearch binaries with
loopback-only persistent MCP and agent/IDE integration. Owns `src/platform.rs`,
`src/update.rs`, `src/startup.rs`, `packaging/`, and installer/updater
contracts. Does not own search ranking or evidence semantics.

## 2. Work classification

### Invariants

- Single-SKU policy; seven-target matrix with exact 16-asset assembly;
  packaging files and docs stay synchronized (`make packaging-check`).

### Capabilities

- Release binaries + bootstrap installers; binary-first self-update;
  persistent `mcp serve`; startup supervision/croncheck/restart;
  `integrate` print-by-default with `--apply` atomic backed-up mutation.

### Infrastructure

- Shared build/smoke/checksum/assembly logic between qualify and release;
  release preflight and installer fallback/fail-closed coverage.

### Polish

- Service deployment docs and integration examples.

## 3. Non-goals

OS package pipelines, container-primary install, unattended auto-update,
non-loopback exposure, board-tuned assets, mandatory MCPB.

## 4. Current state

Closed. First binary-enabled release published as `v0.3.9` after the
phase-16 corrective hardening (qualified commit
`0cbbeee79a34b7f6d2d226cef535096adc58b4c3`, qualification run
`34653366561`, tagged release run `34655458760`).

## 5. Target architecture

Achieved: one machine-checked target/asset contract across workflow,
installers, updater, docs, and assembly with exact asset-set equality.

## 6. Dependency graph

```text
M001 release binaries and installers
    |
    +--> M002 binary-first self-update
    |
    +--> M003 persistent Streamable HTTP MCP ------+
    |                                              |
    +--> M004 startup supervision -----------------+
                   |                               |
                   `--> M005 agent/IDE integration--+
                               |
                               `--> M006 first-release hardening (corrective)
```

M001 is hard for M002; M002/M003/M004 are operational for M006 publication.

## 7. Milestones

### M001 — Release binaries and bootstrap installers

Historical plan: `plans/archive/phase-6-release-binaries-and-bootstrap-installers.md`.

### M002 — Binary-first self-update

Historical plan: `plans/archive/phase-7-binary-first-self-update.md`.

### M003 — Persistent Streamable HTTP MCP

Historical plan: `plans/archive/phase-8-persistent-streamable-http-mcp.md`.

### M004 — Startup supervision, croncheck, restart

Historical plan: `plans/archive/phase-9-startup-supervision-croncheck-and-restart.md`.

### M005 — Agent/IDE integration and deployment closure

Historical plan:
`plans/archive/phase-10-agent-ide-integration-and-deployment-closure.md`.

### M006 — First binary release hardening and cutover (corrective)

Historical plan:
`plans/archive/phase-16-first-binary-release-hardening-and-cutover.md`.
Closed the post-`v0.3.8` landing gap; `v0.3.8` was not backfilled.

## 8. Cross-cutting requirements

No search-behavior change. Installer fallback is fail-closed and covered by
deterministic tests independent of a live release. Docs never advertise a
dead latest-release URL.

## 9. Verification strategy

Seven-target qualify plus exact assembly inspection, Unix/Windows bootstrap
smoke against published installers, and `update --check` resolution proof.

## 10. Risks and decision points

None open. Later transport changes require re-qualification under release rules.

## 11. Completion definition

Closed: qualification and tagged release share one code path and the exact
qualified SHA is the tagged SHA.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 | closed | `plans/archive/phase-6-release-binaries-and-bootstrap-installers.md` | `plans/closure/binary-distribution-deployment/001-status.md` | — |
| M002 | closed | `plans/archive/phase-7-binary-first-self-update.md` | `plans/closure/binary-distribution-deployment/001-status.md` | — |
| M003 | closed | `plans/archive/phase-8-persistent-streamable-http-mcp.md` | `plans/closure/binary-distribution-deployment/001-status.md` | — |
| M004 | closed | `plans/archive/phase-9-startup-supervision-croncheck-and-restart.md` | `plans/closure/binary-distribution-deployment/001-status.md` | — |
| M005 | closed | `plans/archive/phase-10-agent-ide-integration-and-deployment-closure.md` | `plans/closure/binary-distribution-deployment/001-status.md` | — |
| M006 | closed | `plans/archive/phase-16-first-binary-release-hardening-and-cutover.md` | `plans/closure/binary-distribution-deployment/001-status.md` | — |
