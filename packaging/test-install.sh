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
mkdir -p "$fake_bin" "$home_dir"

printf '#!/usr/bin/env bash\nif [ "$1" = "--version" ]; then echo "eggsearch 0.3.8"; else echo "mock"; fi\n' > "$good_bin"
printf '#!/usr/bin/env bash\nif [ "$1" = "--version" ]; then echo "eggsearch 0.3.7"; else echo "mock"; fi\n' > "$bad_bin"
chmod 0755 "$good_bin" "$bad_bin"

printf '%s\n' '#!/usr/bin/env bash' 'case "$1" in -s) echo Linux;; -m) echo x86_64;; *) exit 1;; esac' > "$fake_bin/uname"
printf '%s\n' '#!/usr/bin/env bash' 'set -euo pipefail' 'output=""' 'url=""' 'while (($# > 0)); do' '  case "$1" in' '    --output) output="$2"; shift 2;;' '    --write-out) shift 2;;' '    --*) shift;;' '    *) url="$1"; shift;;' '  esac' 'done' 'if [[ "$MOCK_MODE" == 404 && "$url" != *.sha256 ]]; then echo 404; exit 0; fi' 'if [[ "$url" == *.sha256 ]]; then' '  if [[ "$MOCK_MODE" == checksum-mismatch ]]; then printf "%064d  eggsearch-x86_64-unknown-linux-gnu\n" 0 > "$output"; else printf "%s  eggsearch-x86_64-unknown-linux-gnu\n" "$(sha256sum "$MOCK_BIN" | awk "{print \$1}")" > "$output"; fi' 'else' '  if [[ "$MOCK_MODE" == bad-version ]]; then cp "$MOCK_BAD" "$output"; else cp "$MOCK_BIN" "$output"; fi' 'fi' 'echo 200' > "$fake_bin/curl"
printf '%s\n' '#!/usr/bin/env bash' 'set -euo pipefail' 'root=""' 'while (($# > 0)); do' '  if [[ "$1" == --root ]]; then root="$2"; shift 2; else shift; fi' 'done' 'mkdir -p "$root/bin"' 'cp "$MOCK_BIN" "$root/bin/eggsearch"' > "$fake_bin/cargo"
chmod 0755 "$fake_bin/uname" "$fake_bin/curl" "$fake_bin/cargo"

run_installer() {
    HOME="$home_dir" PATH="$fake_bin:$PATH" MOCK_BIN="$good_bin" MOCK_BAD="$bad_bin" MOCK_MODE="$1" "$root/packaging/install.sh" --version 0.3.8
}

run_installer success
test "$("$home_dir/.local/bin/eggsearch" --version)" = "eggsearch 0.3.8"

rm -f "$home_dir/.local/bin/eggsearch"
if run_installer checksum-mismatch >/dev/null 2>&1; then
    echo "checksum mismatch was accepted" >&2
    exit 1
fi
test ! -e "$home_dir/.local/bin/eggsearch"

if run_installer bad-version >/dev/null 2>&1; then
    echo "candidate version mismatch was accepted" >&2
    exit 1
fi
test ! -e "$home_dir/.local/bin/eggsearch"

run_installer 404 >/dev/null
test "$("$home_dir/.local/bin/eggsearch" --version)" = "eggsearch 0.3.8"
