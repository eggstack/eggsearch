# Dependency Evidence Hardening M008 — Corrective Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/dependency-evidence-hardening/009-corrective-applicability-provenance-and-parse-status.md`

Source subsystem roadmap:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

Corrective baseline: `a10f23ba92be05d4b42ba1f373c895895cbf88aa`.

Implementation commit:

- `3fd3076c4ad9680490b35a96759cab3f5022e6bc` — gate dependency advisories on provenance

Historical M001-M007 evidence remains in `plans/closure/dependency-evidence-hardening/001-status.md` through `007-status.md`. M008 records a later corrective pass; it does not alter those records.

## 1. Corrective findings and outcomes

| Post-M007 defect | Corrective evidence | Outcome |
|---|---|---|
| Advisory matching ignored source provenance | `advisory_provenance_compatibility()` classifies evidence as Compatible, Incompatible, or Unverified. Non-compatible exact versions remain findings, but produce Unknown assessments without matched/fixed ranges and a stable structured warning. | Pass |
| Assessment dedup collapsed equal package/version findings with different applicability bases | The assessment key includes a deterministic fingerprint of provenance, target context, reference kind/value, source file, and source kind, plus version and advisory identity. | Pass |
| Package matching globally case-folded non-PyPI ecosystems | `canonical_package_name()`, `packages_match()`, and finding index keys share the same explicit ecosystem rules. Display names are unchanged. | Pass |
| Recognized structured parser syntax failures became Complete with no findings | Dispatcher validates structured syntax and supported shape before compatibility Vec parsers can label an empty result Complete. Valid empty inputs remain Complete; unrecognized valid shapes are Unsupported. | Pass |

## 2. Provenance compatibility policy

| Ecosystem/source evidence | Decision |
|---|---|
| Cargo with explicit crates.io index source | Compatible |
| Cargo with absent source identity | Unverified |
| Cargo path, workspace, or git source | Incompatible |
| Go `vendor/modules.txt`, without replacement | Compatible |
| Go manifest evidence without vendored resolution, or missing source proof | Unverified |
| Go replacement, local path, or other typed local/VCS/URL reference | Incompatible |
| npm default-registry lock entry with no explicit source, or an explicit `registry.npmjs.org` host | Compatible |
| npm custom registry URL or git/file/workspace source | Incompatible |
| PyPI default lock entry or explicit `pypi.org` source | Compatible |
| PyPI alternate/unrecognized source | Unverified; typed path/git/URL sources are Incompatible |
| RubyGems lock source containing `rubygems.org` | Compatible |
| RubyGems missing or unrecognized source, and Maven, NuGet, Packagist, OCI, GitHub Actions | Unverified |
| Any finding without an exact resolved version | Unverified |

This gate makes no network or package-manager calls. Incompatible and unverified evidence stays visible in `dependency_findings`; affectedness and fixed-version claims are withheld as Unknown, and `dependency_provenance_unverified_for_advisory` is exposed through the structured warning boundary.

## 3. Ecosystem identity matrix

| Ecosystem | Comparison rule | Rationale |
|---|---|---|
| PyPI | Lowercase; collapse runs of `-`, `_`, and `.` to `-` | [Python name normalization](https://packaging.python.org/en/latest/specifications/name-normalization/) |
| NuGet | Case-insensitive | [NuGet package ID matching](https://learn.microsoft.com/en-us/nuget/consume-packages/finding-and-choosing-packages) |
| npm | Exact text | [npm package names must be lowercase](https://docs.npmjs.com/package-name-guidelines/) |
| crates.io | Exact text | [Cargo registry package-name fields are case-sensitive](https://doc.rust-lang.org/cargo/reference/registry-index.html) |
| Go | Exact module path | [Go module identity is its declared module path](https://go.dev/ref/mod); no case folding is applied |
| RubyGems, Packagist, Maven, GitHub Actions, OCI | Exact text | No shared comparison normalizer is established by the checked-in evidence, so exact comparison is the conservative rule |

Positive and negative matrix cases cover all ten ecosystems, including Go case distinction, PyPI punctuation/case normalization, NuGet case-insensitivity, and npm/cargo case-distinct inputs. The response preserves original package spelling.

## 4. Malformed-versus-empty parser evidence

| Input | Malformed payload | Valid empty payload |
|---|---|---|
| Cargo manifest and lock | Malformed | Complete |
| Composer lock | Malformed | Complete |
| Poetry and uv locks | Malformed | Complete |
| Pipfile lock | Malformed | Complete |
| npm package lock and shrinkwrap | Malformed | Complete |
| NuGet `packages.lock.json` | Malformed | Complete |
| Maven `pom.xml` and .NET `.csproj` | Malformed XML | Complete |

Table-driven fixtures also assert that syntactically valid unsupported Cargo, Pipfile, and Composer shapes report Unsupported. Property coverage mutates malformed recognized TOML, JSON, and XML payloads and asserts they do not become Complete-empty. Existing file, aggregate, ordering, and truncation bounds remain unchanged.

## 5. Verification

On implementation commit `3fd3076c4ad9680490b35a96759cab3f5022e6bc`:

| Gate | Result |
|---|---|
| `cargo fmt --check` | Pass (`make check`) |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass (`make check`) |
| `cargo test --locked --features mock --test security_applicability_contract --test security_applicability_regression --test security_applicability_corpus --test security_workflow --test static_guards` | 169 passed |
| `cargo test --locked --all-features --test property_dependency_parse --test dependency_fixtures` | 36 passed |
| `cargo test --locked --all-features` | Pass as part of `make check`; 3,293 library tests plus all integration suites and 4 doctests |
| `make check` | Pass; includes feature checks, repo hygiene, and packaging contract |
| `make bench-check` | Pass; Criterion harness compiled with `--no-run` |
| `RUSTUP_TOOLCHAIN=nightly make fuzz-smoke` | Pass; all four bounded targets completed without a crash |
| `RUSTUP_TOOLCHAIN=nightly cargo fuzz run dependency_parse -- -max_total_time=60` | Pass; 615,889 executions, no crash |

The smoke campaign's final dependency parser run completed 660,578 executions without a crash. The standalone run above was made against the committed implementation. Fuzz tooling rewrote its generated `fuzz/Cargo.lock`; that generated lockfile change was restored and is not part of the implementation.

No production dependency changed; no dependency-tree or release-binary-size delta applies. The only new runtime behavior is conservative withholding of exact applicability when registry identity is not established.

## 6. Residual findings and recommendation

| Severity | Residual |
|---|---|
| High/Medium | None identified |
| Low, accepted limitation | Maven, NuGet, Packagist, OCI, GitHub Actions, and other unverified source cases remain Unknown because checked-in evidence does not establish public-registry artifact identity. |
| Low, accepted limitation | The ecosystem matrix uses exact text where a shared authoritative normalization contract is not established; this can miss case-variant advisory matches but cannot merge distinct identities. |

Recommendation: **closed**. M008 acceptance criteria are met. M009 remains operationally blocked until the exact M008 closure candidate has successful hosted CI evidence.
