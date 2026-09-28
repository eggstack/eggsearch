#!/usr/bin/env python3
import subprocess
import sys
import re


def run(*args):
    return subprocess.run(args, check=False, capture_output=True, text=True)


default = run("cargo", "tree", "--locked", "-i", "reqwest")
if default.returncode == 0 or "did not match any packages" not in default.stderr:
    print("http-dependency-shape: reqwest must be absent from the default graph", file=sys.stderr)
    sys.exit(1)

all_features = run("cargo", "tree", "--locked", "--all-features", "-i", "reqwest")
if all_features.returncode != 0:
    print(all_features.stderr, file=sys.stderr)
    sys.exit(all_features.returncode)
tree = all_features.stdout
direct = [
    re.sub(r" v[^ ]+$", "", line.strip()[4:])
    for line in tree.splitlines()[1:]
    if line.startswith(("├── ", "└── "))
]
if direct != ["chromiumoxide"]:
    print("http-dependency-shape: reqwest must remain only through chromiumoxide in all-features", file=sys.stderr)
    sys.exit(1)

print("http-dependency-shape: default graph is reqwest-free; all-features reqwest is chromiumoxide-only")
