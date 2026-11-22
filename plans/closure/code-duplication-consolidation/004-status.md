# M004 Shared Engine JSON Transport — Closure Evidence

Status: closed

Candidate: the working tree on `feat/auto-20261117-044c4885` above baseline
`fb9d6538f8ea2e21e08add43fb3f71ebe6ceaa64`, uncommitted because the request did not ask for a
commit. The canonical gate ran on exactly this tree; its output and the diff fingerprint are in
`plans/closure/code-duplication-consolidation/gate.md`.

Source plan: `plans/implementation/code-duplication-consolidation/004-shared-engine-json-transport.md`

Roadmap: `plans/subsystems/code-duplication-consolidation-roadmap.md` M004

Decision authority: no ADR. Bounded-I/O ownership (ADR-0002) is unchanged; no `reqwest` in
production source.

## Change

- `src/meta/engines/api.rs` (110 lines) adds
  `get_json_bounded<T>`: one GET builder, caller query pairs and headers, `engine_timeout`,
  `map_request_error`, `BadStatus`, `read_bounded_body`, and
  `ParseFailed { reason: "invalid JSON: {e}" }`.
- The nine forge engines now build parameters and call the helper; URL construction, query
  pairs, auth header names and values, the `max_results == 0` early return, and `convert`
  are unchanged.
- `truncate_body` exists once in `api.rs`; the eight byte-identical copies and their
  duplicated tests are gone. The five identical test-only `param` helpers are one
  `#[cfg(test)]` function in `api.rs`.
- `parse_owner_repo_from_url` stays duplicated: the two GitLab bodies are not byte-identical
  after the move (one carries additional inline documentation).

## Line counts

| Engine | Before | After |
|---|---:|---:|
| gitea_code | 509 | 495 |
| gitea_issues | 488 | 471 |
| gitea_releases | 437 | 421 |
| github_code | 526 | 514 |
| github_issues | 700 | 613 |
| github_releases | 559 | 492 |
| gitlab_code | 514 | 466 |
| gitlab_issues | 556 | 508 |
| gitlab_releases | 618 | 571 |

## Equivalence evidence

- `cargo test --locked --all-features --test engine_api_conformance`: 7 passed. The suite
  asserts status variant/status/engine, `ParseFailed` reason contains `invalid JSON`, zero
  results issue no request, the per-page limit, the documented auth header, and skipped invalid
  URLs for all nine engines.
- `cargo test --locked --all-features --lib engines::`: 602 passed.
- `cargo test --locked --all-features --test dispatch_fault_injection`: 34 passed.
- Grep evidence: all nine forge files call `get_json_bounded`; no `let resp = client` block
  remains in them; the only `truncate_body` and `param` definitions are in `api.rs`.

## Deferred

Registry, advisory, scholarly, keyless API, and other non-forge engines keep their current
shape. Their remaining identical blocks are recorded for a future milestone; this milestone did
not widen the helper into a request-builder abstraction.
