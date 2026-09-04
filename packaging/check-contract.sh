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

test "$(grep -c . "$targets")" -eq 7
while IFS='|' read -r target asset os _arch; do
    [[ -z "$target" ]] && continue
    test -n "$target"
    test -n "$asset"
    if [[ "$os" == "windows" ]]; then
        grep -F "$target" "$windows_installer" >/dev/null
        grep -F "$asset" "$windows_installer" >/dev/null
    else
        grep -F "$target" "$unix_installer" >/dev/null
        grep -F "$asset" "$unix_installer" >/dev/null
    fi
    grep -F "$target" "$workflow" >/dev/null
    grep -F "$asset" "$workflow" >/dev/null
    if [[ "$os" == "windows" ]]; then
        [[ "$asset" == *.exe ]]
    else
        [[ "$asset" != *.exe ]]
    fi
done < "$targets"

bash -n "$unix_installer"
bash -n "$root/packaging/release-smoke.sh"
"$unix_installer" --help >/dev/null
"$root/packaging/test-install.sh"
if "$unix_installer" --version invalid >/dev/null 2>&1; then
    echo "installer accepted an invalid version" >&2
    exit 1
fi
guard_output="$(mktemp)"
trap 'rm -f "$guard_output"' EXIT
if command -v dash >/dev/null 2>&1; then
    shell_under_test=dash
else
    shell_under_test='sh'
fi
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
