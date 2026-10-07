#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ] || ! command -v shopt >/dev/null 2>&1; then
    echo "packaging contract checks require bash" >&2
    exit 2
fi

set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
targets="$root/packaging/release-targets.txt"
contract="$root/release/eggpack/distribution.toml"
workflow="$root/.github/workflows/release-eggpack.yml"
provenance_workflow="$root/.github/workflows/release-provenance.yml"
unix_installer="$root/packaging/install.sh"
windows_installer="$root/packaging/install.ps1"
rust_contract="$root/src/platform.rs"
docs_installation="$root/docs/installation.md"

python3 "$root/packaging/check-workflow-pins.py"
python3 "$root/packaging/check-planning-consistency.py"
"$root/packaging/release-validate.sh" tree "$(git -C "$root" rev-parse HEAD)"

# Producer authority is `release/eggpack/` plus the Eggpack-generated workflow.
# `packaging/release-targets.txt` is a frozen *consumer* compatibility mirror
# that runtime code, installers, and docs depend on independently; it is no
# longer a producer input. This check compares the mirror against the contract
# rather than treating both as coequal authorities, so a rename cannot slip
# through by editing only one of them.
python3 - "$targets" "$contract" "$workflow" "$provenance_workflow" "$unix_installer" "$windows_installer" "$rust_contract" "$docs_installation" <<'PY'
import re
import os
import sys
import tomllib

(
    targets_path,
    contract_path,
    workflow_path,
    provenance_path,
    unix_path,
    windows_path,
    rust_path,
    docs_path,
) = sys.argv[1:]

rows = []
with open(targets_path, encoding="utf-8") as handle:
    for raw in handle:
        line = raw.strip()
        if not line:
            continue
        fields = line.split("|")
        if len(fields) != 4 or any(not field for field in fields):
            raise SystemExit(f"invalid release target row: {line}")
        rows.append(tuple(fields))

if len(rows) != 7 or len({row[0] for row in rows}) != 7 or len({row[1] for row in rows}) != 7:
    raise SystemExit("release target compatibility table must contain seven unique targets and assets")

with open(contract_path, "rb") as handle:
    contract = tomllib.load(handle)
if contract["schema_version"] != 1:
    raise SystemExit("distribution contract schema_version must be 1")
if contract["product"]["id"] != "eggsearch":
    raise SystemExit(f"distribution contract product id is {contract['product']['id']!r}, want 'eggsearch'")

contract_targets = {}
for entry in contract["targets"]:
    triple = entry["triple"]
    asset_form = entry["asset"]
    if asset_form.get("kind") != "direct":
        raise SystemExit(f"contract target {triple} is not a direct asset form")
    asset = asset_form["asset"].replace("{product}", "eggsearch").replace("{target}", triple)
    sidecar = entry["checksum"]["sidecar"].replace("{asset}", asset)
    if sidecar != f"{asset}.sha256":
        raise SystemExit(f"contract sidecar for {triple} is not '{asset}.sha256'")
    install = asset_form["install"].replace("{product}", "eggsearch").replace("{target}", triple)
    if install != "eggsearch" and install != "eggsearch.exe":
        raise SystemExit(
            f"contract install name for {triple} is {install!r}, want the bare product name"
        )
    contract_targets[triple] = (tuple(entry["aliases"]), asset)

if len(contract_targets) != 7:
    raise SystemExit("distribution contract must define exactly seven targets")

mirror = {row[0]: row[1] for row in rows}
if set(mirror) != set(contract_targets):
    raise SystemExit("release-targets.txt and the distribution contract cover different targets")
for triple, asset in mirror.items():
    if contract_targets[triple][1] != asset:
        raise SystemExit(
            f"release-targets.txt asset for {triple} is {asset!r}, contract expands to "
            f"{contract_targets[triple][1]!r}"
        )

# The generated workflow must carry every contracted target and every public
# asset name, and must never request the permissions or flags the producer
# contract forbids.
def read(path):
    with open(path, encoding="utf-8") as handle:
        return handle.read()

workflow = read(workflow_path)
# The generated workflow addresses candidates by canonical triple and by the
# bound package/binary pair. Public asset names are resolved at run time from
# the same contract, so asserting triple coverage here is the meaningful check;
# `eggpack ci check` proves the workflow renders from these exact inputs.
for triple in contract_targets:
    if triple not in workflow:
        raise SystemExit(f"generated workflow does not mention target {triple}")

for forbidden in ("--clobber", "id-token: write", "attestations: write", "artifact-metadata: write"):
    if forbidden in workflow:
        raise SystemExit(f"generated workflow must not contain {forbidden!r}")
if "contents: write" not in workflow:
    raise SystemExit("generated workflow has no draft-staging write scope")
if workflow.count("contents: write") != 1:
    raise SystemExit("generated workflow must request contents: write exactly once")
if "_stage-github-draft" not in workflow:
    raise SystemExit("generated workflow does not stage a draft release")
if re.search(r"^\s*push:\s*$", workflow, re.MULTILINE):
    # Dispatch-only is what keeps the generated writer from ever racing a tag
    # push. The legacy writer triggers on `v*` tag pushes, so while both files
    # coexist the generated workflow must stay opt-in and explicit.
    raise SystemExit("generated workflow must not add a push trigger")
