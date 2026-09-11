#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ] || ! command -v shopt >/dev/null 2>&1; then
    echo "installer tests require bash" >&2
    exit 2
fi

set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/eggsearch-installer-test.XXXXXX")"
trap 'rm -rf "$temp_dir"' EXIT
good_bin="$temp_dir/mock-good"
bad_bin="$temp_dir/mock-bad"
fake_bin="$temp_dir/bin"
home_dir="$temp_dir/home"
url_log="$temp_dir/urls"
cargo_log="$temp_dir/cargo"
mkdir -p "$fake_bin" "$home_dir"

printf '#!/usr/bin/env bash\nif [ "$1" = "--version" ]; then echo "eggsearch 0.3.8"; else echo "mock"; fi\n' > "$good_bin"
printf '#!/usr/bin/env bash\nif [ "$1" = "--version" ]; then echo "eggsearch 0.3.7"; else echo "mock"; fi\n' > "$bad_bin"
chmod 0755 "$good_bin" "$bad_bin"

printf '%s\n' '#!/usr/bin/env bash' 'case "$1" in -s) echo Linux;; -m) echo x86_64;; *) echo "Unknown";; esac' > "$fake_bin/uname"
printf '%s\n' '#!/usr/bin/env bash' 'set -euo pipefail' 'output=""' 'url=""' 'while (($# > 0)); do' '  case "$1" in' '    --output) output="$2"; shift 2;;' '    --write-out) shift 2;;' '    --*) shift;;' '    *) url="$1"; shift;;' '  esac' 'done' 'printf "%s\n" "$url" >> "$MOCK_URL_LOG"' 'if [[ "$MOCK_MODE" == network ]]; then exit 7; fi' 'if [[ "$MOCK_MODE" == http-500 && "$url" != *.sha256 ]]; then echo 500; exit 0; fi' 'if [[ "$MOCK_MODE" == checksum-404 && "$url" == *.sha256 ]]; then echo 404; exit 0; fi' 'if [[ "$MOCK_MODE" == 404 && "$url" != *.sha256 ]]; then echo 404; exit 0; fi' 'if [[ "$url" == *.sha256 ]]; then' '  if [[ "$MOCK_MODE" == checksum-mismatch ]]; then printf "%064d  eggsearch-x86_64-unknown-linux-gnu\n" 0 > "$output"; else printf "%s  eggsearch-x86_64-unknown-linux-gnu\n" "$(sha256sum "$MOCK_BIN" | awk "{print \$1}")" > "$output"; fi' 'else' '  if [[ "$MOCK_MODE" == bad-version ]]; then cp "$MOCK_BAD" "$output"; else cp "$MOCK_BIN" "$output"; fi' 'fi' 'echo 200' > "$fake_bin/curl"
printf '%s\n' '#!/usr/bin/env bash' 'set -euo pipefail' 'printf "%s\n" cargo >> "$MOCK_CARGO_LOG"' 'root=""' 'while (($# > 0)); do' '  if [[ "$1" == --root ]]; then root="$2"; shift 2; else shift; fi' 'done' 'mkdir -p "$root/bin"' 'cp "$MOCK_BIN" "$root/bin/eggsearch"' > "$fake_bin/cargo"
chmod 0755 "$fake_bin/uname" "$fake_bin/curl" "$fake_bin/cargo"

run_installer() {
    local mode="$1"
    : > "$url_log"
    : > "$cargo_log"
    HOME="$home_dir" PATH="$fake_bin:$PATH" MOCK_BIN="$good_bin" MOCK_BAD="$bad_bin" MOCK_MODE="$mode" MOCK_URL_LOG="$url_log" MOCK_CARGO_LOG="$cargo_log" EGGSEARCH_INSTALL_TEST_MODE=1 EGGSEARCH_INSTALL_TEST_BASE_URL="http://mock/releases/latest/download" "$root/packaging/install.sh" "${2:---version}" "${3:-0.3.8}"
}

