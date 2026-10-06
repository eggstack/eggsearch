#!/usr/bin/env python3
"""Print the exact post-cutover public release inventory, one name per line.

The inventory is derived from the Eggpack distribution contract rather than
hardcoded, so a target or rename cannot leave the guards describing a set that
is no longer produced. `--from-contract` yields the 19-file public inventory:

    7 versionless executables
    7 `.sha256` sidecars
    release-manifest.json
    install.sh
    install.ps1
    install-exact.sh
    install-exact.ps1

The last three are additive producer evidence from Eggpack's `ProductWrappers`
staging; the other 16 are the historical names, unchanged.
"""
import argparse
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONTRACT = ROOT / "release" / "eggpack" / "distribution.toml"

# Published by Eggpack `ProductWrappers` staging beside the product wrappers.
PRODUCER_EVIDENCE = ("release-manifest.json",)
GENERATED_INSTALLERS = ("install-exact.sh", "install-exact.ps1")
PRODUCT_WRAPPERS = ("install.sh", "install.ps1")

EXPECTED_COUNT = 19


def contract_assets():
    with CONTRACT.open("rb") as handle:
        contract = tomllib.load(handle)
    product = contract["product"]["id"]
    assets = []
    for entry in contract["targets"]:
        triple = entry["triple"]
        if entry["asset"].get("kind") != "direct":
            raise SystemExit(f"expected a direct asset form for {triple}")
        asset = entry["asset"]["asset"].replace("{product}", product).replace("{target}", triple)
        sidecar = entry["checksum"]["sidecar"].replace("{asset}", asset)
        assets.append(asset)
        assets.append(sidecar)
    return assets


def inventory():
    assets = contract_assets()
    assets.extend(PRODUCER_EVIDENCE)
    assets.extend(PRODUCT_WRAPPERS)
    assets.extend(GENERATED_INSTALLERS)
    if len(assets) != EXPECTED_COUNT:
        raise SystemExit(
            f"derived inventory has {len(assets)} names, expected {EXPECTED_COUNT}"
        )
    if len(set(assets)) != EXPECTED_COUNT:
        raise SystemExit("derived inventory contains duplicate names")
    return sorted(assets)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--from-contract",
        action="store_true",
        help="derive the inventory from the Eggpack distribution contract (default)",
    )
    parser.add_argument("--count", action="store_true", help="print only the count")
    args = parser.parse_args()
    names = inventory()
    if args.count:
        print(len(names))
        return 0
    print("\n".join(names))
    return 0


if __name__ == "__main__":
    sys.exit(main())