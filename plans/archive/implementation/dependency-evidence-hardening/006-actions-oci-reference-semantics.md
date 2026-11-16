# Plan 006 — GitHub Actions and OCI Reference Semantics

Status: implementation plan

Source roadmap:
`plans/subsystems/dependency-evidence-hardening-roadmap.md`

Milestone: M006

Primary class: capability

Baseline for planning: `e0dab301cc04bf5e1e232177e2e2647262a1fdd7`

Hard dependency: M001 closed.

## Objective

Represent GitHub Actions and OCI dependencies as typed references rather than
pretending every ref/tag/digest is an exact semantic package version. Correct
Docker `FROM` option/stage parsing and distinguish remote, local, and Docker
actions.

## Current implementation evidence

- Actions parsing scans trimmed lines for `uses:`, splits at the final `@`,
  and stores the ref in `DependencyFinding.version` with High confidence.
- It cannot distinguish commit SHA, tag, branch, expression, reusable workflow,
  local action, same-repository action, or Docker action.
- Docker parsing treats the token immediately after `FROM ` as the image, so
  `FROM --platform=...` is misparsed.
- OCI digest syntax and variable tags are split with a simple final colon.
- Container findings are labeled `DependencySource::LockFile` despite an
  existing Dockerfile source kind.
- Stage aliases can be mistaken for external images.

## Upstream format anchors

- GitHub Actions workflow `uses` syntax:
  https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
- GitHub action ref stability guidance:
  https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax
- Docker `FROM` grammar:
  https://docs.docker.com/reference/dockerfile

## Required production changes

### 1. GitHub Actions reference classification

Recognize at least:

- `owner/repo@ref`;
- `owner/repo/path@ref`;
- reusable workflow remote refs;
- local `./path` actions/workflows;
- same-repository `$/path` refs supported by current GitHub Actions;
- `docker://image[:tag][@digest]`;
- expressions/unknown dynamic values.

Classify remote refs as full commit SHA, tag-like, branch/other mutable ref, or
unknown expression where determinable. A full SHA is immutable source
reference evidence, not a package semantic version. Tag/branch refs remain
references and do not independently establish an exact resolved commit.

Local/same-repository actions should not be emitted as external GitHub package
dependencies. They may be represented as local/workspace findings if useful
for evidence completeness.

Docker actions feed OCI evidence, not GitHubActions package evidence.

### 2. Workflow YAML handling

Preserve the M005 YAML dependency decision. If a shared YAML parser was
qualified there, reuse it. Otherwise use one bounded workflow-specific
extraction seam that handles quoted scalars and comments without executing
expressions.

Do not evaluate GitHub expression language.

### 3. Dockerfile parsing

Parse `FROM [--platform=...] image [AS name]` correctly.

Track prior stage aliases so `FROM builder AS final` is recognized as an
internal stage rather than an external image.

Classify:

- image with immutable digest;
- image with tag;
- image with both tag and digest;
- untagged/default-latest image;
- ARG/interpolated image reference;
- `scratch`;
- internal stage alias.

Use `DependencySource::Dockerfile`.

A tag is a mutable source reference unless the evidence model explicitly
supports tag-as-requirement semantics. A digest is immutable provenance but
not a semantic package version.

### 4. Compose image references

Handle `image:` values consistently for compose/docker-compose recognized
files, including registry ports, tag+digest, quotes, and interpolation. Do not
split `registry.example:5000/name:tag` incorrectly.

### 5. File routing

Recognize conventional Dockerfile variants and compose filenames only with
bounded, explicit rules. Do not route arbitrary YAML to the container parser
merely because a path substring contains `docker-compose`.

## Required tests

GitHub Actions:

- full SHA;
- major and exact tags;
- branch;
- owner/repo/path;
- reusable workflow;
- local `./`;
- same-repository `$/`;
- Docker action;
- expression/dynamic ref;
- quoted/commented `uses`.

OCI/Docker:

- ordinary tag;
- registry port;
- digest;
- tag+digest;
- `--platform`;
- ARG interpolation;
- `scratch`;
- multiple stages and stage alias reuse;
- compose quoted image;
- compose variable reference.

Regression requirements:

- `FROM --platform=$BUILDPLATFORM rust:1.89 AS builder` reports `rust`, not
  `--platform=...`.
- `ubuntu@sha256:...` does not report `ubuntu@sha256` as the package and the
  digest tail as a semantic version.
- `uses: actions/checkout@main` cannot become exact resolved-version
  applicability evidence.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked dependency_parse
cargo test --locked --features mock --test security_applicability_regression
cargo test --locked --features mock --test security_workflow
cargo test --locked --all-features
```

## Documentation updates

Update `architecture/security.md` with action/OCI ref semantics and
`architecture/hardening.md` if workflow YAML parsing adds parser-specific
resource limits.

## Acceptance criteria

- Action refs are typed as references with immutability semantics.
- Local and Docker actions are not misidentified as remote GitHub packages.
- Docker `FROM` options, stage aliases, tags, digests, ports, and variables
  are parsed without false exact versions.
- Container findings use the correct source kind.
- No expression/build-arg evaluation or external lookup is introduced.

## Stop conditions

Do not resolve Git refs, tags, images, expressions, or Docker ARG values over
the network. Unknown/dynamic references remain explicit unknown evidence.
