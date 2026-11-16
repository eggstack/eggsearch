# ADR-0012: Non-Interactive Challenge Extensions

Status: accepted
Date: 2026-11-13
Decision owners: project maintainers
Affected subsystems: search capability; keyless HTML engines; hardening and resource bounds; provider diagnostics.
Related: `plans/adrs/ADR-0009-bounded-challenge-solving.md`; `plans/adrs/ADR-0010-challenge-solver-coverage-and-cost-policy.md`; `architecture/engines.md` § Challenge solving; `plans/archive/implementation/agentic-retrieval-integrity/026-anubis-extension-fail-closed.md`; `plans/archive/implementation/agentic-retrieval-integrity/027-anubis-css-load-extension.md`.

## Context and forces

Anubis v1.28 added **challenge extensions**. A challenge document now carries an
`extensions` list; every named extension must validate after the method validator and
before a clearance JWT is issued. The open-source registry ships one extension,
`css-load`. It renders a same-origin
`<style>@import url("/.within.website/x/cmd/anubis/ext/css-load/<challenge-id>")</style>`
in the challenge page and records a hit when that URL is fetched. The proprietary
`headless`/BotStopper extension is a different class: it requires JavaScript and
WebAssembly execution and sends a detection report.

ADR-0010's permitted class covers challenge **methods** (`fast`, `slow`, `preact`,
`metarefresh`) and ALTCHA. It is silent on extensions, and it says any additional
method requires an evaluation against ADR-0009's prohibition list. Extensions are a
separate axis and must not inherit permission from the method list.

Today the solver ignores `challenge.extensions`. A `css-load` challenge is parsed,
solved, submitted, and then rejected by the extension validator. The operator sees
`pass cookie rejected`, which attributes the failure to the proof; the actual cause is
an unperformed same-origin fetch. That is a classification defect even if no
extension is ever supported.

The forces are: (a) keep the permitted class limited to machine-verifiable,
non-interactive acts; (b) do not accept an unbounded instruction channel from a page
to the client; (c) keep the operator opt-in and every cost bound; (d) classify an
extension failure honestly.

## Considered alternatives

1. **Refuse every non-empty `extensions` list.** Safe, and it is the default until
   this ADR is accepted. Rejected as the permanent answer: `css-load` is a
   server-published same-origin liveness check, not a human gate, and refusing it
   forfeits deployments that layer it on an otherwise solvable challenge.
2. **Permit any extension the page lists.** Rejected. Extension names are
   page-controlled; permitting the list would let an upstream require JavaScript,
   WebAssembly, visual verification, or a proprietary detector, and the client would
   have no bounded interpretation of the name.
3. **Permit a named, evaluated set.** Selected. The set starts with `css-load` only;
   every future extension needs its own evaluation.
4. **Use the browser path for extensions.** Rejected. It adds a Chrome process and
   profile for one same-origin GET, and it does not help with the proprietary
   `headless` detector.

## Decision

- Permit a narrowly defined class: **non-interactive, server-published, same-origin
  resource-fetch extensions**. The client's only action is an ordinary bounded HTTP
  `GET` to a URL derived from the challenge's published base prefix, a fixed
  extension path, and the parsed challenge ID. The response grants no authority
  beyond proving that the URL was fetched.
- The only member of the class today is Anubis `css-load` (v1.28+).
- Conditions for every member:
  - only when `[search.challenge].enabled` is true;
  - same origin as the challenge, with the base prefix taken from the validated
    `#anubis_base_prefix` value;
  - the URL is constructed from the fixed extension path and the validated challenge
    ID, never copied from arbitrary page text;
  - at most one request per extension per solve, inside the remaining solve deadline;
  - no cookies, authorization headers, request body, or operator-supplied value is
    forwarded;
  - redirects are disabled and the response body is bounded and discarded (cap at
    4 KiB);
  - no JavaScript, WebAssembly, OCR, image, or interactive step; no new dependency.
- **Excluded:** `headless`/BotStopper, any interactive or visual extension, any
  extension that requires executing code, and every unknown name. A non-empty
  extension list containing an excluded or unknown name fails closed before the proof
  is submitted, with a sentence naming the extension list and the existing
  `unsupported scheme` refusal key.
- Adding another extension requires evaluating it against ADR-0009's prohibition list
  and recording the outcome in a new ADR or a superseding record. It does not inherit
  permission from this list.

## Consequences and tradeoffs

Advantages: deployments that layer a cheap liveness check stay reachable; the cost is
one same-origin GET; the failure classification becomes truthful; the client still
performs no interactive or visual work.

Tradeoffs: the permitted class is no longer strictly proof-of-work or time challenge.
Each extension adds a review obligation, and a future extension could become a
fingerprinting channel. The mitigation is the narrow class plus the no-cookie and
no-header rules: the extension learns only that the challenge ID was fetched, which
the proof submission already reveals.

## Compatibility, security, and migration

No MCP tool, response-schema, or configuration-key change. The ten-key refusal
vocabulary is unchanged; an extension refusal is recorded under `unsupported scheme`
with a specific sentence. No migration. `src/fetch/**` stays independent of
`src/meta/**`. Every ADR-0009 and ADR-0010 prohibition stands unchanged, including the
ban on user-agent spoofing, proxy rotation, interactive solving, and retry widening.

## Verification and status

Adoption authorizes the class; it does not change behaviour. Implementation lands
through M027 with a synthetic `css-load` fixture, a not-found and unknown-extension
refusal test that performs zero submissions, a disabled-path test that performs zero
extension requests, and a live record if a deployment publishing `css-load` is
reachable. Until M027 closes, M026's fail-closed refusal remains the behaviour.
