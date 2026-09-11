$ErrorActionPreference = 'Stop'

$Root = Split-Path -Parent $PSScriptRoot
$Installer = Get-Content -Raw (Join-Path $PSScriptRoot 'install.ps1')
$TargetContract = Get-Content (Join-Path $PSScriptRoot 'release-targets.txt') | Where-Object { $_.Trim() }

foreach ($Row in $TargetContract) {
    $Fields = $Row -split '\|'
    if ($Fields.Count -ne 4) { throw "Invalid release target row: $Row" }
    if ($Fields[2] -ne 'windows') { continue }
    $Pair = "`$Target = '$($Fields[0])'`n        `$Asset = '$($Fields[1])'"
    if ($Installer -notlike "*$Pair*") { throw "PowerShell installer mapping is missing: $Row" }
}

if ($Installer -notlike '*EGGSEARCH_INSTALL_TEST_BASE_URL*') { throw 'PowerShell test base URL hook is missing' }
if ($Installer -notlike '*releases/download/v$Version*') { throw 'PowerShell pinned release URL is missing' }
if ($Installer -notlike '*releases/latest/download*') { throw 'PowerShell latest release URL is missing' }
if ($Installer -notlike '*StatusCode -eq 404*') { throw 'PowerShell binary 404 fallback is missing' }
if ($Installer -notlike '*StatusCode*else*throw*') { throw 'PowerShell non-404 download failure is not fail-closed' }
if ($Installer -notlike '*Get-FileHash -Algorithm SHA256*') { throw 'PowerShell checksum verification is missing' }
if ($Installer -notlike '*Candidate version mismatch*') { throw 'PowerShell candidate version verification is missing' }
if ($Installer -match '(?im)\bsudo\b') { throw 'PowerShell installer must not invoke sudo' }

Write-Output 'PowerShell installer contract checks passed'
