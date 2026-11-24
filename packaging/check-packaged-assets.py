#!/usr/bin/env python3
"""Assert that every asset a published crate reads at compile time is packaged.

`cargo package` selects files from `[package] include`, so an asset referenced
by `include_str!` or `include_bytes!` under `src/` that is not matched by an
include pattern is absent from the crate tarball and from the maturin sdist.
The extracted artifact then fails to compile.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
INCLUDE_CALL = re.compile(r"include_(?:str|bytes)!\s*\(")
INCLUDE_LITERAL = re.compile(r'include_(?:str|bytes)!\s*\(\s*"([^"]+)"\s*\)')


def package_files(manifest: pathlib.Path) -> set[str]:
    completed = subprocess.run(
        [
            "cargo",
            "package",
            "--list",
            "--allow-dirty",
            "--locked",
            "--quiet",
            "--manifest-path",
            str(manifest),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        sys.stderr.write(completed.stderr)
        raise SystemExit("packaged-assets: cargo package --list failed")
    return {line.strip() for line in completed.stdout.splitlines() if line.strip()}


def strip_line_comments(source: str) -> str:
    return "\n".join(line.split("//", 1)[0] for line in source.splitlines())


def main() -> int:
    packaged = package_files(ROOT / "Cargo.toml")
    binding_packaged = package_files(ROOT / "bindings/python/Cargo.toml")
    problems: list[str] = []
    binding_lock = ROOT / "bindings/python/Cargo.lock"
    if not binding_lock.is_file() or "Cargo.lock" not in binding_packaged:
        problems.append("bindings/python/Cargo.lock is missing from the binding package file set")
    checked = 0
    for source in sorted((ROOT / "src").rglob("*.rs")):
        text = strip_line_comments(source.read_text(encoding="utf-8", errors="replace"))
        calls = len(INCLUDE_CALL.findall(text))
        literals = INCLUDE_LITERAL.findall(text)
        origin = source.relative_to(ROOT).as_posix()
        if calls != len(literals):
            problems.append(
                f"{origin}: {calls} include_str!/include_bytes! call(s) but "
                f"{len(literals)} string literal(s); use a literal path so this guard can see it"
            )
        for literal in literals:
            checked += 1
            target = (source.parent / literal).resolve()
            try:
                relative = target.relative_to(ROOT).as_posix()
            except ValueError:
                problems.append(f"{origin}: {literal} resolves outside the package root")
                continue
            if not target.is_file():
                problems.append(f"{origin}: {literal} does not exist")
            elif relative not in packaged:
                problems.append(
                    f"{origin}: {literal} is not matched by [package] include in Cargo.toml"
                )
    for problem in problems:
        print(f"packaged-assets: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(
        f"packaged-assets: ok ({checked} compile-time asset(s) across {len(packaged)} root files; "
        f"binding lock packaged among {len(binding_packaged)} binding files)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
