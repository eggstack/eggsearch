#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
targets="${EGRESS_CONTRACT_TARGETS:-$root/packaging/release-targets.txt}"
workflow="${EGRESS_CONTRACT_WORKFLOW:-$root/.github/workflows/egress-feature-qualify.yml}"
python3 - "$targets" "$workflow" "$root" <<'PY'
import re
import sys
targets_path, workflow_path, root = sys.argv[1:]
def read(path):
    with open(path, encoding="utf-8") as handle:
        return handle.read()
rows = []
with open(targets_path, encoding="utf-8") as handle:
    for raw in handle:
        line = raw.strip()
        if not line:
            continue
        fields = line.split("|")
        if len(fields) != 4 or any(not field for field in fields):
            raise SystemExit(f"invalid release target row: {line}")
        rows.append(fields[0])
release_set = sorted(set(rows))
if len(rows) != 7 or len(release_set) != 7:
    raise SystemExit("release target contract must contain seven unique targets")
workflow = read(workflow_path)
singular = re.findall(r"^\s*(?:-\s*)?target:\s*([^\s#\"']+)", workflow, re.MULTILINE)
plural_raw = re.findall(r"^\s*(?:-\s*)?targets:\s*([^\n#]+)", workflow, re.MULTILINE)
dash_target = re.findall(r"--target\s+([^\s\"']+)", workflow)
candidates = []
candidates.extend(singular)
for chunk in plural_raw:
    for token in re.split(r"[\s,]+", chunk):
        token = token.strip().strip("\"'")
        if token:
            candidates.append(token)
candidates.extend(dash_target)
def looks_like_target(token):
    token = token.strip().strip("\"'")
    if "${{" in token or "$" in token:
        return False
    if token.count("-") < 2:
        return False
    return ("unknown" in token) or ("apple" in token) or ("pc-windows" in token)
matrix_set = sorted({t.strip().strip("\"'") for t in candidates if looks_like_target(t)})
missing = [t for t in release_set if t not in matrix_set]
extra = [t for t in matrix_set if t not in release_set]
if missing or extra:
    if missing:
        print("egress qualification matrix is missing maintained targets:", file=sys.stderr)
        for t in missing:
            print(f"  missing: {t}", file=sys.stderr)
    if extra:
        print("egress qualification matrix has extra targets:", file=sys.stderr)
        for t in extra:
            print(f"  extra: {t}", file=sys.stderr)
    print(f"release-targets.txt: {release_set}", file=sys.stderr)
    print(f"egress workflow matrix: {matrix_set}", file=sys.stderr)
    raise SystemExit("egress qualification target matrix must exactly equal packaging/release-targets.txt")
if "cargo check --locked --features egress" not in workflow:
    raise SystemExit("egress workflow must run cargo check --locked --features egress")
for target in release_set:
    if target not in workflow:
        raise SystemExit(f"egress workflow does not reference maintained target: {target}")
for forbidden in ["upload-artifact", "upload-release", "cargo publish", "softprops/", "svenstaro/", "gh release"]:
    if forbidden in workflow:
        raise SystemExit(f"egress workflow must remain non-publishing; found forbidden marker: {forbidden}")
if "msrv-all-features" not in workflow:
    raise SystemExit("egress workflow must retain the MSRV all-features job")
if "cargo +1.89.0 check --locked --all-features" not in workflow:
    raise SystemExit("egress workflow must run cargo +1.89.0 check --locked --all-features")
required_paths = [
    "src/fetch/egress.rs",
    "src/core/config.rs",
    "src/meta/engines/mod.rs",
    "src/meta/adapter/builders.rs",
    "src/meta/adapter/mod.rs",
    "src/mcp/state.rs",
    "tests/egress_routing.rs",
    "tests/static_guards.rs",
    "tests/egress_qualify_contract.rs",
    "Cargo.toml",
    "Cargo.lock",
    "packaging/release-targets.txt",
    "packaging/check-egress-qualify-contract.sh",
    ".github/workflows/egress-feature-qualify.yml",
]
uncovered = [p for p in required_paths if p not in workflow]
if uncovered:
    for p in uncovered:
        print(f"  uncovered trigger path: {p}", file=sys.stderr)
    raise SystemExit("egress workflow path filters do not cover the provider-route construction seam")
symbol_owner = [
    ("build_http_client_with_egress", "src/meta/engines/mod.rs"),
    ("build_default_engines_with_egress", "src/meta/adapter/builders.rs"),
    ("new_with_egress", "src/meta/adapter/mod.rs"),
    ("config.egress", "src/mcp/state.rs"),
    ("EgressSection", "src/core/config.rs"),
    ("apply_route", "src/fetch/egress.rs"),
]
for symbol, rel in symbol_owner:
    with open(f"{root}/{rel}", encoding="utf-8") as handle:
        source = handle.read()
    if symbol not in source:
        raise SystemExit(f"expected route-construction symbol {symbol!r} in {rel}")
    if rel not in workflow:
        raise SystemExit(f"route-construction owner {rel} is not covered by the egress workflow path filters")
print("egress qualification contract agrees with packaging/release-targets.txt")
print(f"targets: {', '.join(release_set)}")
PY