for forbidden in ("gh release publish", "release publish", "git push origin --tags", "git tag "):
    if forbidden in workflow:
        raise SystemExit(f"generated workflow must not contain {forbidden!r}")

# Provenance is an authenticity layer over already-staged bytes. The invariant
# is that it never *behaves* as a writer, not that it never holds a write-scoped
# token. GitHub answers `GET /releases/tags/{tag}` with 404 for a draft release
# unless the token carries push access, so a read-scoped token cannot observe the
# very draft this workflow exists to attest and fails closed with "release not
# found". Assert the behavioural property directly, and confine any write scope
# to the job that needs it rather than the whole file.
provenance = read(provenance_path)
if "attestations: write" not in provenance:
    raise SystemExit("provenance workflow must request attestations: write")

# Comment lines are stripped first so that prose explaining *why* a write scope
# exists cannot itself satisfy or trip a structural assertion.
provenance_code = "\n".join(
    line for line in provenance.splitlines() if not line.lstrip().startswith("#")
)
_lines = provenance_code.splitlines()
for _i, _line in enumerate(_lines):
    if re.match(r"^permissions:\s*$", _line):
        _j = _i + 1
        while _j < len(_lines) and _lines[_j].strip() and _lines[_j][:1].isspace():
            if _lines[_j].strip() == "contents: write":
                raise SystemExit(
                    "provenance workflow must keep release-write scope job-scoped, "
                    "not workflow-wide"
                )
            _j += 1
for forbidden in ("gh release create", "gh release edit", "gh release upload", "gh release delete", "--clobber"):
    if forbidden in provenance_code:
        raise SystemExit(f"provenance workflow must not mutate release state: {forbidden!r}")

unix = read(unix_path)
windows = read(windows_path)
rust = read(rust_path)
docs = read(docs_path)
unix_pairs = re.findall(r'TARGET="([^"]+)"\s+ASSET="([^"]+)"', unix)
windows_pairs = re.findall(r"\$Target = '([^']+)'\s+\$Asset = '([^']+)'", windows)
rust_pairs = re.findall(r'rust_target:\s*"([^"]+)",\s*asset:\s*"([^"]+)",', rust)
doc_pairs = re.findall(r"\|\s*`([^`]+)`\s*\|\s*`([^`]+)`\s*\|", docs)
if unix_pairs != [(row[0], row[1]) for row in rows if row[2] != "windows"]:
    raise SystemExit("Unix installer target mapping does not exactly match release-targets.txt")
if windows_pairs != [(row[0], row[1]) for row in rows if row[2] == "windows"]:
    raise SystemExit("PowerShell installer target mapping does not exactly match release-targets.txt")
if rust_pairs != [(row[0], row[1]) for row in rows]:
    raise SystemExit("Rust updater target mapping does not exactly match release-targets.txt")
if not all((row[0], row[1]) in doc_pairs for row in rows):
    raise SystemExit("installation documentation target mapping does not match release-targets.txt")

for target, asset, os_name, _arch in rows:
    if os_name == "windows" and not asset.endswith(".exe"):
        raise SystemExit(f"Windows release asset lacks .exe suffix: {asset}")
    if os_name != "windows" and asset.endswith(".exe"):
        raise SystemExit(f"non-Windows release asset has .exe suffix: {asset}")
PY

bash -n "$unix_installer" "$root/packaging/release-smoke.sh" "$root/packaging/release-validate.sh"
"$root/packaging/check-egress-qualify-contract.sh"
"$unix_installer" --help >/dev/null
"$root/packaging/test-install.sh"

if command -v pwsh >/dev/null 2>&1; then
    EGGSEARCH_INSTALLER="$windows_installer" pwsh -NoProfile -Command '$tokens = $null; $errors = $null; [System.Management.Automation.Language.Parser]::ParseFile($env:EGGSEARCH_INSTALLER, [ref]$tokens, [ref]$errors) | Out-Null; if ($errors.Count -gt 0) { $errors | ForEach-Object { Write-Error $_.Message }; exit 1 }'
    pwsh -NoProfile -File "$root/packaging/test-install.ps1"
fi

guard_output="$(mktemp)"
trap 'rm -f "$guard_output"' EXIT
shell_under_test="$(command -v dash || command -v sh)"
if "$shell_under_test" "$unix_installer" --help >"$guard_output" 2>&1; then
    echo "installer unexpectedly ran under sh" >&2
    exit 1
fi
grep -F "requires bash" "$guard_output" >/dev/null

grep -F "cargo install eggsearch --version" "$unix_installer" >/dev/null
grep -F "cargo install eggsearch --locked" "$unix_installer" >/dev/null
grep -F "Invoke-WebRequest" "$windows_installer" >/dev/null
grep -F "Get-FileHash -Algorithm SHA256" "$windows_installer" >/dev/null
if grep -F "sudo" "$unix_installer" >/dev/null; then
    echo "installer must never invoke sudo" >&2
    exit 1
fi