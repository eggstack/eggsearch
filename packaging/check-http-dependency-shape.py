#!/usr/bin/env python3
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def run(*args):
    return subprocess.run(args, cwd=ROOT, check=False, capture_output=True, text=True)


def reverse_roots(manifest, package, *args):
    result = run(
        "cargo",
        "tree",
        "--manifest-path",
        str(manifest),
        "--color",
        "never",
        "--locked",
        *args,
        "-i",
        package,
    )
    if result.returncode != 0:
        if "did not match any packages" in result.stderr:
            return []
        print(result.stderr, file=sys.stderr)
        sys.exit(result.returncode)
    return [
        re.sub(r" v[^ ]+$", "", line.strip()[4:])
        for line in result.stdout.splitlines()[1:]
        if line.startswith(("├── ", "└── "))
    ]


def check_reqwest(label, manifest, *args):
    roots = reverse_roots(manifest, "reqwest", *args)
    if roots and roots != ["chromiumoxide"]:
        print(
            f"http-dependency-shape: {label} graph may carry reqwest only through "
            f"chromiumoxide, found {roots}",
            file=sys.stderr,
        )
        sys.exit(1)
    return roots


def graph_nodes(manifest):
    result = run(
        "cargo",
        "tree",
        "--manifest-path",
        str(manifest),
        "--color",
        "never",
        "--locked",
        "--target",
        "all",
        "--prefix",
        "none",
    )
    if result.returncode != 0:
        print(result.stderr, file=sys.stderr)
        sys.exit(result.returncode)
    return {line.strip().split(" ", 1)[0] for line in result.stdout.splitlines() if line.strip()}


def check_binding_graph(manifest):
    reqwest_roots = check_reqwest("bindings/python", manifest, "--target", "all")
    nodes = graph_nodes(manifest)
    forbidden = sorted(
        nodes
        & {"openssl", "openssl-probe", "openssl-sys", "native-tls", "native-tls-alpn"}
    )
    if forbidden:
        print(
            f"http-dependency-shape: bindings/python graph contains forbidden TLS nodes {forbidden}",
            file=sys.stderr,
        )
        sys.exit(1)
    return reqwest_roots, forbidden


root_manifest = ROOT / "Cargo.toml"
binding_manifest = ROOT / "bindings/python/Cargo.toml"
default_roots = check_reqwest("default", root_manifest)
all_features_roots = check_reqwest("all-features", root_manifest, "--all-features")
binding_roots, binding_forbidden = check_binding_graph(binding_manifest)

print(
    "http-dependency-shape: reqwest "
    f"default={default_roots or 'absent'} "
    f"all-features={all_features_roots or 'absent'} "
    f"binding={binding_roots or 'absent'} forbidden-binding-tls={binding_forbidden or 'absent'}"
)
