#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ] || ! command -v shopt >/dev/null 2>&1; then
    echo "eggsearch installer requires bash; re-run the command with bash." >&2
    exit 2
fi

set -euo pipefail

REPOSITORY="eggstack/eggsearch"
VERSION=""

usage() {
    printf 'Usage: %s [--version X.Y.Z]\n' "$0"
}

while (($# > 0)); do
    case "$1" in
        --version)
            if (($# < 2)); then
                echo "--version requires a value" >&2
                usage >&2
                exit 2
            fi
            VERSION="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ -n "$VERSION" && ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
    echo "invalid version: $VERSION" >&2
    exit 2
fi

if [[ -z "${HOME:-}" ]]; then
    echo "HOME is not set; cannot select a user-local install directory" >&2
    exit 1
fi

if ((EUID == 0)); then
    INSTALL_DIR="/usr/local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
fi

OS="$(uname -s)"
MACHINE="$(uname -m)"
TARGET=""
ASSET=""

case "$OS:$MACHINE" in
    Linux:x86_64|Linux:amd64)
        TARGET="x86_64-unknown-linux-gnu"
        ASSET="eggsearch-x86_64-unknown-linux-gnu"
        ;;
    Linux:aarch64|Linux:arm64)
        TARGET="aarch64-unknown-linux-gnu"
        ASSET="eggsearch-aarch64-unknown-linux-gnu"
        ;;
    Linux:armv7l|Linux:armv7)
        TARGET="armv7-unknown-linux-gnueabihf"
        ASSET="eggsearch-armv7-unknown-linux-gnueabihf"
        ;;
    Darwin:x86_64|Darwin:amd64)
        TARGET="x86_64-apple-darwin"
        ASSET="eggsearch-x86_64-apple-darwin"
        ;;
    Darwin:arm64|Darwin:aarch64)
        TARGET="aarch64-apple-darwin"
        ASSET="eggsearch-aarch64-apple-darwin"
        ;;
esac

TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/eggsearch-install.XXXXXX")"
cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

download() {
    local url="$1"
    local output="$2"
    local status
    if ! status="$(curl --silent --show-error --location --output "$output" --write-out '%{http_code}' "$url")"; then
        echo "download failed: $url" >&2
        return 1
    fi
    if [[ "$status" != "200" ]]; then
        echo "download returned HTTP $status: $url" >&2
        if [[ "$status" == "404" ]]; then
            return 10
        fi
        return 1
    fi
}

verify_checksum() {
    local checksum_file="$1"
    local asset_file="$2"
    local expected_name
    local expected_digest
    read -r expected_digest expected_name < "$checksum_file"
    if [[ ! "$expected_digest" =~ ^[0-9A-Fa-f]{64}$ || "$expected_name" != "$ASSET" ]]; then
        echo "invalid checksum file for $ASSET" >&2
        return 1
    fi
    if command -v sha256sum >/dev/null 2>&1; then
        (cd "$(dirname "$asset_file")" && printf '%s  %s\n' "$expected_digest" "$ASSET" | sha256sum -c -)
    elif command -v shasum >/dev/null 2>&1; then
        local actual_digest
        actual_digest="$(shasum -a 256 "$asset_file" | awk '{print $1}')"
        [[ "$actual_digest" == "$expected_digest" ]] || {
            echo "checksum mismatch for $ASSET" >&2
            return 1
        }
    else
        echo "sha256sum or shasum is required to verify the release binary" >&2
        return 1
    fi
}

install_candidate() {
    local candidate="$1"
    local destination="$INSTALL_DIR/eggsearch"
    mkdir -p "$INSTALL_DIR"
    local staged="$INSTALL_DIR/.eggsearch.$$"
    install -m 0755 "$candidate" "$staged"
    mv -f "$staged" "$destination"
    echo "installed eggsearch at $destination"
    case ":${PATH:-}:" in
        *":$INSTALL_DIR:"*) ;;
        *) echo "add $INSTALL_DIR to PATH to invoke eggsearch directly" ;;
    esac
}

install_from_cargo() {
    command -v cargo >/dev/null 2>&1 || {
        echo "Cargo is required for this unsupported target or missing release asset. Install Rust from https://rustup.rs/ and retry." >&2
        exit 1
    }
    local cargo_root
    if ((EUID == 0)); then
        cargo_root="/usr/local"
    else
        cargo_root="$HOME/.local"
    fi
    mkdir -p "$cargo_root"
    if [[ -n "$VERSION" ]]; then
        cargo install eggsearch --version "$VERSION" --locked --root "$cargo_root"
    else
        cargo install eggsearch --locked --root "$cargo_root"
    fi
    local candidate="$cargo_root/bin/eggsearch"
    [[ -x "$candidate" ]] || {
        echo "Cargo completed without producing $candidate" >&2
        exit 1
    }
    local output
    output="$($candidate --version)" || {
        echo "Cargo-installed eggsearch failed its version check" >&2
        exit 1
    }
    [[ "$output" == *eggsearch* ]] || {
        echo "Cargo-installed candidate did not identify as eggsearch" >&2
        exit 1
    }
    local candidate_version
    candidate_version="$(printf '%s\n' "$output" | awk '$1 == "eggsearch" { print $2; exit }')"
    if [[ -n "$VERSION" && "$candidate_version" != "$VERSION" ]]; then
        echo "Cargo-installed candidate version mismatch: expected $VERSION, got $output" >&2
        exit 1
    fi
    install_candidate "$candidate"
}

if [[ -z "$TARGET" ]]; then
    echo "no prebuilt eggsearch release for $OS/$MACHINE; using the documented Cargo fallback"
    install_from_cargo
    exit 0
fi

command -v curl >/dev/null 2>&1 || {
    echo "curl is required to bootstrap the supported $TARGET release binary" >&2
    exit 1
}

if [[ -n "$VERSION" ]]; then
    BASE_URL="https://github.com/$REPOSITORY/releases/download/v$VERSION"
else
    BASE_URL="https://github.com/$REPOSITORY/releases/latest/download"
fi

CANDIDATE="$TEMP_DIR/$ASSET"
CHECKSUM="$TEMP_DIR/$ASSET.sha256"
if download "$BASE_URL/$ASSET" "$CANDIDATE"; then
    :
else
    status=$?
    if [[ "$status" -eq 10 ]]; then
        echo "release asset is unavailable for $TARGET; using the documented Cargo fallback"
        install_from_cargo
        exit 0
    fi
    exit "$status"
fi

download "$BASE_URL/$ASSET.sha256" "$CHECKSUM"
verify_checksum "$CHECKSUM" "$CANDIDATE"
chmod 0755 "$CANDIDATE"

VERSION_OUTPUT="$($CANDIDATE --version)" || {
    echo "downloaded eggsearch candidate failed its version check" >&2
    exit 1
}
[[ "$VERSION_OUTPUT" == *eggsearch* ]] || {
    echo "downloaded candidate did not identify as eggsearch" >&2
    exit 1
}
candidate_version="$(printf '%s\n' "$VERSION_OUTPUT" | awk '$1 == "eggsearch" { print $2; exit }')"
if [[ -n "$VERSION" && "$candidate_version" != "$VERSION" ]]; then
    echo "downloaded candidate version mismatch: expected $VERSION, got $VERSION_OUTPUT" >&2
    exit 1
fi

install_candidate "$CANDIDATE"
