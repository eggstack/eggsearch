# M014 Pass-Cookie Lifetime and Egress Binding — Closure Status

Status: conditionally closed

Source plan: `plans/implementation/agentic-retrieval-integrity/014-pass-cookie-lifetime-and-egress-binding.md`

Roadmap: `plans/subsystems/agentic-retrieval-integrity-roadmap.md`

Planning baseline: `dace41c3a75307ea35b745d695a11630ec6495c0`

Implementation candidate and closure commit: the commit that carries this record.

## Result

`src/meta/challenge/cookie.rs` is now the sole owner of pass-cookie lifetime parsing:

- `PublishedLifetime { Unstated, MaxAge(u64), ExpiresAt(SystemTime), Malformed }`, with `kind()`
  exposing only the attribute kind.
- `parse_issued(&HeaderMap, name)` reads the value from the cookie-pair segment only, matches
  attribute names case-insensitively, parses `Expires` as IMF-fixdate with `chrono`, treats a
  present-but-unusable attribute as `Malformed`, and lets `Max-Age` win over `Expires` (RFC 6265
  §5.3).
- `reuse_ttl(lifetime, now, ceiling)` implements the plan's resolution order exactly: a zero
  ceiling stores nothing, `Unstated` falls back to the ceiling, `Malformed` stores nothing,
  `MaxAge(0)` stores nothing, other `Max-Age` values and future `Expires` are clamped by the
  ceiling, and a past/tie `Expires` stores nothing.

`SolveOutcome::Solved` carries the parsed lifetime; both scheme modules deleted their duplicated
`extract_cookie` helpers and parse once. `PassCookieCache::remove(provider)` evicts a rejected
cookie, and both engines call it when a replay is still challenged. `eggsearch doctor` warns when
`[egress]` is routed and cookie reuse is enabled.

## Named condition

The plan's live acceptance requires both measurement arms (derived active versus the fixed-window
reference) and at least one provider whose solve count fell, or an explicit statement that no
provider publishes a lifetime. Startpage publishes a lifetime (~20 min `Expires`), but it is longer
than the shipped 240 s ceiling, so at the default the clamp binds and the derivation cannot reduce
the solve count; at a raised ceiling both the old and new rules re-solve once the challenge is
served, so a solve-count reduction is not expected for this upstream. The observed benefit is the
avoided dead-token request: the derived arm's entry expired on the published schedule and the
second search took a miss and re-solved directly (`cookie_misses=2`, `cookie_hits=0`, 3 rows),
where the fixed 30-minute rule would have presented the expired token first.

The two-arm solve-count comparison was therefore not run. Closing as `closed` would claim an
acceptance the plan does not authorize; the milestone is complete in implementation and verified in
behaviour, and this condition records the missing comparative measurement.

## Published attributes (live capture, 2026-11-13, value redacted)

`spchal-auth` from the Startpage pass-challenge `302`:

```
name=spchal-auth value=<redacted>
attributes= Path=/; Domain=startpage.com; Expires=Fri, 13 Nov 2026 16:52:50 GMT; Secure; SameSite=None
```

No `Max-Age`. `Expires` is approximately 20 minutes after issuance. The challenge response also
sets `sp_pow` with `Max-Age=300` and an `Expires` roughly 20 minutes out; `Max-Age` wins for that
cookie. Mojeek published no lifetime on the captured response, so it falls back to
`cookie_ttl_seconds`.

Resolved window: `min(~1200 s, cookie_ttl_seconds)`. At the shipped default ceiling of 240 s the
clamp binds, so the derivation is a no-op for the shipped configuration; it shortens reuse only
when an operator raises `cookie_ttl_seconds` above the published lifetime.

## Live measurement

One MCP process, `[search.challenge] enabled = true`, `max_difficulty = 6`,
`max_attempts = 50000000`, `timeout_ms = 15000`, `cookie_ttl_seconds = 1800` (above the published
~1200 s), provider `startpage`:

```
t0-search  1.29 s, 3 rows, 0 failed
t0-status  solves_attempted=1, solves_succeeded=1, cookie_misses=1,
           last_candidates=1949962, last_wall_clock_ms=698
```

The second search was issued after the published expiry had passed, to observe whether the derived
window prevents presenting a dead token:

elapsed=1261.4 s
t21-search  6.60 s, 3 rows, 0 failed
t21-status  solves_attempted=2, solves_succeeded=2, cookie_hits=0, cookie_misses=2,
            last_candidates=16831309, last_wall_clock_ms=5908
```

The derived window expired at the published ~20 minutes. Instead of presenting the dead token,
the second search took a cache miss, received a fresh challenge, solved it (16.8M candidates,
5.9 s), and returned 3 rows with no provider failure. A fixed 30-minute window would have sent the
expired cookie and produced a `pass cookie rejected` blocked result first.

The shipped default ceiling (240 s) still binds for this upstream because the published lifetime is
longer; the derivation matters to operators who raise `cookie_ttl_seconds` above the published
lifetime, and it is a no-op at the default.

## Verification

```bash
cargo fmt --all -- --check                    # clean
cargo clippy --locked --all-targets --all-features -- -D warnings   # clean
cargo check --locked --no-default-features    # clean
cargo test --locked --all-features --lib meta::challenge            # 82 passed
cargo test --locked --all-features --lib meta::engines::startpage   # 47 passed
cargo test --locked --all-features --lib meta::engines::mojeek      # 20 passed
cargo test --locked --all-features --bin eggsearch commands::doctor # 7 passed
cargo test --locked --all-features                                 # 6,291 passed / 28 ignored
```

Cookie unit tests cover every row of the plan's table, attribute-name case, the
cookie-pair-versus-attribute value rule, `Max-Age` precedence over a past `Expires`, malformed
values, and attribute-kind-only rendering. Engine flow tests cover `Max-Age=0` leaving no entry,
a lifetime beyond the ceiling being clamped, zero TTL solving every time, and rejection eviction.

## Deviations recorded

- `SolveOutcome::Solved` gained the lifetime field as planned; no other public shape changed.
- The doctor warning is a plain warning string, with no new `WarningCode`, payload field, or
  validation failure, as the plan required.
- The egress decision is "keep using the cached cookie, evict on rejection, document and warn"; no
  address detection and no route-dependent TTL shortening.

## Residual risk

- A system-clock step can extend a stored `Expires`-derived window by at most `cookie_ttl_seconds`;
  it can never be indefinite.
- An egress exit that varies per connection can still cause a re-challenge; the failing search
  reports `blocked` and the next starts cold.
- Attribute drift (a provider changing or dropping `Max-Age`/`Expires`) falls back to the ceiling or
  to no reuse only when the attribute is present but unusable; a dropped attribute is `Unstated`.
- Mojeek published no lifetime on the captured response, so its reuse is unchanged in practice.
