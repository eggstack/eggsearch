#!/usr/bin/env python3
"""Render the non-publishing exact-SHA qualifier from Eggpack's release workflow."""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / ".github" / "workflows" / "release-eggpack.yml"
OUTPUT = ROOT / ".github" / "workflows" / "qualify-eggpack-candidate.yml"


def replace_once(text, old, new, label):
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one {label}, found {count}")
    return text.replace(old, new, 1)


def replace_count(text, old, new, expected, label):
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"expected {expected} {label}, found {count}")
    return text.replace(old, new)


def job_blocks(text):
    section_start = text.index("jobs:\n") + len("jobs:\n")
    section = text[section_start:]
    matches = list(re.finditer(r"(?m)^  ([a-z][a-z0-9_]*):\n", section))
    return {
        match.group(1): section_start + match.start() for match in matches
    }, [(match.group(1), section_start + match.start()) for match in matches]


def render(source):
    jobs, matches = job_blocks(source)
    expected = {
        "preflight",
        "resolve",
        "build_aarch64_apple_darwin",
        "build_aarch64_pc_windows_msvc",
        "build_aarch64_unknown_linux_gnu",
        "build_armv7_unknown_linux_gnueabihf",
        "build_x86_64_apple_darwin",
        "build_x86_64_pc_windows_msvc",
        "build_x86_64_unknown_linux_gnu",
        "qualify_build_aarch64_apple_darwin",
        "qualify_build_aarch64_pc_windows_msvc",
        "qualify_build_aarch64_unknown_linux_gnu",
        "qualify_build_armv7_unknown_linux_gnueabihf",
        "qualify_build_x86_64_apple_darwin",
        "qualify_build_x86_64_pc_windows_msvc",
        "qualify_build_x86_64_unknown_linux_gnu",
        "validate_build_aarch64_apple_darwin",
        "validate_build_aarch64_pc_windows_msvc",
        "validate_build_aarch64_unknown_linux_gnu",
        "validate_build_armv7_unknown_linux_gnueabihf",
        "validate_build_x86_64_apple_darwin",
        "validate_build_x86_64_pc_windows_msvc",
        "validate_build_x86_64_unknown_linux_gnu",
        "required_gate",
        "aggregate",
        "stage",
    }
    if set(jobs) != expected or not matches or matches[-1][0] != "stage":
        raise SystemExit("Eggpack producer job shape changed; qualifier generation stopped")

    output = replace_once(source, "name: Eggpack candidate builds", "name: Eggpack exact-SHA candidate qualification", "workflow name")
    output = replace_once(output, "      release_tag:\n        description: Exact existing tag to stage\n", "      candidate_sha:\n        description: Exact 40-character commit SHA to qualify without release writes\n", "dispatch input")
    output = replace_once(
        output,
        "  group: eggpack-${{ github.workflow }}-${{ github.event_name == 'workflow_dispatch' && inputs.release_tag || github.ref }}",
        "  group: eggpack-${{ github.workflow }}-${{ inputs.candidate_sha }}",
        "concurrency candidate identity",
    )
    checkout_ref = 'ref: "${{ github.event_name == \'workflow_dispatch\' && inputs.release_tag || github.ref }}"'
    output = replace_count(
        output,
        checkout_ref,
        'ref: "${{ inputs.candidate_sha }}"',
        len(expected) - 1,
        "exact-SHA checkout refs",
    )

    resolve_start = output.index("  resolve:\n")
    jobs, matches = job_blocks(output)
    resolve_end = next((start for name, start in matches if start > resolve_start), -1)
    if resolve_end < 0:
        raise SystemExit("resolve job boundary was not found")
    resolve_block = output[resolve_start:resolve_end]
    validation_step = (
        "      - name: Validate exact-tag source\n"
        "        shell: bash\n"
        "        run: \"test -n \\\"${{ inputs.release_tag }}\\\"\"\n"
    )
    resolve_block = replace_once(resolve_block, validation_step, "", "exact-tag validation step")
    resolve_block = replace_once(
        resolve_block,
        "      - name: Resolve runtime release identity\n        shell: bash\n        run: |\n",
        "      - name: Resolve runtime release identity\n        shell: bash\n        env:\n          CANDIDATE_SHA: ${{ inputs.candidate_sha }}\n        run: |\n",
        "runtime identity step",
    )
    resolve_block = replace_once(
        resolve_block,
        '          head_sha="$(git rev-parse --verify HEAD^{commit})"\n',
        '          if [[ ! "$CANDIDATE_SHA" =~ ^[0-9a-f]{40}$ ]]; then echo \'candidate_sha must be exactly 40 lowercase hexadecimal characters\' >&2; exit 1; fi\n'
        '          head_sha="$(git rev-parse --verify HEAD^{commit})"\n'
        '          if [[ "$head_sha" != "$CANDIDATE_SHA" ]]; then echo \'checked-out commit does not match candidate_sha\' >&2; exit 1; fi\n'
        '          candidate_version="$(python3 -c \'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])\')"\n'
        '          if [[ ! "$candidate_version" =~ ^[0-9]+\\.[0-9]+\\.[0-9]+$ ]]; then echo \'Cargo.toml package version must be a three-part numeric version\' >&2; exit 1; fi\n'
        '          candidate_release_id="v${candidate_version}"\n',
        "exact SHA and package identity guard",
    )
    resolve_block = replace_once(
        resolve_block,
        "'--tag' '${{ inputs.release_tag }}'",
        "'--tag' \"$candidate_release_id\"",
        "synthetic candidate release identity",
    )
    output = output[:resolve_start] + resolve_block + output[resolve_end:]

    jobs, matches = job_blocks(output)
    stage_start = jobs["stage"]
    output = output[:stage_start].rstrip() + "\n"
    output = replace_once(output, 'name: "eggpack-finalized-release"', 'name: "eggpack-candidate-qualified"', "candidate artifact name")
    output = output.replace("release_tag", "candidate_sha")
    output = output.replace("Eggpack candidate builds", "Eggpack exact-SHA candidate qualification")

    windows_start = output.index("  validate_build_x86_64_pc_windows_msvc:\n")
    _, job_matches = job_blocks(output)
    windows_end = next(
        (start for _, start in job_matches if start > windows_start), len(output)
    )
    windows_block = output[windows_start:windows_end]
    wrapper_steps = """\
      - name: Run Windows PowerShell installer and release-smoke wrappers
        shell: pwsh
        env:
          EGGSEARCH_CANDIDATE_SHA: ${{ inputs.candidate_sha }}
        run: |
          $ErrorActionPreference = 'Stop'
          $SourceRevision = $env:EGGSEARCH_CANDIDATE_SHA
          $CandidateDirectory = 'eggpack-handoff/build_x86_64_pc_windows_msvc/candidates'
          $CandidatePath = Join-Path $CandidateDirectory 'candidate-direct'
          $HandoffPath = 'eggpack-handoff/build_x86_64_pc_windows_msvc/build-handoff.json'
          $QualificationPath = 'eggpack-handoff/build_x86_64_pc_windows_msvc/evidence.json'
          $Handoff = Get-Content -Raw -LiteralPath $HandoffPath | ConvertFrom-Json
          $Qualification = Get-Content -Raw -LiteralPath $QualificationPath | ConvertFrom-Json
          if ($Handoff.source_revision -ne $SourceRevision -or $Handoff.target -ne 'x86_64-pc-windows-msvc') { throw 'Candidate handoff identity mismatch' }
          if ($Qualification.source_revision -ne $SourceRevision -or $Qualification.target -ne $Handoff.target) { throw 'Qualification evidence identity mismatch' }
          if ($Handoff.release_id -notmatch '^v(?<version>[0-9]+[.][0-9]+[.][0-9]+)$') { throw 'Candidate release version is invalid' }
          $CandidateVersion = $Matches.version
          if (-not (Test-Path -LiteralPath $CandidatePath -PathType Leaf)) { throw 'Qualified Windows candidate is missing' }
          $BinaryPath = Join-Path $env:RUNNER_TEMP 'eggsearch-x86_64-pc-windows-msvc.exe'
          Copy-Item -LiteralPath $CandidatePath -Destination $BinaryPath -Force
          $BinaryHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $BinaryPath).Hash.ToLowerInvariant()
          $DirectCandidates = @($Qualification.candidates | Where-Object { $_.selector.kind -eq 'direct' })
          if ($DirectCandidates.Count -ne 1 -or $BinaryHash -ne ([string]$DirectCandidates[0].sha256).ToLowerInvariant()) { throw 'Windows wrapper binary does not match qualified candidate' }
          foreach ($RelativePath in @('packaging/install.ps1', 'packaging/test-install.ps1', 'packaging/release-smoke.ps1')) {
            $Tokens = $null
            $ParseErrors = $null
            $ScriptPath = (Resolve-Path -LiteralPath $RelativePath).Path
            [System.Management.Automation.Language.Parser]::ParseFile($ScriptPath, [ref]$Tokens, [ref]$ParseErrors) | Out-Null
            if ($ParseErrors.Count -gt 0) { $ParseErrors | ForEach-Object { Write-Error $_.Message }; throw ('PowerShell parse failed: ' + $RelativePath) }
          }
          & './packaging/test-install.ps1'
          & './packaging/release-smoke.ps1' $BinaryPath $CandidateVersion
          $EvidenceDirectory = 'eggpack-wrapper-evidence/build_x86_64_pc_windows_msvc'
          New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
          $WrapperEvidence = [ordered]@{ schema_version = 1; source_revision = $SourceRevision; target = $Handoff.target; release_id = $Handoff.release_id; binary_sha256 = $BinaryHash; powershell_version = $PSVersionTable.PSVersion.ToString(); test_install = 'passed'; release_smoke = 'passed' }
          $WrapperEvidence | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'wrapper-evidence.json') -Encoding utf8NoBOM
          Write-Output 'PowerShell installer wrapper passed'
          Write-Output 'PowerShell release-smoke wrapper passed'
      - name: Upload Windows wrapper evidence
        uses: "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"
        with:
          name: "eggpack-windows-wrapper-evidence-${{ inputs.candidate_sha }}"
          path: "./eggpack-wrapper-evidence/build_x86_64_pc_windows_msvc/wrapper-evidence.json"
          if-no-files-found: error
          retention-days: 7
"""
    windows_block = replace_once(
        windows_block,
        "      - name: Upload consumer validation evidence\n",
        wrapper_steps + "      - name: Upload consumer validation evidence\n",
        "Windows PowerShell wrapper qualification",
    )
    output = output[:windows_start] + windows_block + output[windows_end:]

    if "contents: write" in output or "_stage-github-draft" in output:
        raise SystemExit("generated qualifier contains a release-writing path")
    if "secrets." in output or "gh release" in output or "cargo publish" in output or "git tag " in output:
        raise SystemExit("generated qualifier contains a secret or publication command")
    if "github.ref" in output or "release_tag" in output:
        raise SystemExit("generated qualifier retains a tag or implicit-ref path")
    if not all(name in output for name in expected - {"stage"}):
        raise SystemExit("generated qualifier omitted an expected producer job")
    return output


def main():
    rendered = render(SOURCE.read_text(encoding="utf-8"))
    if "--check" in sys.argv[1:]:
        if not OUTPUT.exists() or OUTPUT.read_text(encoding="utf-8") != rendered:
            raise SystemExit("candidate qualifier workflow is out of date; run the generator")
        print("candidate qualifier workflow: up to date")
        return
    OUTPUT.write_text(rendered, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
