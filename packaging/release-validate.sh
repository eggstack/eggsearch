#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ] || ! command -v shopt >/dev/null 2>&1; then
    echo "release validation requires bash" >&2
    exit 2
fi

set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
targets="$root/packaging/release-targets.txt"
inputs="$root/packaging/release-inputs.txt"

check_tree() {
    local commit="${1:-$(git -C "$root" rev-parse HEAD)}"
    local missing=0
    while IFS= read -r path; do
        [[ -z "$path" ]] && continue
        if [[ ! -e "$root/$path" ]]; then
            echo "release input missing from commit $commit: $path" >&2
            missing=1
        fi
    done < "$inputs"
    if ((missing)); then
        return 1
    fi
}

package_version() {
    cargo metadata --locked --no-deps --format-version 1 |
        jq -r '.packages[] | select(.name == "eggsearch") | .version'
}

valid_version() {
    [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]
}

expected_assets() {
    while IFS='|' read -r _target asset _os _arch; do
        [[ -z "$asset" ]] && continue
        printf '%s\n' "$asset" "$asset.sha256"
    done < "$targets"
    printf '%s\n' install.sh install.ps1
}

verify_asset_set() {
    local dist="$1"
    local mode="$2"
    local commit="$3"
    local tag="${4:--}"
    local version="$5"
    local expected_file observed_file missing_file unexpected_file
    expected_file="$(mktemp)"
    observed_file="$(mktemp)"
    missing_file="$(mktemp)"
    unexpected_file="$(mktemp)"
    trap 'rm -f "$expected_file" "$observed_file" "$missing_file" "$unexpected_file"' RETURN

    expected_assets | sort > "$expected_file"
    find "$dist" -maxdepth 1 -type f -exec basename {} \; | sort > "$observed_file"

    echo "release asset validation"
    echo "mode=$mode"
    echo "commit=$commit"
    echo "package_version=$version"
    echo "tag=$tag"
    echo "expected assets:"
    sed 's/^/  /' "$expected_file"
    echo "observed assets:"
    sed 's/^/  /' "$observed_file"

    comm -23 "$expected_file" "$observed_file" > "$missing_file"
    comm -13 "$expected_file" "$observed_file" > "$unexpected_file"
    if [[ -s "$missing_file" || -s "$unexpected_file" ]]; then
        echo "missing assets:"
        sed 's/^/  /' "$missing_file"
        echo "unexpected assets:"
        sed 's/^/  /' "$unexpected_file"
        return 1
    fi

    while IFS='|' read -r _target asset _os _arch; do
        [[ -z "$asset" ]] && continue
        local checksum="$dist/$asset.sha256"
        if command -v sha256sum >/dev/null 2>&1; then
            (cd "$dist" && sha256sum -c "$asset.sha256")
        else
            (cd "$dist" && shasum -a 256 -c "$asset.sha256")
        fi
        [[ -x "$dist/$asset" || "$asset" == *.exe ]] || {
            echo "release asset is not executable: $asset" >&2
            return 1
        }
        test -s "$checksum"
    done < "$targets"
}

case "${1:-}" in
    tree)
        check_tree "${2:-}"
        ;;
    candidate)
        check_tree
        version="$(package_version)"
        valid_version "$version" || {
            echo "Cargo package version is not a release candidate: $version" >&2
            exit 1
        }
        ;;
    assets)
        verify_asset_set "$2" "$3" "$4" "${5:--}" "$6"
        ;;
    *)
        echo "Usage: $0 tree [COMMIT] | candidate | assets DIST MODE COMMIT TAG VERSION" >&2
        exit 2
        ;;
esac
