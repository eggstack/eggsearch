#!/usr/bin/env python3
"""Derive release/eggpack/workflow-shape.json from the producer authority files.

The static workflow shape is the identity-free seam that `eggpack ci generate`
and `eggpack ci check` consume. Hand-maintaining it would make it a second copy
of every producer fact in pack.toml, build-bindings.toml,
qualification-bindings.toml, and consumer-validators.json — exactly the drift
this subsystem exists to eliminate. Deriving it keeps one authority per fact
and keeps this file byte-stable.

Run from the repository root:

    python3 packaging/gen-release-workflow-shape.py

The rendered result is checked by `eggpack ci check`; the working tree stays
clean only when the shape on disk already matches its inputs.
"""
import json
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RELEASE = ROOT / "release" / "eggpack"
SHAPE = RELEASE / "workflow-shape.json"

# Selection/display order, deliberately not sorted: this is the order the
# operator sees and the order `eggpack ci _resolve-release` is given.
SELECTED_ALIASES = [
    "linux-x64",
    "linux-arm64",
    "linux-armv7",
    "macos-x64",
    "macos-arm64",
    "windows-x64",
    "windows-arm64",
]


def load_toml(name):
    with (RELEASE / name).open("rb") as handle:
        return tomllib.load(handle)


def main():
    pack = load_toml("pack.toml")
    build = load_toml("build-bindings.toml")
    qualification = load_toml("qualification-bindings.toml")
    with (RELEASE / "consumer-validators.json").open("rb") as handle:
        validators = json.load(handle)

    targets = []
    for row in pack["targets"]:
        triple = row["target"]
        if triple not in build["targets"]:
            raise SystemExit(f"build bindings do not cover {triple}")
        if triple not in validators:
            raise SystemExit(f"consumer validators do not cover {triple}")
        toolchain = dict(row.get("toolchain", {}))
        targets.append(
            {
                "target": triple,
                "strategy": row["strategy"],
                "host_os": row["host_os"],
                "host_arch": row["host_arch"],
                "toolchain": toolchain,
                "floor": row["floor"],
                "qualification": row["qualification"],
                "support": row["support"],
            }
        )
    targets.sort(key=lambda entry: entry["target"])

    shape = {
        "schema_version": 1,
        "targets": targets,
        "selected_aliases": list(SELECTED_ALIASES),
        "build_bindings": build,
        "qualification_bindings": qualification,
        "consumer_validators": validators,
        "staging": {
            "provider": "git_hub_draft",
            "tag_source": "dispatch_input",
            "required": True,
        },
    }

    with SHAPE.open("w", encoding="utf-8") as handle:
        json.dump(shape, handle, indent=2, sort_keys=True)
        handle.write("\n")
    print(f"wrote {SHAPE.relative_to(ROOT)} ({len(targets)} targets)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
