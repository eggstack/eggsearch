# Repository Hardening M006 — Chromiumoxide Zero-Reqwest Closure

Status: blocked

Recommendation: blocked

Implementation candidate reviewed: `85322ac127033bdc4058177ea09cff67417308d0`

## Dependency gate

M003 is closed. The default dependency graph is reqwest-free, and the current
all-features graph has only one reqwest reverse-dependency root:

```text
reqwest 0.13.4
└── chromiumoxide 0.9.1
```

The upstream chromiumoxide 0.9.1 manifest still declares reqwest as a
non-optional dependency. No released upstream version exposes the plan's
optional HTTP DevTools discovery seam, so the browser launch-only integration
cannot remove this edge without an unapproved fork or behavior change.

Evidence reviewed on 2026-09-28:

- [chromiumoxide 0.9.1 package and release](https://crates.io/crates/chromiumoxide/0.9.1)
- [chromiumoxide v0.9.1 Cargo manifest](https://github.com/mattsse/chromiumoxide/blob/v0.9.1/Cargo.toml)
- `packaging/check-http-dependency-shape.py` records and enforces the present graph shape.
- Exact-candidate CI passed: run [`36467672672`](https://github.com/eggstack/eggsearch/actions/runs/36467672672), including the graph-shape guard.

## Cleared dependency and next action

M003's hard dependency is satisfied. The sole remaining blocker is an upstream
chromiumoxide release making HTTP discovery optional or providing an
equivalent injection seam without regressing `Browser::launch`. Keep this plan
blocked; do not fork. When an eligible upstream version is published, rebase
this plan's candidate, adopt it, add the launch/no-reqwest evidence, and rerun
the browser transport/profile and full repository gates.
