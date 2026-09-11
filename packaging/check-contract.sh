#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ] || ! command -v shopt >/dev/null 2>&1; then
    echo "packaging contract checks require bash" >&2
    exit 2
fi

set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
targets="$root/packaging/release-targets.txt"
workflow="$root/.github/workflows/release-binaries.yml"
unix_installer="$root/packaging/install.sh"
windows_installer="$root/packaging/install.ps1"
rust_contract="$root/src/platform.rs"
docs_installation="$root/docs/installation.md"

"$root/packaging/release-validate.sh" tree "$(git -C "$root" rev-parse HEAD)"

python3 - "$targets" "$workflow" "$unix_installer" "$windows_installer" "$rust_contract" "$docs_installation" <<'PY'
import re
import os
import sys

targets_path, workflow_path, unix_path, windows_path, rust_path, docs_path = sys.argv[1:]
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
    raise SystemExit("release target contract must contain seven unique targets and assets")

def read(path):
    with open(path, encoding="utf-8") as handle:
        return handle.read()

workflow = read(workflow_path)
workflow_targets = re.findall(r"^\s+target:\s*([^\s#]+)", workflow, re.MULTILINE)
workflow_assets = re.findall(r"^\s+asset:\s*([^\s#]+)", workflow, re.MULTILINE)
expected_targets = [row[0] for row in rows]
expected_assets = [row[1] for row in rows]
matrix_rows = [row for row in rows if row[0] != "armv7-unknown-linux-gnueabihf"]
if workflow_targets != [row[0] for row in matrix_rows] or workflow_assets != [row[1] for row in matrix_rows]:
    raise SystemExit("release workflow target matrix does not exactly match release-targets.txt")
if "target armv7-unknown-linux-gnueabihf.2.17" not in workflow or 'asset="eggsearch-armv7-unknown-linux-gnueabihf"' not in workflow:
    raise SystemExit("release workflow armv7 target does not match release-targets.txt")
if workflow.count("name: ${{ needs.preflight.outputs.artifact_prefix }}-${{ matrix.target }}") != 3:
    raise SystemExit("release workflow artifact names are not unique per target job")
if "merge-multiple: true" not in workflow or "release-validate.sh assets" not in workflow:
    raise SystemExit("release workflow is missing merged exact asset validation")
for path in set(re.findall(r"packaging/[A-Za-z0-9_./-]+", workflow)):
    if not os.path.exists(os.path.join(os.path.dirname(targets_path), path.removeprefix("packaging/"))):
        raise SystemExit(f"release workflow references missing packaging path: {path}")

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

for target, asset, os_name, arch in rows:
    if os_name == "windows" and not asset.endswith(".exe"):
        raise SystemExit(f"Windows release asset lacks .exe suffix: {asset}")
    if os_name != "windows" and asset.endswith(".exe"):
        raise SystemExit(f"non-Windows release asset has .exe suffix: {asset}")
PY

bash -n "$unix_installer" "$root/packaging/release-smoke.sh" "$root/packaging/release-validate.sh"
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
