# Python Native Library M006 — Distribution Closure Reconciliation

Status: closed
Candidate: `faa1da2e5d5a8c38045001fec1ae1a230bc36bde`
Implementation plan: `plans/implementation/python-native-library/006-m004-exact-sha-distribution-closure.md`
Roadmap: `plans/subsystems/python-native-library-roadmap.md` M006

## Reconciliation

M006 executed the exact-SHA qualification M004 was missing. The final
candidate is the M007 repair SHA `faa1da2`, qualified by hosted run
`38617598636` (5/5 wheel builds, 20/20 installed-wheel smokes across five
targets × CPython 3.11–3.14, extracted-sdist offline rebuild with
clean-venv consumer run) plus the local replication in `004-status.md`.

Superseded runs and their dispositions:

- `38522548258` on baseline `1434c1e` (27 green jobs): predates M005/M007
  bytes; historical signal only, never qualification for this candidate.
- `38608502185` on `c8bd030`: deficiency evidence — 3 Windows config-test
  failures from shared-CWD fallback files plus a Unix-only filename
  assert; repaired test-only in `748ae59`.
- `38611932990` on `748ae59` and instrumented `38614592873` on `d5e4dcb`:
  deficiency evidence — 1 Windows failure each (`alpha` snippet miss),
  root-caused to the `safe_open_relative` non-unix directory-open defect
  now owned by M007.
- `38617598636` on `faa1da2`: the qualifying run. Overall conclusion
  `failure` comes solely from the audit job on upstream rustls advisories
  RUSTSEC-2026-0346–0349 (recorded Medium residual, dependency-process
  owned); all 26 artifact jobs succeed.

Retained history: M001–M003 closures stand untouched; M005 closed at
`baf00cf`; M007 closed here alongside. Root compatibility holds
(`cargo package --list` 291 entries, `cargo publish --dry-run` clean,
`make packaging-check` green, drift guard and egress qualification green
on `faa1da2`, `CI` red only at the standing dependency-policy gate).
`Published to PyPI: no` — publication remains separately authorized and
would require fresh exact-SHA qualification of whatever bytes it ships.

M006 is closed. M004 closes in the same commit via `004-status.md` and
M007 via `007-status.md`, completing M001–M007.
