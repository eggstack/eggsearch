import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PRODUCER = ROOT / ".github" / "workflows" / "release-eggpack.yml"
QUALIFIER = ROOT / ".github" / "workflows" / "qualify-eggpack-candidate.yml"
GENERATOR = ROOT / "packaging" / "gen-eggpack-qualify-workflow.py"


def fail(message):
    raise SystemExit(f"eggpack qualifier contract: {message}")


def workflow_jobs(text):
    section = text.split("jobs:\n", 1)[1]
    return re.findall(r"(?m)^  ([a-z][a-z0-9_]*):$", section)


def main():
    result = subprocess.run(
        [sys.executable, str(GENERATOR), "--check"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode:
        fail(result.stderr.strip() or result.stdout.strip())

    producer = PRODUCER.read_text(encoding="utf-8")
    qualifier = QUALIFIER.read_text(encoding="utf-8")
    source_jobs = workflow_jobs(producer)
    qualifier_jobs = workflow_jobs(qualifier)
    if not source_jobs or source_jobs[-1] != "stage":
        fail("release producer must retain its final stage job")
    if qualifier_jobs != source_jobs[:-1]:
        fail("qualifier jobs differ from producer jobs with only stage removed")
    if "'on':\n  workflow_dispatch:" not in qualifier:
        fail("qualifier must be workflow_dispatch-only")
    if re.search(r"(?m)^\s+(?:push|pull_request):\s*$", qualifier):
        fail("qualifier must not add an automatic trigger")

    with (ROOT / "release/eggpack/distribution.toml").open("rb") as handle:
        contract = tomllib.load(handle)
    for target in contract["targets"]:
        if target["triple"] not in qualifier:
            fail(f"missing contracted target {target['triple']}")

    for forbidden in (
        "contents: write",
        "_stage-github-draft",
        "_prepare-stage",
        "secrets.",
        "GITHUB_TOKEN",
        "gh release",
        "cargo publish",
        "git tag ",
        "api.github.com/repos/eggstack/eggsearch/releases",
        "release_tag",
        "github.ref",
    ):
        if forbidden in qualifier:
            fail(f"qualifier contains forbidden release path {forbidden!r}")

    required = (
        "candidate_sha:",
        "CANDIDATE_SHA: ${{ inputs.candidate_sha }}",
        "^[0-9a-f]{40}$",
        "candidate_sha must be exactly 40 lowercase hexadecimal characters",
        'git rev-parse --verify HEAD^{commit}',
        '[[ \"$head_sha\" != \"$CANDIDATE_SHA\" ]]',
        'tomllib.load(open(\"Cargo.toml\", \"rb\"))[\"package\"][\"version\"]',
        "--tag' \"$candidate_release_id\"",
        'name: "eggpack-candidate-qualified"',
        "retention-days: 7",
        "required_gate:",
        "aggregate:",
    )
    for marker in required:
        if marker not in qualifier:
            fail(f"qualifier is missing required contract marker {marker!r}")
    if qualifier.count('ref: "${{ inputs.candidate_sha }}"') != len(qualifier_jobs):
        fail("every job must check out the exact candidate SHA")
    if qualifier.count("permissions:\n      contents: read") != len(qualifier_jobs):
        fail("every job must retain read-only contents permission")
    if qualifier.count("permissions:\n  contents: read") != 1:
        fail("workflow-level permission must remain contents: read only")
    print(f"eggpack qualifier contract: ok ({len(qualifier_jobs)} producer jobs, 7 targets)")


if __name__ == "__main__":
    main()