run_unpinned_installer() {
    local mode="$1"
    : > "$url_log"
    : > "$cargo_log"
    HOME="$home_dir" PATH="$fake_bin:$PATH" MOCK_BIN="$good_bin" MOCK_BAD="$bad_bin" MOCK_MODE="$mode" MOCK_URL_LOG="$url_log" MOCK_CARGO_LOG="$cargo_log" EGGSEARCH_INSTALL_TEST_MODE=1 EGGSEARCH_INSTALL_TEST_BASE_URL="http://mock/releases/latest/download" "$root/packaging/install.sh"
}

run_installer success --version 0.3.8 > "$temp_dir/success.log"
test "$("$home_dir/.local/bin/eggsearch" --version)" = "eggsearch 0.3.8"
grep -F 'http://mock/releases/latest/download/eggsearch-x86_64-unknown-linux-gnu' "$url_log" >/dev/null
grep -F 'installed eggsearch at' "$temp_dir/success.log" >/dev/null
grep -F 'add ' "$temp_dir/success.log" >/dev/null
test ! -s "$cargo_log"

rm -f "$home_dir/.local/bin/eggsearch"
if run_installer http-500 --version 0.3.8 >/dev/null 2>&1; then exit 1; fi
test ! -e "$home_dir/.local/bin/eggsearch"
test ! -s "$cargo_log"

if run_installer network --version 0.3.8 >/dev/null 2>&1; then exit 1; fi
test ! -s "$cargo_log"

if run_installer checksum-404 --version 0.3.8 >/dev/null 2>&1; then exit 1; fi
test ! -s "$cargo_log"

if run_installer checksum-mismatch --version 0.3.8 >/dev/null 2>&1; then exit 1; fi
test ! -e "$home_dir/.local/bin/eggsearch"
test ! -s "$cargo_log"

if run_installer bad-version --version 0.3.8 >/dev/null 2>&1; then exit 1; fi
test ! -e "$home_dir/.local/bin/eggsearch"
test ! -s "$cargo_log"

rm -f "$home_dir/.local/bin/eggsearch"
run_installer 404 --version 0.3.8 >/dev/null 2>&1
test "$("$home_dir/.local/bin/eggsearch" --version)" = "eggsearch 0.3.8"
grep -F 'cargo' "$cargo_log" >/dev/null

rm -f "$home_dir/.local/bin/eggsearch"
run_unpinned_installer success >/dev/null
grep -F 'releases/latest/download' "$url_log" >/dev/null

rm -f "$home_dir/.local/bin/eggsearch"
HOME="$home_dir" PATH="$fake_bin:$PATH" MOCK_BIN="$good_bin" MOCK_BAD="$bad_bin" MOCK_MODE=success MOCK_URL_LOG="$url_log" MOCK_CARGO_LOG="$cargo_log" EGGSEARCH_INSTALL_TEST_MODE=1 EGGSEARCH_INSTALL_TEST_BASE_URL="http://mock/releases/download/v0.3.8" "$root/packaging/install.sh" --version 0.3.8 >/dev/null
grep -F 'http://mock/releases/download/v0.3.8/eggsearch-x86_64-unknown-linux-gnu' "$url_log" >/dev/null

mkdir -p "$temp_dir/unsupported-home"
printf '%s\n' '#!/usr/bin/env bash' 'case "$1" in -s) echo Plan9;; -m) echo weird;; *) exit 1;; esac' > "$fake_bin/uname"
chmod 0755 "$fake_bin/uname"
: > "$cargo_log"
HOME="$temp_dir/unsupported-home" PATH="$fake_bin:$PATH" MOCK_BIN="$good_bin" MOCK_CARGO_LOG="$cargo_log" "$root/packaging/install.sh" --version 0.3.8 >/dev/null
test -x "$temp_dir/unsupported-home/.local/bin/eggsearch"
grep -F cargo "$cargo_log" >/dev/null
