#!/usr/bin/env python3
import sys
from pathlib import Path


def read(name):
    return Path(name).read_text(encoding="utf-8")


def fail(msg):
    print(f"planning-consistency: {msg}", file=sys.stderr)
    return 1


def find_row(text, marker):
    for line in text.splitlines():
        if marker in line and line.strip().startswith("|"):
            return line
    return None


def main():
    root = Path(__file__).resolve().parent.parent
    roadmap = read(root / "plans/subsystems/repository-hardening-roadmap.md")
    registry = read(root / "plans/registry.md")
    overview = read(root / "plans/implementation/repository-hardening/000-overview-and-sequencing.md")
    errors = []
    if "| M001" not in roadmap or "| M007" not in roadmap:
        errors.append("roadmap milestone status table must contain M001 through M007")
    if "| M001" not in overview or "| M007" not in overview:
        errors.append("overview milestone table must contain M001 through M007")
    for name, text in [("roadmap", roadmap), ("overview", overview)]:
        for line in text.splitlines():
            low = line.lower()
            if "m001-m005" in low and "closed" in low:
                errors.append(f"{name} must not claim M001-M005 closed while members are conditional or blocked: {line.strip()[:160]}")
    for line in registry.splitlines():
        low = line.lower()
        if "hardening" in low and "m001-m005" in low and "closed" in low:
            errors.append(f"registry hardening row must not claim M001-M005 closed: {line.strip()[:160]}")
    expectations = [
        ("M001", "conditionally"),
        ("M002", "conditionally"),
        ("M003", "closed"),
        ("M004", "closed"),
        ("M005", "closed"),
        ("M006", "blocked"),
        ("M007", "conditionally closed"),
    ]
    for mid, want in expectations:
        marker = f"| {mid}"
        rrow = find_row(roadmap, marker)
        orow = find_row(overview, marker)
        if rrow is None:
            errors.append(f"roadmap missing row {mid}")
        elif want not in rrow.lower():
            errors.append(f"roadmap {mid} row must contain '{want}': {rrow.strip()[:160]}")
        if orow is None:
            errors.append(f"overview missing row {mid}")
        elif want not in orow.lower():
            errors.append(f"overview {mid} row must contain '{want}': {orow.strip()[:160]}")
    r_m001 = find_row(roadmap, "| M001") or ""
    r_m004 = find_row(roadmap, "| M004") or ""
    if "m007" not in r_m001.lower():
        errors.append("roadmap M001 must reference M007 corrective evidence")
    if "m007" not in r_m004.lower():
        errors.append("roadmap M004 must reference M007 corrective ratchet")
    hardening_lines = [l for l in registry.splitlines() if "hardening" in l.lower()]
    hardening_blob = "\n".join(hardening_lines).lower()
    if "m007" not in hardening_blob or "conditionally" not in hardening_blob or "blocked" not in hardening_blob:
        errors.append("registry hardening entries must mention M007, conditional status, and blocked M006")
    m007_impl = None
    for line in registry.splitlines():
        if "007-corrective-closure-evidence-and-process-ratchet" in line:
            m007_impl = line
            break
    if m007_impl is None:
        errors.append("registry must link M007 implementation plan")
    elif "conditionally closed" not in m007_impl.lower():
        errors.append(f"registry M007 row must be conditionally closed: {m007_impl.strip()[:200]}")
    if errors:
        for e in errors:
            print(f"planning-consistency: {e}", file=sys.stderr)
        return 1
    print("planning-consistency: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
