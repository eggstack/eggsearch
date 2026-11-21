# ADR-0019: Broadened Challenge Coverage, Browser Completion, and Browser-Like Identity

Status: accepted
Date: 2026-11-19
Decision owners: project maintainers
Affected subsystems: search capability; keyless HTML engines; fetch path; browser render path;
hardening and cost policy; provider diagnostics; transport boundary.
Affected long-term sections: none require amendment. The widened class stays inside ADR-0006's
keyless retrieval posture; this record changes which client acts are permitted, not what the
product promises.
Related: `plans/adrs/ADR-0009-bounded-challenge-solving.md` (superseded in part);
`ADR-0010` (superseded in part); `ADR-0013` (superseded in part); `ADR-0002` (transport
ownership); `ADR-0006` (keyless deterministic retrieval); `ADR-0014` (browser artifacts and
render bounds); `ADR-0017` and `ADR-0018` (fetch-path resolution and recovery);
`architecture/engines.md` § Challenge solving.

## Context and forces

The permitted class has been a name list since ADR-0009, widened twice. ADR-0010 replaced
hardcoded names with the published Anubis registry; ADR-0013 added WASM `sha256` and deferred
`argon2id` and `hashx` as "separate cost and dependency questions" that "must not be smuggled in
with `sha256`". Both widenings were reactive: an upstream shipped a puzzle, and the solver
reported `unsupported scheme` until a record caught up. That lag now repeats faster than a record
can be written, and the lag itself is a classification defect — it attributes an upstream change
to the proof.

Four observations make the reactive posture untenable.

**Anubis moved past the method we cover.** v1.28.0 added WASM `argon2id` and `hashx`, both
disabled by default, with upstream stating it will enable them "in a future version after they
have been sufficiently tested". The method already implemented, `sha256`, is the one Anubis
documents as having "known tools that solve this particular challenge" and recommends against.
A deployment that follows that advice becomes unreachable while the solver is at its most
conformant.

**ALTCHA's digest set is wider than one algorithm.** It now publishes `PBKDF2/SHA-256`,
`PBKDF2/SHA-384`, `PBKDF2/SHA-512`, plain `SHA-256`, `SHA-384`, `SHA-512`, `ARGON2ID`, and
`SCRYPT`. The solver accepts exactly one normalised algorithm, `PBKDF2/SHA-256`
(`src/meta/challenge/altcha.rs`). A Mojeek switch to any other digest is a silent provider loss
classified as an unsupported scheme rather than as drift.

**Memory-hard puzzles break the cost model, not just the name list.** Anubis `argon2id`, ALTCHA
`ARGON2ID`/`SCRYPT`, and Cap.js's default HashWX all price a solve in memory or in ASIC
resistance rather than in hash count. ADR-0010's ceilings — difficulty, attempts, wall clock —
express a hash search. An attempts-only bound lets a memory-hard challenge spend unbounded
resident memory to stay under it. The upstream equivalence table makes the gap concrete:
`fast` 4 ≈ `sha256` 16 ≈ `hashx` 15 ≈ `argon2id` 4, because one Argon2 evaluation is worth
roughly four thousand SHA-256 candidates.

**Many walls are not puzzles.** An agent meets (i) an interstitial script the page expects a
browser to run — AWS WAF Challenge answers `202` with `x-amzn-waf-action: challenge` and an
`aws-waf-token` (ii) a client challenge attached on page load, such as Cap.js's instrumentation
layer or Vercel BotID's `initBotId` (iii) a policy decision: `403` `DENY` from an Anubis rule,
or `402 Payment Required` from Cloudflare AI Crawl Control. No puzzle-name list reaches (i) or
(ii), and (iii) must not be reached at all.

The product goal is retrieval. A page an origin is willing to serve to a real browser should be
retrievable by an agent that presents an honest, ordinary client identity and does the work the
origin asks for. The forces are: (a) serve that goal instead of auditing upstream name lists;
(b) keep every cost bounded and visible now that the capability is not opt-in; (c) never become a
credential, authentication, or payment bypass; (d) never present a *third party's* identity;
(e) keep every widening attributable in diagnostics.

## Considered alternatives

