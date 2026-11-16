#!/usr/bin/env python3
import subprocess
import sys
import re


def run(*args):
    return subprocess.run(args, check=False, capture_output=True, text=True)


def reverse_roots(*args):
    result = run("cargo", "tree", "--color", "never", "--locked", *args, "-i", "reqwest")
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


def check(label, *args):
    roots = reverse_roots(*args)
    if roots and roots != ["chromiumoxide"]:
        print(
            f"http-dependency-shape: {label} graph may carry reqwest only through "
            f"chromiumoxide, found {roots}",
            file=sys.stderr,
        )
        sys.exit(1)
    return roots


default_roots = check("default")
all_features_roots = check("all-features", "--all-features")

print(
    "http-dependency-shape: reqwest "
    f"default={default_roots or 'absent'} "
    f"all-features={all_features_roots or 'absent'}"
)
