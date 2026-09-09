#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ] || ! command -v shopt >/dev/null 2>&1; then
    echo "repository hygiene checks require bash" >&2
    exit 2
fi

set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
failures=0

fail() {
    echo "repo-hygiene: $1" >&2
    failures=$((failures + 1))
}

tracked="$(mktemp)"
trap 'rm -f "$tracked"' EXIT
git -C "$root" ls-files > "$tracked"

while IFS= read -r path; do
    case "$path" in
        typescript|typescript.*|script.log|*.script|coverage/*|*.profraw|*.profdata)
            fail "forbidden transcript/log artifact is tracked: $path"
            ;;
    esac
    case "$path" in
        target/*|fuzz/target/*|node_modules/*|dist/*|build/*|__pycache__/*|*.pyc|*.pyo)
            fail "tracked build output: $path"
            ;;
    esac
    case "$path" in
        *.swp|*.swo|*~|*.rs.bk|*.bak|.DS_Store|*/.DS_Store)
            fail "tracked temporary/editor artifact: $path"
            ;;
    esac
done < "$tracked"

if git -C "$root" ls-files | grep -Ex 'test_struct' >/dev/null; then
    fail "tracked stray root binary: test_struct"
fi

esc_match="$(mktemp)"
trap 'rm -f "$tracked" "$esc_match"' EXIT
: > "$esc_match"
while IFS= read -r path; do
    case "$path" in
        *:*|*\\*|*" "*)
            ;;
    esac
    case "$path" in
        */* | .* | Cargo.lock | CHANGELOG.md)
            continue
            ;;
    esac
    f="$root/$path"
    if [ ! -f "$f" ]; then
        continue
    fi
    size="$(wc -c < "$f" | tr -d '[:space:]')"
    case "$size" in
        ''|*[!0-9]*)
            continue
            ;;
    esac
    if [ "$size" -gt 1048576 ]; then
        continue
    fi
    if grep -q "$(printf '\033')" "$f" 2>/dev/null; then
        echo "$path" >> "$esc_match"
    fi
done < "$tracked"
if [ -s "$esc_match" ]; then
    fail "ANSI escape sequences in unexpected root text artifacts: $(tr '\n' ' ' < "$esc_match")"
fi

size_fail=0
while IFS= read -r path; do
    case "$path" in
        */* | .* )
            continue
            ;;
    esac
    case "$path" in
        Cargo.lock|CHANGELOG.md|README.md|AGENTS.md|LICENSE)
            continue
            ;;
    esac
    f="$root/$path"
    if [ ! -f "$f" ]; then
        continue
    fi
    size="$(wc -c < "$f" | tr -d '[:space:]')"
    case "$size" in
        ''|*[!0-9]*)
            continue
            ;;
    esac
    if [ "$size" -gt 102400 ]; then
        echo "repo-hygiene: oversized unexpected root blob: $path (${size} bytes)" >&2
        size_fail=1
    fi
done < "$tracked"
if [ "$size_fail" -ne 0 ]; then
    failures=$((failures + 1))
fi

if [ "$failures" -ne 0 ]; then
    echo "repo-hygiene: $failures check(s) failed" >&2
    exit 1
fi
echo "repo-hygiene: ok"