1. **Keep the name list and add names reactively.** Rejected. It has already produced three
   false `unsupported scheme` refusals (`slow`, WASM `sha256`, and the current `argon2id`/
   `hashx` pair) and guarantees a fourth each time an upstream ships a method.
2. **Permit any challenge the page names.** Rejected for ADR-0010's reason: the name is
   page-controlled, so an unknown label becomes an unbounded instruction channel.
3. **Permit a property-based class with an evaluated member set.** Selected.
4. **Permit all widening axes, including third-party solvers and interactive CAPTCHA.** Rejected
   by the decision owner. Outsourcing a solve to a service or a human stays out of scope, and
   interactive or OCR work is incompatible with the bounded-I/O and no-image posture.
5. **Route every wall through the browser.** Rejected as the only mechanism. It is the right
   mechanism for script-driven walls, but a Chrome process is the wrong price for a one-second
   hash search.
6. **Hand-roll TLS/JA4 impersonation in this crate.** Rejected as the primary identity mechanism.
   Production source may not use `reqwest` (guard-enforced) and the TLS stack belongs to
   `eggfetch-core` under ADR-0002, so this is a transport-owner dependency decision, not a solver
   detail. The real browser already presents an authentic browser TLS fingerprint for free.

## Decision

### 1. The permitted class is defined by properties, not by names

An exchange is in scope when all of these hold:

- the server publishes the parameters needed to answer it;
- the answer is deterministic from those parameters, with no human judgement, image, OCR, or
  semantic task;
- the client's only acts are local computation, ordinary same-origin requests the server itself
  directs, and — where the wall requires code execution — running the page's own challenge script
  inside the shipped browser under the existing render caps;
- every cost is expressible under an operator-visible ceiling;
- the identity presented is the operator's configured profile, consistent for the whole exchange.

### 2. The evaluated member set

| Family | Members | Mechanism |
|---|---|---|
| Anubis | `fast`, `slow`, `preact`, `metarefresh`, `sha256`, `argon2id`, `hashx`; extension `css-load` | local computation; one bounded same-origin GET for the extension |
| ALTCHA | `PBKDF2/SHA-256`, `PBKDF2/SHA-384`, `PBKDF2/SHA-512`, `SHA-256`, `SHA-384`, `SHA-512`, `ARGON2ID`, `SCRYPT` | local computation |
| Script-completed interstitials | any wall whose challenge is completed by a page script, including AWS WAF Challenge (`202`, `x-amzn-waf-action: challenge`) | shipped browser, existing render caps and session limits |

Membership is evaluated, not open by label. A name outside this set still fails closed as
`unsupported scheme` with a sentence naming it, before any submission.

### 3. Cost policy gains a memory ceiling

A per-solve memory ceiling is added beside difficulty, attempts, and wall clock. It is the
binding bound for `argon2id`, `SCRYPT`, and any future memory-hard member; the attempt ceiling
alone cannot express them. `max_difficulty` keeps its per-family meaning and the documentation
must state that units differ: Anubis WASM members count leading zero *bits*, `fast` counts hex
*nibbles*. ADR-0010's single-threaded, single-process nonce search stands unchanged, as does the
cost-asymmetry constraint and the determinism rule.

### 4. Solving is on by default, bounded

`[search.challenge].enabled` defaults to true. Shipped ceilings stay conservative, a per-origin
solve rate applies, and every solve remains inside the call deadline. `enabled = false` restores
the previous behaviour exactly. The operator still accepts the CPU, memory, and upstream terms
that enabling implies; the diagnostics must make the cost visible rather than implied.

### 5. Browser-like identity is permitted; a third party's identity is not

The client may present a browser-like `User-Agent` with the matching request-header set, held
consistent for the whole exchange, because challenge metadata is bound to identity: Anubis records
`User-Agent`, `X-Real-IP`, and since v1.28 `Referer`, and rejects a pass whose issuing policy no
longer matches. It may not present itself as a *named third party* such as Googlebot, fabricate a
forward-confirmed reverse-DNS claim, or reuse another client's session. Presenting our own client
as browser-like is an identity choice; claiming to be someone else is impersonation, and Anubis
has already shipped a fix (GHSA-5gwm-367w-7fgj) for exactly that bypass class.

### 6. Retained prohibitions

These are not reopened and are not authorized by any member set above:

