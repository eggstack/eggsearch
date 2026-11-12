# ADR-0009: Bounded Computational Challenge Solving for Keyless HTML Upstreams

Status: accepted
Date: 2026-11-11
Decision owners: project maintainers
Affected subsystems: search capability; keyless HTML engines; hardening and resource bounds; provider diagnostics.
Related: `plans/adrs/ADR-0006-keyless-deterministic-retrieval-and-optional-extraction.md`; `architecture/engines.md` § Scraper Fidelity; `architecture/hardening.md`; `plans/registry.md`.

## Context and forces

Eggsearch descends from a Rust rewrite of SearXNG. Its keyless HTML engines are a first-class
capability (ADR-0006), and the shipped defaults are scrapers. Two upstreams that the engine set
already targets now gate their search routes behind machine-verifiable proof-of-work challenges:

- `startpage` serves an Anubis challenge (`script#anubis_challenge`) on `/search`, `/sp/search`,
  and `POST /do/search`, including to browser user agents. The observed rule set on 2026-11-11 was
  `{"algorithm":"fast","difficulty":6}`.
- `mojeek` serves an ALTCHA challenge. The observed parameters on 2026-11-11 were
  `PBKDF2/SHA-256`, `cost = 8000`, `keyLength = 32`, with a 16-byte `keyPrefix` and a search
  bound of the published `maxCounter`.

SearXNG, the project this engine set derives from, ships solvers for both: `solve_anubis_pow` /
`solve_anubis_preact_challenge` and `solve_altcha` in `searx/utils.py`. SearXNG marks the affected
engines `inactive: true` by default because solving costs CPU, but the capability is part of its
engine contract rather than a prohibited act. Its Startpage engine additionally caps Anubis at
`max_difficulty = 4` and raises a captcha exception above that, so today it refuses the
difficulty-6 wall rather than solving it.

ADR-0006's decision bullet currently forbids CAPTCHA solving outright, alongside user-agent
spoofing, proxy rotation, and unbounded retries. That grouping conflates four different acts. A
bounded, deterministic, CPU-only proof-of-work computation against a challenge the server itself
published is not credential bypass, does not evade a rate limit, does not impersonate a browser,
and does not conceal an outage. Treating it as equivalent to those is stricter than the upstream
project the design follows, and it converts a solvable availability problem into permanent
provider loss.

The forces are: (a) preserve the first-class keyless posture instead of retreating to paid APIs;
(b) keep every solve bounded, observable, and deterministic so it cannot become a silent retry
loop; (c) avoid importing a browser or an external solving service; (d) keep failure classification
honest when a challenge is genuinely unsolvable within budget.

## Considered alternatives

1. Keep the blanket prohibition. Rejected: it forfeits upstreams whose gate is a published,
   purely computational puzzle, and diverges from the SearXNG behaviour the engine set mirrors.
2. Delegate to a browser (`browser` feature) for every challenged upstream. Rejected as the
   default path: it turns a ~1 s CPU task into a Chrome process, a profile, and a much larger
   runtime surface, and it still cannot pass an interactive CAPTCHA.
3. Use a third-party CAPTCHA-solving service or a human-in-the-loop step. Rejected: external
   dependency, credential exposure, non-deterministic, and unrelated to the proof-of-work case.
4. Solve computationally verifiable challenges in-process, under explicit CPU/time budgets, with
   cached pass cookies and unchanged failure classification: **selected**.

## Decision

- Bounded in-process solving of **computational, non-interactive** challenges is permitted for
  keyless HTML upstreams. The permitted class is a published proof-of-work: Anubis (`fast` and
  `preact` methods) and ALTCHA PBKDF2 challenges. Nothing else is in scope.
- The following remain prohibited, and are not reopened by this ADR: interactive CAPTCHA or visual
  puzzle solving, OCR of challenge images, third-party or human solving services, credential or
  login bypass, session or cookie theft, proxy rotation, user-agent spoofing, and retry widening to
  conceal an outage.
- Solving is **opt-in and off by default** through configuration. An operator who enables it
  accepts the CPU cost and the upstream terms for that provider. A disabled or exhausted solver
  leaves the provider's behaviour exactly as today.
- Every solve is bounded. The bound covers a challenge-difficulty ceiling, a total attempt
  ceiling, a wall-clock ceiling, and a per-provider solve rate. A solver that reaches a bound
  returns a classified failure; it never retries in a loop and never blocks the request past the
  search deadline.
- A successful solve yields a pass cookie that is cached per provider for a bounded TTL and
  replayed on subsequent requests. Solving is not repeated per subquery. Cache entries are
  process-local, never persisted, and never shared across providers.
- Solver output is deterministic for a given challenge. Any client-supplied nonce, counter, or
  timing field that the upstream does not verify is a fixed constant, not a random value, so tests
  are reproducible.
- Failure classification is unchanged: an unsolved, unsupported, or budget-exhausted challenge is
  `EngineError::Blocked`, which maps to `blocked` and `FailureClass::Blocked`, and never to an
  empty result set. A challenge that is solved but yields no parseable rows is still `ParseFailed`.
- Solving must never be used to reach a provider that is otherwise unauthorized, rate-limited, or
  absent from configuration. It is not a routing change.
- The dependency budget for this capability is limited to offline, pure-computation crates. No new
  network, TLS, browser, or service dependency may be added for it; `sha2` is already a direct
  dependency and the transport (`eggfetch-core`) already provides cookie handling.

## Consequences and tradeoffs

Advantages: keeps keyless upstreams reachable without an API key or a browser, aligns the engine
contract with the SearXNG lineage, and localizes bot-mitigation handling in one reviewed module
instead of per-engine ad hoc code.

Costs: CPU burn on a solve (measured 2026-11-11: an Anubis `fast` challenge needs `16^difficulty`
expected SHA-256 candidates — difficulty 4 ≈ 65.5K and 0.05 s, difficulty 6 ≈ 16.8M and ~6.7 s on
one core at 0.4 µs per candidate; an ALTCHA challenge at `cost = 8000` took 292 counter steps and
0.42 s, worst case 1000 steps ≈ 1.4 s). Anubis difficulty is risk-scored per client: the honest
`eggsearch` user agent received difficulty 6 while a Chrome user agent received 4, so the shipped
ceiling must be chosen for the honest agent's cost, not the easiest observed case. Also a small
dependency delta, and a new configuration surface that must be
documented and defaulted off. Success is also not guaranteed: upstreams may raise difficulty,
change methods, or bind challenges to additional signals, and the solver must fail closed when
that happens.

## Compatibility, security, and migration

No new MCP tools and no response-schema change. The ten-tool contract, `eggfetch-core` transport
ownership, bounded-response rules, SSRF/redirect policy, and cache isolation are unchanged. Solver
code paths perform only local computation plus requests the engine would already make; every
request remains behind `read_bounded_body`/`read_bounded_response`. Cookie values are secrets-like
and must never be logged, serialized into diagnostics, or written to fixtures. A challenge is
answered at most once per provider per TTL, so solving cannot amplify request volume.

## Verification and status

Adoption authorizes the capability; it does not by itself change production behaviour. Enabling it
requires a registered implementation plan with synthetic challenge fixtures, budget-exhaustion
tests, a mock transport that issues a challenge and then serves a result page, and a live
verification record per provider in `tests/fixtures/engines/README.md`. Until that plan closes, the
engines continue to report `blocked` exactly as they do now.
