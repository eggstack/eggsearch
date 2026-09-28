import re
import sys
from pathlib import Path


def has_movable_workflow_uses(text):
    for line in text.splitlines():
        stripped = line.lstrip()
        if not stripped.startswith("uses:") and not re.match(r"-\s*uses\s*:", stripped):
            continue
        value = stripped.split(":", 1)[1].strip()
        if value.startswith(("'", '"')):
            quote = value[0]
            value = value[1:].split(quote, 1)[0]
        else:
            value = value.split("#", 1)[0].strip()
        if value.startswith("./"):
            continue
        if "@" not in value or not re.fullmatch(r"[0-9a-fA-F]{40}", value.rsplit("@", 1)[1]):
            return True
    return False


def main():
    cases = {
        "  - uses: actions/checkout@v4": True,
        "  - uses: actions/checkout@main": True,
        "  - uses: actions/checkout@abc123": True,
        "  - uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4": False,
        "  - uses: ./local/action": False,
        "# - uses: actions/checkout@v4": False,
        "  uses: 'org/repo/.github/workflows/reusable.yml@v1'": True,
    }
    for sample, expected in cases.items():
        if has_movable_workflow_uses(sample) != expected:
            print(f"workflow-pin-check: parser fixture failed: {sample}", file=sys.stderr)
            return 1
    files = sorted(Path(".github/workflows").glob("*.yml"))
    files += sorted(Path(".github/workflows").glob("*.yaml"))
    failures = [str(path) for path in files if has_movable_workflow_uses(path.read_text())]
    if failures:
        print("workflow-pin-check: external action refs must use full 40-character SHAs", file=sys.stderr)
        print("\n".join(failures), file=sys.stderr)
        return 1
    print(f"workflow-pin-check: ok ({len(files)} workflow files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
