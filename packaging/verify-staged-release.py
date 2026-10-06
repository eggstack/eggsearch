#!/usr/bin/env python3
"""Prove that an already-staged Eggpack draft release holds exactly what it claims.

This is the read-only half of Eggsearch's provenance workflow. It never mutates
release state. It verifies, for the exact tag given:

1. the draft release exists and is still unpublished;
2. its remote asset set equals the exact 19-file post-cutover inventory, with
   no missing and no unexpected names;
3. each downloaded file's local digest and size equal the digest and size GitHub
   reports for that asset;
4. all seven binary/checksum sidecar pairs verify against their own sidecar;
5. `release-manifest.json` declares the same release id, source revision, and
   target inventory as the contract, and its artifact digests match the staged
   bytes;
6. `install.sh` / `install.ps1` equal the checked-in wrapper sources at the tag;
7. the two generated exact installers are present and non-empty.

Any failure is a hard failure. This script proves identity; it never repairs.
"""
import argparse
import hashlib
import json
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONTRACT = ROOT / "release" / "eggpack" / "distribution.toml"
sys.path.insert(0, str(ROOT / "packaging"))

from expected_release_assets import (  # noqa: E402
    GENERATED_INSTALLERS,
    PRODUCT_WRAPPERS,
    inventory,
)


def fail(message):
    print(f"verify-staged-release: {message}", file=sys.stderr)
    raise SystemExit(1)


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def gh_release_json(tag, *fields):
    completed = subprocess.run(
        [
            "gh",
            "release",
            "view",
            tag,
            "--repo",
            "eggstack/eggsearch",
            "--json",
            ",".join(fields),
        ],
        capture_output=True,
        text=True,
        timeout=120,
    )
    if completed.returncode != 0:
        fail(f"could not read the GitHub Release for {tag}")
    return json.loads(completed.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--staged-dir", required=True)
    args = parser.parse_args()

    staged = Path(args.staged_dir)
    if not staged.is_dir():
        fail(f"staged directory {staged} does not exist")
    tag = args.tag

    remote = gh_release_json(tag, "isDraft", "tagName", "targetCommitish", "assets")
    if not remote.get("isDraft"):
        fail(f"release {tag} is not a draft; refusing to attest published bytes")
    if remote.get("tagName") != tag:
        fail(f"release tag mismatch: requested {tag}, GitHub reports {remote.get('tagName')}")

    remote_assets = {asset["name"]: asset for asset in remote.get("assets", [])}
    expected = inventory()
    expected_set = set(expected)
    missing = sorted(expected_set - set(remote_assets))
    unexpected = sorted(set(remote_assets) - expected_set)
    if missing or unexpected:
        if missing:
            fail(f"staged release is missing assets: {', '.join(missing)}")
        fail(f"staged release has unexpected assets: {', '.join(unexpected)}")

    # Local bytes must equal what GitHub says it holds, for both digest and size.
    for name in expected:
        path = staged / name
        if not path.is_file():
            fail(f"downloaded asset is missing: {name}")
        local_digest = sha256(path)
        local_size = path.stat().st_size
        reported = remote_assets[name]
        remote_digest = (reported.get("digest") or "").split(":")[-1]
        if not remote_digest:
            fail(f"GitHub reported no digest for asset {name}")
        if remote_digest != local_digest:
            fail(f"digest mismatch for {name}")
        if reported.get("size") != local_size:
            fail(f"size mismatch for {name}: remote {reported.get('size')}, local {local_size}")

    with CONTRACT.open("rb") as handle:
        contract = tomllib.load(handle)
    contract_assets = {}
    for entry in contract["targets"]:
        triple = entry["triple"]
        asset = (
            entry["asset"]["asset"]
            .replace("{product}", "eggsearch")
            .replace("{target}", triple)
        )
        contract_assets[triple] = asset

    # Seven binary/checksum pairs must self-verify.
    for triple, asset in contract_assets.items():
        binary = staged / asset
        sidecar = staged / f"{asset}.sha256"
        if not binary.is_file() or not sidecar.is_file():
            fail(f"incomplete binary/checksum pair for {triple}")
        recorded = sidecar.read_text(encoding="utf-8").split()
        if len(recorded) != 2 or recorded[1].lstrip("*") != asset:
            fail(f"checksum sidecar for {triple} does not name {asset}")
        if recorded[0] != sha256(binary):
            fail(f"checksum sidecar for {triple} does not match the staged binary")

    # The manifest is Eggpack's final-bytes evidence: it must describe this
    # release, this source, and exactly the staged artifact digests.
    manifest_path = staged / "release-manifest.json"
    if not manifest_path.is_file():
        fail("release-manifest.json is missing from the staged release")
    with manifest_path.open("r", encoding="utf-8") as handle:
        manifest = json.load(handle)
    if manifest.get("schema_version") != 1:
        fail("release-manifest.json has an unexpected schema_version")
    if manifest.get("product_id") != "eggsearch":
        fail(f"release-manifest.json declares product_id {manifest.get('product_id')!r}")
    if manifest.get("release_id") != tag:
        fail(
            f"release-manifest.json declares release_id {manifest.get('release_id')!r}, "
            f"expected {tag!r}"
        )
    head = subprocess.run(
        ["git", "-C", str(ROOT), "rev-list", "-n", "1", tag],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if head.returncode != 0:
        fail(f"could not resolve tag {tag} locally")
    source_revision = head.stdout.strip()
    if manifest.get("source_revision") != source_revision:
        fail(
            "release-manifest.json declares source_revision "
            f"{manifest.get('source_revision')!r}, tag {tag} points at {source_revision!r}"
        )
    manifest_targets = {}
    for record in manifest.get("targets", []):
        manifest_targets[record.get("target")] = record
    if set(manifest_targets) != set(contract_assets):
        fail("release-manifest.json target inventory differs from the distribution contract")
    for triple, asset in contract_assets.items():
        form = manifest_targets[triple].get("form", {})
        if form.get("kind") != "direct":
            fail(f"release-manifest.json records {triple} as {form.get('kind')!r}, expected direct")
        artifact = form.get("artifact", {})
        if artifact.get("name") != asset:
            fail(
                f"release-manifest.json names {triple} artifact {artifact.get('name')!r}, "
                f"contract says {asset!r}"
            )
        if artifact.get("sha256") != sha256(staged / asset):
            fail(f"release-manifest.json digest for {triple} does not match the staged bytes")
        if artifact.get("size") != (staged / asset).stat().st_size:
            fail(f"release-manifest.json size for {triple} does not match the staged bytes")

    # Public wrapper bytes must equal the checked-in wrapper sources.
    for name in PRODUCT_WRAPPERS:
        if (staged / name).read_bytes() != (ROOT / "packaging" / name).read_bytes():
            fail(f"staged {name} differs from the checked-in wrapper source")

    for name in GENERATED_INSTALLERS:
        path = staged / name
        if not path.is_file() or path.stat().st_size == 0:
            fail(f"generated exact installer is missing or empty: {name}")

    print(
        f"verify-staged-release: ok ({tag}, {len(expected)} assets, draft-only, "
        "digests/sizes/sidecars/manifest/wrappers verified)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())