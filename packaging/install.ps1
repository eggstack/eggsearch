[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$Version,
    [switch]$Service
)

$ErrorActionPreference = 'Stop'
$Repository = 'eggstack/eggsearch'

if ($Version -and $Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$') {
    throw "Invalid version: $Version"
}

$Architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
if ($env:PROCESSOR_ARCHITEW6432) {
    $Architecture = $env:PROCESSOR_ARCHITEW6432
}
$Target = $null
$Asset = $null
switch ($Architecture.ToUpperInvariant()) {
    'X64' {
        $Target = 'x86_64-pc-windows-msvc'
        $Asset = 'eggsearch-x86_64-pc-windows-msvc.exe'
    }
    'ARM64' {
        $Target = 'aarch64-pc-windows-msvc'
        $Asset = 'eggsearch-aarch64-pc-windows-msvc.exe'
    }
}

$Principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
$IsAdministrator = $Principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if ($IsAdministrator) {
    $InstallDirectory = Join-Path $env:ProgramFiles 'Eggsearch'
} else {
    $InstallDirectory = Join-Path $env:LOCALAPPDATA 'Eggsearch'
}
$Destination = Join-Path $InstallDirectory 'eggsearch.exe'
$TempDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("eggsearch-install-{0}" -f ([guid]::NewGuid().ToString('N')))
New-Item -ItemType Directory -Path $TempDirectory | Out-Null

function Test-Candidate {
    param([string]$Candidate)
    $Output = (& $Candidate --version 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or $Output -notmatch 'eggsearch') {
        throw "Downloaded candidate failed its eggsearch version check: $Output"
    }
    $CandidateVersion = if ($Output -match '(?m)^eggsearch\s+([^\s]+)') { $Matches[1] } else { $null }
    if ($Version -and $CandidateVersion -ne $Version) {
        throw "Downloaded candidate version mismatch: expected $Version, got $Output"
    }
}

function Install-Candidate {
    param([string]$Candidate)
    New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null
    $Staged = Join-Path $InstallDirectory ('.eggsearch-{0}.tmp' -f ([guid]::NewGuid().ToString('N')))
    try {
        Copy-Item -LiteralPath $Candidate -Destination $Staged -Force
        Move-Item -LiteralPath $Staged -Destination $Destination -Force
    } finally {
        if (Test-Path -LiteralPath $Staged) {
            Remove-Item -LiteralPath $Staged -Force
        }
    }
    Write-Output "Installed eggsearch at $Destination"
    $PathEntries = @($env:Path -split ';')
    $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not ($PathEntries -contains $InstallDirectory) -and -not (($UserPath -split ';') -contains $InstallDirectory)) {
        Write-Output "Add $InstallDirectory to your user PATH to invoke eggsearch directly"
    }
    if ($Service) {
        & $Destination startup install
        if ($LASTEXITCODE -ne 0) {
            throw "Binary installed at $Destination, but startup registration failed. Run '$Destination startup instructions' for next steps."
        }
    }
}

function Install-FromCargo {
    $Cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $Cargo) {
        throw 'Cargo is required for this unsupported target or missing release asset. Install Rust from https://rustup.rs/ and retry.'
    }
    $CargoRoot = Join-Path $TempDirectory 'cargo-root'
    New-Item -ItemType Directory -Path $CargoRoot | Out-Null
    if ($Version) {
        & cargo install eggsearch --version $Version --locked --root $CargoRoot
    } else {
        & cargo install eggsearch --locked --root $CargoRoot
    }
    if ($LASTEXITCODE -ne 0) {
        throw 'Cargo installation failed'
    }
    $Candidate = Join-Path $CargoRoot 'bin\eggsearch.exe'
    if (-not (Test-Path -LiteralPath $Candidate)) {
        throw "Cargo completed without producing $Candidate"
    }
    Test-Candidate $Candidate
    Install-Candidate $Candidate
}

try {
    if (-not $Target) {
        Write-Output "No prebuilt eggsearch release for Windows/$Architecture; using the documented Cargo fallback"
        Install-FromCargo
        exit 0
    }

    $BaseUrl = if ($Version) {
        "https://github.com/$Repository/releases/download/v$Version"
    } else {
        "https://github.com/$Repository/releases/latest/download"
    }
    $Candidate = Join-Path $TempDirectory $Asset
    $Checksum = Join-Path $TempDirectory "$Asset.sha256"
    $AssetUrl = "$BaseUrl/$Asset"
    $ChecksumUrl = "$BaseUrl/$Asset.sha256"
    $AssetWasMissing = $false
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $AssetUrl -OutFile $Candidate
    } catch {
        $StatusCode = $_.Exception.Response.StatusCode.value__
        if ($StatusCode -eq 404) {
            $AssetWasMissing = $true
        } else {
            throw ('Release binary download failed with HTTP ' + $StatusCode + ': ' + $_.Exception.Message)
        }
    }
    if ($AssetWasMissing) {
        Write-Output "Release asset is unavailable for $Target; using the documented Cargo fallback"
        Install-FromCargo
        exit 0
    }

    Invoke-WebRequest -UseBasicParsing -Uri $ChecksumUrl -OutFile $Checksum
    $ChecksumLine = (Get-Content -LiteralPath $Checksum -Raw).Trim()
    if ($ChecksumLine -notmatch '^(?<Digest>[0-9A-Fa-f]{64})\s{1,2}(?<Name>\S+)$' -or $Matches.Name -ne $Asset) {
        throw "Invalid checksum file for $Asset"
    }
    $ActualDigest = (Get-FileHash -Algorithm SHA256 -LiteralPath $Candidate).Hash
    if ($ActualDigest -ne $Matches.Digest.ToUpperInvariant()) {
        throw "Checksum mismatch for $Asset"
    }
    Test-Candidate $Candidate
    Install-Candidate $Candidate
} finally {
    if (Test-Path -LiteralPath $TempDirectory) {
        Remove-Item -LiteralPath $TempDirectory -Recurse -Force
    }
}
