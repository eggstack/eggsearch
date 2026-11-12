# ADR-0010: Challenge Solver Coverage and Cost Policy

Status: accepted
Date: 2026-11-12
Decision owners: project maintainers
Affected subsystems: search capability; keyless HTML engines; hardening and resource bounds; provider diagnostics.
Related: `plans/adrs/ADR-0009-bounded-challenge-solving.md` (superseded in part); `architecture/engines.md` § Challenge solving; `plans/implementation/agentic-retrieval-integrity/013-challenge-solver-conformance-corrections.md`; `016-anubis-metarefresh-solver.md`.

## Context and forces

ADR-0009 permitted in-process solving of published computational challenges but named the
permitted methods explicitly: Anubis `fast` and `preact`, plus ALTCHA PBKDF2. Two problems surfaced
during the first live integration and the review that followed.

**Method coverage was narrower than the scheme.** Anubis registers four challenge methods in a
pluggable registry: `fast` (SHA-256 proof-of-work), `slow` (registered as a deprecated alias of
`fast`, using the identical scheme), `preact` (a browser component), and `metarefresh` (a
meta-refresh path that echoes published data after a wait, intended for clients without
JavaScript). Naming only two of them produced a conformance defect: a deployment that publishes
`slow` is solvable by the same code path, yet the solver refuses it as an unsupported scheme and
the provider reports `blocked` for a reason that is not true. Refusing a published method because
of its label is not a safety control.

`metarefresh` is a different case. It is a time challenge rather than a proof-of-work: the client
echoes published data after honouring a minimum elapsed time, and performs no hashing at all.
ADR-0009's permission was written for proof-of-work, so covering it is a genuine scope extension
rather than a naming fix, and it needs this ADR rather than an implementation plan alone.

**Cost asymmetry changes the policy question.** Measured against a live Startpage wall on
2026-11-11: difficulty 6 required 23.07M SHA-256 candidates and completed in 3.05 s on one core in
a Rust release build. Anubis's own documentation places difficulty 6 at roughly 15 minutes for a
JavaScript client, because it assumes a browser solver. A compiled solver therefore pays about
1/300th of the intended price.

That asymmetry does not make solving wrong — the published cost is paid in full and bounded — but
it relocates the control. Once the solver is three orders of magnitude cheaper than assumed, the
only thing limiting how fast an operator can pass challenges is the operator's own ceiling. Making
the search faster would remove the last cost that tracks request volume, and the wall would stop
deterring anything.

## Considered alternatives

- **Keep ADR-0009's method list and add methods case by case.** Rejected: it produced the `slow`
  refusal, and each new method would need another superseding record for what is one registry.
- **Permit any method string the upstream publishes.** Rejected: `metarefresh` and any future method
  must still be evaluated against the prohibition list before being implemented, or an unknown
  method becomes an unbounded instruction channel.
- **Parallelise the nonce search to fit harder challenges.** Rejected on the cost argument above.
- **Lower `max_difficulty` defaults so parallelism is unnecessary.** Partly adopted already: the
  default ceiling of 5 keeps Startpage blocked, and difficulty 6 is an explicit operator opt-in
  that also requires raising `max_attempts` and the timeouts.

## Decision

- **Coverage follows the published registry, not a hardcoded name list.** The permitted class is
  every challenge method a supported deployment publishes and documents: Anubis `fast`, `slow`
  (treated as `fast`), `preact`, and `metarefresh`, plus ALTCHA KDF challenges. A method that is
  not in that evaluated set still fails closed as an unsupported scheme. Adding a further method
  requires evaluating it against the prohibition list and recording the outcome; it does not
  inherit permission from this list.
- **`metarefresh` is permitted as a time challenge**, not as a proof-of-work. It performs no
  hashing, echoes only data the server published for that challenge, and honours the server's
  minimum elapsed time inside the configured wall-clock ceiling.
- **The nonce search stays single-threaded and single-process.** No parallel, batched, GPU, or
  distributed solving is permitted as long as this ADR stands. This is a deliberate cost policy,
  not an implementation limitation: the ceiling must remain the binding control on solve rate.
  Changing it requires a superseding ADR, not a milestone.
- **Cost asymmetry is a first-class design constraint.** Any future change that materially reduces
  the per-candidate cost of a solve must be evaluated against this policy before landing.
- **`max_difficulty` remains the operator's explicit acceptance of cost.** Raising it is the
  supported way to buy a harder challenge, and the documentation must state the three-knob
  relationship (`max_difficulty`, `max_attempts`, and the timeouts) rather than implying one knob
  is enough.
- **Every ADR-0009 prohibition stands unchanged and is not reopened**: interactive CAPTCHA or
  visual puzzle answering, OCR of challenge images, third-party or human solving services,
  credential or login bypass, session or cookie theft, proxy rotation, user-agent spoofing, and
  retry widening to conceal an outage. Determinism, opt-in default-off behaviour, the four
  ceilings, per-provider cookie caching, and `blocked`-never-empty classification are also
  unchanged from ADR-0009.

## Consequences and tradeoffs

Advantages: the solver stops reporting a false `unsupported scheme` for a method it can already
solve; a no-JavaScript path becomes available that costs nothing to compute, which is both cheaper
and closer to upstream intent than grinding hashes; and the cost policy is stated where a future
contributor will read it, instead of being an undocumented choice in a milestone.

Tradeoffs: covering `metarefresh` widens the permitted class, so the review obligation for each
future method is explicit rather than implied. Keeping the solver single-threaded means a
difficulty-7 challenge is likely out of reach inside any sane deadline — that is accepted, and the
provider simply stays `blocked`. The measured 300× advantage over a browser persists; this ADR
constrains it from growing rather than removing it.

## Compatibility, security, and migration

No MCP tool, response-schema, or wire-string change. Coverage is internal to
`src/meta/challenge/`; configuration keys are unchanged, so no migration is required and a config
written for ADR-0009 keeps its meaning. Every prohibition above still applies, and the solver still
performs only local computation plus requests the engine would already make, behind the existing
bounded-body and deadline rules. Pass cookies remain secrets-like. `metarefresh` echoes only data
the upstream published in that challenge response; it must not echo request headers, cookies, or
any operator-supplied value.

## Verification and status

Adoption authorizes the wider class; it does not by itself change behaviour. Implementation lands
through registered plans with synthetic fixtures per method, budget-exhaustion tests, an
unsupported-method fail-closed test, and a live verification record where a method can be observed.
Until those plans close, an unlisted method continues to report `blocked` exactly as it does today.
