#!/usr/bin/env python3
"""Required product-owned consumer validator for the natively-hosted targets.

Core qualification already runs a bounded `--version` smoke per target, so this
validator adds the product-level guarantees a release should not ship without:

1. the candidate executes and identifies itself as the exact workspace version;
2. `--help` exits zero;
3. on GNU Linux targets, the GLIBC symbol ceiling stays at or below the
   documented 2.17 floor compatibility promise.

The GLIBC check is the local half of a two-part proof: the Zig/cargo-zigbuild
pair in the producer config pins the floor, and this validator proves the exact
staged bytes honour it. It is a no-op on non-ELF candidates (macOS, Windows),
where there is no GLIBC surface to prove.

Contract: invoked with exactly one argument, the absolute path to the exact
Eggpack-qualified candidate binary.
"""
import os
import re
import shutil
import subprocess
import sys

GLIBC_CEILING = (2, 17)
TIMEOUT_SECS = 120
MAX_OUTPUT = 256 * 1024


def fail(message):
    print(f"release-validate: {message}", file=sys.stderr)
    raise SystemExit(1)


def run(command, **kwargs):
    try:
        return subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_SECS,
            **kwargs,
        )
    except subprocess.TimeoutExpired:
        fail(f"command exceeded the {TIMEOUT_SECS}s bound: {command[0]}")
    except OSError as error:
        fail(f"could not execute {command[0]}: {error.strerror}")


def is_elf(path):
    with open(path, "rb") as handle:
        return handle.read(4) == b"\x7fELF"


def glibc_ceiling(path):
    readelf = shutil.which("readelf")
    if readelf is None:
        fail("readelf is required for the glibc ceiling proof and is unavailable")
    completed = run([readelf, "--version-info", path])
    if completed.returncode != 0:
        fail("readelf --version-info failed on the candidate")
    ceiling = (0, 0)
    for major, minor in re.findall(r"GLIBC_(\d+)\.(\d+)", completed.stdout):
        ceiling = max(ceiling, (int(major), int(minor)))
    return ceiling


def workspace_version(root):
    cargo = os.path.join(root, "Cargo.toml")
    if not os.path.isfile(cargo):
        return None
    with open(cargo, "r", encoding="utf-8") as handle:
        for line in handle:
            match = re.match(r'^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+[^"]*)"', line)
            if match:
                return match.group(1)
    return None


def main():
    if len(sys.argv) != 2:
        fail("usage: validate-release-binary.py <candidate-path>")
    candidate = sys.argv[1]
    if not os.path.isfile(candidate):
        fail("candidate path is not a file")

    version = run([candidate, "--version"], check=False)
    if version.returncode != 0:
        fail("candidate could not execute --version")
    lines = version.stdout.strip().splitlines()
    identity = lines[0].strip() if lines else ""
    if not identity.startswith("eggsearch "):
        fail(f"candidate did not identify as eggsearch: {identity!r}")

    expected = workspace_version(os.getcwd())
    if expected is not None and identity != f"eggsearch {expected}":
        fail(f"candidate reports {identity!r}, workspace version is {expected!r}")

    helped = run([candidate, "--help"], check=False)
    if helped.returncode != 0:
        fail("candidate --help exited non-zero")

    if is_elf(candidate):
        ceiling = glibc_ceiling(candidate)
        if ceiling > GLIBC_CEILING:
            fail(f"candidate requires glibc {ceiling[0]}.{ceiling[1]}, above the 2.17 ceiling")
        print(f"release-validate: ok ({identity}, glibc <= {ceiling[0]}.{ceiling[1]})")
    else:
        print(f"release-validate: ok ({identity}, no GLIBC surface to prove)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())