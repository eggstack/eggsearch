#!/usr/bin/env python3
"""Required product-owned consumer validator for the ARMv7 release candidate.

Eggpack classifies `armv7-unknown-linux-gnueabihf` as `structural`, which means
core qualification deliberately executes nothing: there is no hosted ARMv7
builder. That classification is honest, but it would otherwise leave the ARMv7
runtime guarantee unproven, so this validator carries it instead.

It is a *required* consumer validator, which means Eggpack gates `required_gate`
and `aggregate` on it: a red result here fails the whole release exactly as a
red core qualification would.

It asserts, in order:

1. the candidate is an ELF 32-bit ARM executable (not ARM64, not x86-64);
2. its GLIBC symbol requirements do not exceed the documented 2.17 ceiling;
3. `--version` and `--help` both execute under the pinned ARMv7 runtime.

Both container images are pinned by digest, not by tag, so a retagged upstream
image cannot change what this proof actually ran. The script fails closed: if
Docker, QEMU binfmt registration, or either image is unavailable, it exits
non-zero with a named reason rather than skipping the check.

Contract: invoked with exactly one argument, the absolute path to the exact
Eggpack-qualified candidate binary.
"""
import os
import re
import shutil
import subprocess
import sys

# Pinned by digest, never by tag. `arm32v7/ubuntu:20.04` is the ARMv7 runtime
# image the predecessor hand-maintained workflow used for the same proof, and
# `tonistiigi/binfmt` registers qemu-user binfmt handlers for it.
ARMV7_RUNTIME_IMAGE = (
    "arm32v7/ubuntu@sha256:d9a6ab6baaf86907ea1d46f0cbc87547920077c9c6017721faed6eeb51e9aa7e"
)
BINFMT_IMAGE = "tonistiigi/binfmt@sha256:400a4873b838d1b89194d982c45e5fb3cda4593fbfd7e08a02e76b03b21166f0"

# The 32-bit ARM hard-float dynamic loader plus the GNU/Linux ARM library path,
# matching how the predecessor workflow executed the ARMv7 candidate.
ARM_LOADER = "/lib/ld-linux-armhf.so.3"
ARM_LIBRARY_PATH = "/lib/arm-linux-gnueabihf:/lib:/usr/lib"

# Documented compatibility ceiling. Exceeding it is a hard failure, not a
# warning: the plan forbids weakening the floor to make a build pass.
GLIBC_CEILING = (2, 17)

TIMEOUT_SECS = 540
MAX_OUTPUT = 256 * 1024


def fail(message):
    print(f"armv7-validate: {message}", file=sys.stderr)
    raise SystemExit(1)


def run(command, **kwargs):
    """Run a bounded command; never echo captured output contents."""
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


def assert_armv7_elf(candidate):
    header = b""
    with open(candidate, "rb") as handle:
        header = handle.read(64)
    if len(header) < 20 or header[:4] != b"\x7fELF":
        fail("candidate is not an ELF executable")
    if header[4] != 1:
        fail("candidate is not ELF32")
    machine = int.from_bytes(header[18:20], "little" if header[5] == 1 else "big")
    # 40 == EM_ARM. 183 == EM_AARCH64, 62 == EM_X86_64: a bitness/arch mixup in
    # the build bindings is the exact regression this check exists to catch.
    if machine != 40:
        fail(f"candidate ELF machine is {machine}, expected 40 (EM_ARM)")


def glibc_ceiling(path):
    """Highest GLIBC_<n> requirement declared by the candidate."""
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
        fail("usage: validate-armv7-candidate.py <candidate-path>")
    candidate = sys.argv[1]
    if not os.path.isfile(candidate):
        fail("candidate path is not a file")
    if not shutil.which("docker"):
        fail("docker is required for the ARMv7 runtime proof and is unavailable")

    assert_armv7_elf(candidate)

    ceiling = glibc_ceiling(candidate)
    if ceiling == (0, 0):
        fail("no GLIBC version requirements found; refusing to treat that as a pass")
    if ceiling > GLIBC_CEILING:
        fail(f"candidate requires glibc {ceiling[0]}.{ceiling[1]}, above the 2.17 ceiling")

    # Register qemu-user binfmt handlers. Failure here is fatal on purpose: a
    # runtime proof that silently degrades to a skipped check is not a proof.
    register = run(
        ["docker", "run", "--privileged", "--rm", BINFMT_IMAGE, "--install", "arm"],
        check=False,
    )
    if register.returncode != 0:
        fail("could not register qemu-user binfmt handlers for armv7")

    version = run(
        [
            "docker",
            "run",
            "--rm",
            "--platform",
            "linux/arm/v7",
            "-v",
            f"{os.path.dirname(os.path.abspath(candidate))}:/candidate:ro",
            ARMV7_RUNTIME_IMAGE,
            ARM_LOADER,
            "--library-path",
            ARM_LIBRARY_PATH,
            "/candidate/" + os.path.basename(candidate),
            "--version",
        ],
        check=False,
    )
    if version.returncode != 0:
        fail("ARMv7 runtime could not execute --version")
    identity = version.stdout.strip().splitlines()[0].strip() if version.stdout.strip() else ""
    if not identity.startswith("eggsearch "):
        fail(f"ARMv7 candidate did not identify as eggsearch: {identity!r}")

    expected = workspace_version(os.getcwd())
    if expected is not None and identity != f"eggsearch {expected}":
        fail(f"ARMv7 candidate reports {identity!r}, workspace version is {expected!r}")

    helped = run(
        [
            "docker",
            "run",
            "--rm",
            "--platform",
            "linux/arm/v7",
            "-v",
            f"{os.path.dirname(os.path.abspath(candidate))}:/candidate:ro",
            ARMV7_RUNTIME_IMAGE,
            ARM_LOADER,
            "--library-path",
            ARM_LIBRARY_PATH,
            "/candidate/" + os.path.basename(candidate),
            "--help",
        ],
        check=False,
    )
    if helped.returncode != 0:
        fail("ARMv7 runtime could not execute --help")

    print(
        f"armv7-validate: ok ({identity}, 32-bit ARM ELF, glibc <="
        f" {ceiling[0]}.{ceiling[1]}, executed under the pinned ARMv7 runtime)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())