- third-party or human solving services, including ALTCHA Sentinel's keyed challenge endpoint and
  Friendly Captcha's `/api/v2/captcha/siteverify`;
- interactive CAPTCHA, visual puzzle answering, and OCR of challenge images;
- credential, login, or session bypass and authentication walls;
- **payment circumvention**: a `402 Payment Required` from pay-per-crawl or AI Crawl Control is a
  price, not a puzzle. It is reported, never answered;
- cookie theft. Replaying pass cookies this client itself was issued remains in scope
  (M033/M034); taking another client's does not;
- proxy rotation, and retry widening to conceal an outage;
- tarpit traversal: an endless generated maze is abandoned, never followed;
- executing a served `.wasm` module inside our own address space. ADR-0013's rejection stands:
  named algorithms are reimplemented. A browser running a page's script is a different act, in a
  sandbox built for that purpose, not remote code in this process.

## Consequences and tradeoffs

Advantages: coverage stops lagging upstream; a page an origin serves to a browser becomes
retrievable; the cost policy can express memory-hard puzzles; identity rules are stated once
instead of inferred from a prohibition list; and the failure vocabulary keeps telling an operator
whether the wall was a puzzle, a script, or a refusal.

Costs, stated plainly: default-on spends CPU, memory, and browser processes for every operator,
so the shipped ceilings must be conservative and the ledger honest. Browser completion adds a
failure mode and a large per-wall price. A browser-like identity is an arms race with
fingerprinting, may violate individual origins' terms, and is recorded as operator-owned rather
than hidden. The memory ceiling adds a configuration surface and a new out-of-memory class that
must fail closed rather than abort. None of this improves a *policy* refusal: `403` DENY and
`402` remain reachable only by the operator, not by the client.

## Compatibility, security, and migration

No MCP tool, advertised schema, or response-shape change. Two configuration changes: a new memory
ceiling key, and `[search.challenge].enabled` moving from default-false to default-true. The
default flip is a behaviour change for every existing operator and must be stated in
`CHANGELOG.md`, the configuration reference, and `docs/features.md` in the implementing change.

The ten-key refusal vocabulary is unchanged; `unsupported scheme` keeps its meaning for names
outside the evaluated set. Cookie, nonce, salt, key-prefix, challenge-id, and pass-token values
never appear in logs, errors, `Debug` output, fixtures, or `provider_status`. `src/fetch/**`
stays independent of `src/meta/**`; shared orchestration stays in `src/challenge_replay/`; browser
use stays behind the existing optional `browser` feature with ADR-0014's render and session caps.

## Superseding record

Superseded clauses, cited exactly:

- **ADR-0009**: the permitted-class bullet (now §1/§2 here); "Solving is **opt-in and off by
  default**" (now §4); "user-agent spoofing" in the prohibition list, only to the extent of §5;
  and the dependency-budget sentence, only to permit the *already shipped, already optional*
  browser feature to complete a script-driven challenge. No new browser, TLS, network, or service
  dependency is added by this record.
- **ADR-0010**: the coverage bullet's fixed registry list (now §2); and
  "`max_difficulty` remains the operator's explicit acceptance of cost", refined by the memory
  ceiling in §3. The single-threaded policy, the cost-asymmetry constraint, determinism,
  per-provider caching, and `blocked`-never-empty classification all stand.
- **ADR-0013**: the sentences deferring `argon2id` and `hashx` to "their own evaluations". This
  record is that evaluation. The rejection of in-process execution of a served `.wasm` module
  stands.

Every prohibition listed in §6 continues to stand, including the ADR-0009 items this record does
not touch.

## Verification and status

Adoption authorizes the class; it does not change behaviour. Implementation lands through
registered milestones, one per widening axis — puzzle classes, browser completion, and identity —
because they have different cost, dependency, and test surfaces and must not ship as one
change. Each needs: a synthetic fixture per member, a memory-ceiling exhaustion test, a
fail-closed test for a name outside the set, a default-on cost test, a test that a third-party
identity is never presented, and a live record where a wall is observable. Until those milestones
close, today's behaviour stands — opt-in false, the current name list, no browser challenge
completion — and `architecture/engines.md` § Challenge solving continues to describe it.
