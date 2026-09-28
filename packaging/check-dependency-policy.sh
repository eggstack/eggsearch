#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
python3 packaging/check-dependency-policy.py
python3 packaging/check-http-dependency-shape.py
if ! cargo deny --version | grep -Fxq 'cargo-deny 0.20.2'; then
    cargo install cargo-deny --version 0.20.2 --locked
fi
cargo deny check
