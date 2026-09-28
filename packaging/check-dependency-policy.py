from datetime import date
import re
import sys
import tomllib
from pathlib import Path


def main():
    manifest = tomllib.loads(Path("Cargo.toml").read_text())
    lock = tomllib.loads(Path("Cargo.lock").read_text())
    deny = tomllib.loads(Path("deny.toml").read_text())
    requirements = manifest["dependencies"]
    def parse_version(value):
        numeric = value.split("+", 1)[0].split("-", 1)[0].split(".")
        return tuple(map(int, numeric)) + (0,) * (3 - len(numeric))
    if parse_version(requirements["quick-xml"]["version"]) < (0, 41, 0):
        print("dependency-policy-check: quick-xml manifest floor is below 0.41.0", file=sys.stderr)
        return 1
    versions = {}
    for package in lock["package"]:
        versions.setdefault(package["name"], []).append(parse_version(package["version"]))
    if not versions.get("quick-xml") or any(version < (0, 41, 0) for version in versions["quick-xml"]):
        print("dependency-policy-check: lockfile contains quick-xml below 0.41.0", file=sys.stderr)
        return 1
    if any(version[:2] == (0, 23) and version < (0, 23, 45) for version in versions.get("rustls", [])):
        print("dependency-policy-check: lockfile contains rustls 0.23 below 0.23.45", file=sys.stderr)
        return 1
    if deny["graph"].get("all-features") is not True:
        print("dependency-policy-check: cargo-deny graph must include all features", file=sys.stderr)
        return 1
    if deny["advisories"].get("yanked") != "deny" or deny["sources"].get("unknown-registry") != "deny" or deny["sources"].get("unknown-git") != "deny":
        print("dependency-policy-check: advisory/source policy weakened", file=sys.stderr)
        return 1
    if deny["advisories"].get("unmaintained") != "all":
        print("dependency-policy-check: unmaintained advisories must cover the full graph", file=sys.stderr)
        return 1
    ignored = deny["advisories"].get("ignore", [])
    if any(not isinstance(item, dict) or not re.fullmatch(r"RUSTSEC-\d{4}-\d{4}", item.get("id", "")) for item in ignored):
        print("dependency-policy-check: waivers must be exact, documented advisory IDs", file=sys.stderr)
        return 1
    document = Path("docs/dependency-security.md").read_text()
    today = date.today()
    for item in ignored:
        advisory = item["id"]
        reason = item.get("reason", "")
        match = re.search(r"review by (\d{4}-\d{2}-\d{2})", reason)
        if advisory not in document or not match:
            print(f"dependency-policy-check: {advisory} needs a documented review date", file=sys.stderr)
            return 1
        review_date = date.fromisoformat(match.group(1))
        if not today < review_date <= date.fromordinal(today.toordinal() + 60):
            print(f"dependency-policy-check: {advisory} review date is expired or over 60 days away", file=sys.stderr)
            return 1
    print("dependency-policy-check: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
