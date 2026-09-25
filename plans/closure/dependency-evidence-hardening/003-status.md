# Dependency Evidence Hardening M003 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/dependency-evidence-hardening/003-dotnet-jvm-structured-correctness.md`

Source subsystem roadmap:

- `plans/subsystems/dependency-evidence-hardening-roadmap.md`

Repository baseline reviewed: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependency: M001 closed (`bce32e7`).

Implementation commit:

- `0d934f5` M003: .NET/Maven/Gradle structured correctness with quick-xml

## 1. Executive finding

M003 is complete. NuGet lock evidence now reflects the target-framework
resolution graph, and csproj/POM inputs are parsed with a bounded
streaming XML parser instead of line heuristics. Gradle extraction stays
conservative where build-language evaluation would be required. The
`quick-xml` dependency qualified with minimal footprint (one new leaf
crate, no new transitive dependencies).

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| quick-xml qualification | `quick-xml 0.38.4`, `default-features = false`, no optional features | pass | 1 new crate; `memchr` shared; streaming over `&str` |
| NuGet target graphs | `nuget.rs`: TFM/RID context, `resolved` exact, `requested` requirement, relation mapping, project refs, content hashes | pass | Legacy `libraries` shape now `Unsupported` |
| csproj structural | `dotnet.rs`: attribute + child-`Version`, conditions, frameworks, any line layout | pass | Requirements only, never resolved |
| POM structural | `maven.rs`: depth analysis separating management/exclusions/plugins/parent | pass | Scope/optional as context; `${...}` stays requirement |
| Gradle lockfile | `gradle.rs`: exact coords, token validation, dedup | pass | Unchanged semantics, hardened |
| build.gradle conservative | Literal + paren coords, `platform()` wrappers, dynamic/property as unresolved requirements | pass | No Groovy/Kotlin evaluation |
| Footprint account | `cargo tree`, release size before/after, `make bench-check` | pass | See section 4 |

## 3. Production implementation evidence

NuGet two-TFM fixture yields exact findings with `net8.0` and
`net8.0/win-x64` contexts; `CentralTransitive` maps to `Transitive`;
unknown `FutureKind` maps to `Unknown` without rejection; `Project`
entries carry `project reference` provenance with no resolved version.

POM fixture proves `dependencyManagement` (`org.managed:managed`) and
nested exclusion coordinates (`org.hidden:hidden`) never become direct
findings, while scope/optional survive as target context and
`${revision}` stays a requirement.

`build.gradle` `api 'io.projectreactor:reactor-core:${reactorVersion}'`
now yields an explicit unknown-requirement finding instead of a silent
skip (mod.rs expectation updated 4 -> 5).

.NET/JVM split into `dotnet.rs`+`nuget.rs` and `maven.rs`+`gradle.rs`
under the 400-line ratchet; ceilings, ownership, and guard module lists
updated.

## 4. Verification executed

Against candidate `0d934f5`:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked dependency_parse
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo tree --locked
cargo build --locked --release
make bench-check
cargo test --locked --all-features
```

Outcomes:

- `cargo fmt --check`: pass
- `cargo clippy --all-targets --all-features -- -D warnings`: pass
- `dependency_parse` lib filter: 69 passed, 0 failed
- `security_applicability_regression`: 24 passed, 0 failed
- `security_workflow`: 15 passed, 0 failed
- `static_guards`: 44 passed, 0 failed
- `cargo test --locked --all-features`: pass, 0 failures
- `make bench-check`: pass (compile gate)
- `cargo tree`: `quick-xml 0.38.4` -> `memchr` only; sole consumer is eggsearch
- Release binary: 18,777,728 bytes (pre-workstream) -> 19,538,752 bytes
  (+761,024, +4.1% spanning M001-M003 code plus the new crate; rlib
  1.4 MB unlinked). One new leaf crate with zero new transitive
  dependencies is the minimum footprint for the required structural
  parsing; the hand-rolled alternative was rejected by the plan.

## 5. Invariant review

No MSBuild/Groovy/Kotlin/Maven resolution or network lookup. Unknown
interpolation degrades to requirement/unknown. Deterministic ordering
(sorted JSON maps; document order for XML). No new panic paths
(`saturating_sub`, checked pops, lossy conversions).

## 6. Failure and recovery review

Truncated XML yields partial/malformed reports via end-tag checking plus
EOF depth validation, never silent empty output. Legacy `libraries`
NuGet shape is explicit `Unsupported`. Malformed JSON is `Malformed`.

## 7. Migration and compatibility review

Additive fields only. Two intentional behavior refinements: real NuGet
shape replaces the non-conforming `libraries` fixture; dynamic Gradle
versions become visible unknown requirements. No tool/request change.

## 8. Security review

quick-xml pull parsing performs no external entity resolution or
network access; input stays under the 1 MiB file cap with linear
scanning. Project references cannot masquerade as registry packages.

## 9. Documentation and operations

Updated `architecture/security.md` (NuGet/XML/Gradle semantics),
`architecture/build.md` (quick-xml entry), `architecture/maintenance.md`
(split ownership + ceilings).

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| low | Maven version catalogs (`libs.*`) not resolved | Catalog references skipped | Deferred per plan to a future plan |
| low | `platform()`/BOM contents not expanded | BOM edge recorded, contents unknown | By design |

## 11. Roadmap disposition

M003 closed. M004-M006 remain ready; M007 remains blocked on M004-M006
closure.

## 12. Registry updates

Covered in the same closure commit: M003 marked closed.
