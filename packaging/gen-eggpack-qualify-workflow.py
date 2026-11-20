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


def insert_before_job_step(text, job_name, anchor, inserted, label):
    jobs, matches = job_blocks(text)
    start = jobs[job_name]
    end = next((job_start for _, job_start in matches if job_start > start), len(text))
    block = replace_once(text[start:end], anchor, inserted + anchor, label)
    return text[:start] + block + text[end:]


def footprint_steps(target, platform, artifact):
    if platform == "linux":
        step = """      - name: Measure stripped binary size and startup RSS
        shell: bash
        run: |
          set -euo pipefail
          target="@TARGET@"
          build_root="$RUNNER_TEMP/eggpack/$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT"
          binary="$build_root/$target/release/eggsearch"
          test -x "$binary"
          candidate_sha="$(git rev-parse HEAD)"
          binary_bytes="$(wc -c < "$binary" | tr -d '[:space:]')"
          rss_samples=""
          for _ in 1 2 3 4 5; do
            sample="$(/usr/bin/time -f '%M' "$binary" --version 2>&1 >/dev/null)"
            [[ "$sample" =~ ^[0-9]+$ ]]
            if [[ -n "$rss_samples" ]]; then rss_samples="$rss_samples,"; fi
            rss_samples="$rss_samples$sample"
          done
          mkdir -p eggpack-footprint
          printf '{"candidate_sha":"%s","target":"%s","binary_bytes":%s,"probe":"eggsearch --version","peak_rss_kib_samples":[%s],"threshold_applied":false}\\n' "$candidate_sha" "$target" "$binary_bytes" "$rss_samples" | tee "eggpack-footprint/$target.json"
      - name: Upload footprint evidence
        uses: "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"
        with:
          name: "eggsearch-footprint-@ARTIFACT@"
          path: "eggpack-footprint/@TARGET@.json"
          if-no-files-found: error
          retention-days: 7
"""
    elif platform == "windows":
        step = """      - name: Measure stripped binary size and startup RSS
        shell: pwsh
        run: |
          $ErrorActionPreference = "Stop"
          $target = "@TARGET@"
          $buildRoot = [System.IO.Path]::Combine($env:RUNNER_TEMP, "eggpack", "$($env:GITHUB_RUN_ID)-$($env:GITHUB_RUN_ATTEMPT)")
          $binary = [System.IO.Path]::Combine($buildRoot, $target, "release", "eggsearch.exe")
          if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw "Release binary not found: $binary" }
          $candidateSha = (git rev-parse HEAD).Trim()
          $binaryBytes = [long](Get-Item -LiteralPath $binary).Length
          $rssSamples = @()
          for ($attempt = 0; $attempt -lt 10 -and $rssSamples.Count -lt 5; $attempt++) {
            $process = [System.Diagnostics.Process]::new()
            $process.StartInfo.FileName = $binary
            $process.StartInfo.Arguments = "--version"
            $process.StartInfo.UseShellExecute = $false
            $process.StartInfo.CreateNoWindow = $true
            $process.StartInfo.RedirectStandardOutput = $true
            $process.StartInfo.RedirectStandardError = $true
            if (-not $process.Start()) { throw "Could not start release binary" }
            $stdoutTask = $process.StandardOutput.ReadToEndAsync()
            $stderrTask = $process.StandardError.ReadToEndAsync()
            $rssSample = 0L
            do {
              $process.Refresh()
              if ($process.HasExited) { break }
              $currentSample = [long]$process.PeakWorkingSet64
              $process.Refresh()
              if (-not $process.HasExited -and $currentSample -gt $rssSample) { $rssSample = $currentSample }
              if ($process.WaitForExit(1)) { break }
            } while ($true)
            $process.WaitForExit()
            $stdout = $stdoutTask.GetAwaiter().GetResult()
            $stderr = $stderrTask.GetAwaiter().GetResult()
            if ($process.ExitCode -ne 0) { throw "--version exited with $($process.ExitCode): $stderr" }
            if ($rssSample -gt 0) { $rssSamples += $rssSample }
            $process.Dispose()
          }
          if ($rssSamples.Count -ne 5) { throw "Collected $($rssSamples.Count) valid peak working set samples; expected 5" }
          $record = [ordered]@{ candidate_sha = $candidateSha; target = $target; binary_bytes = $binaryBytes; probe = "eggsearch --version"; peak_working_set_bytes_samples = $rssSamples; threshold_applied = $false }
          $json = $record | ConvertTo-Json -Compress
          $outputDir = Join-Path $PWD "eggpack-footprint"
          New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
          $outputPath = Join-Path $outputDir "$target.json"
          Set-Content -LiteralPath $outputPath -Value $json -NoNewline -Encoding utf8
          Write-Output $json
      - name: Upload footprint evidence
        uses: "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"
        with:
          name: "eggsearch-footprint-@ARTIFACT@"
          path: "eggpack-footprint/@TARGET@.json"
          if-no-files-found: error
          retention-days: 7
"""
    else:
        raise SystemExit(f"unsupported footprint platform: {platform}")
    return step.replace("@TARGET@", target).replace("@ARTIFACT@", artifact)


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

    footprint_jobs = [
        ("build_aarch64_pc_windows_msvc", "aarch64-pc-windows-msvc", "windows", "windows-arm64"),
        ("build_aarch64_unknown_linux_gnu", "aarch64-unknown-linux-gnu", "linux", "linux-arm64"),
        ("build_x86_64_pc_windows_msvc", "x86_64-pc-windows-msvc", "windows", "windows-x64"),
        ("build_x86_64_unknown_linux_gnu", "x86_64-unknown-linux-gnu", "linux", "linux-x64"),
    ]
    anchor = "      - name: Capture canonical build handoff\n"
    for job_name, target, platform, artifact in footprint_jobs:
        output = insert_before_job_step(
            output,
            job_name,
            anchor,
            footprint_steps(target, platform, artifact),
            f"{target} footprint measurement",
        )

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
