[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Binary,
    [Parameter(Mandatory = $true, Position = 1)]
    [string]$ExpectedVersion
)

$ErrorActionPreference = 'Stop'
$Output = (& $Binary --version 2>&1 | Out-String).Trim()
$CandidateVersion = if ($Output -match '(?m)^eggsearch\s+([^\s]+)') { $Matches[1] } else { $null }
if ($LASTEXITCODE -ne 0 -or $CandidateVersion -ne $ExpectedVersion) {
    throw "Version smoke failed: $Output"
}
& $Binary --help | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw 'Help smoke failed'
}

$StartInfo = New-Object System.Diagnostics.ProcessStartInfo
$StartInfo.FileName = (Resolve-Path $Binary).Path
$StartInfo.Arguments = 'mcp stdio'
$StartInfo.UseShellExecute = $false
$StartInfo.RedirectStandardInput = $true
$StartInfo.RedirectStandardOutput = $true
$StartInfo.RedirectStandardError = $true
$Process = New-Object System.Diagnostics.Process
$Process.StartInfo = $StartInfo
$Process.Start() | Out-Null

function Send-Request {
    param([hashtable]$Message)
    $Process.StandardInput.WriteLine(($Message | ConvertTo-Json -Compress -Depth 10))
    $Process.StandardInput.Flush()
}

function Receive-Response {
    param([int]$Id)
    $Deadline = [DateTime]::UtcNow.AddSeconds(15)
    $Task = $Process.StandardOutput.ReadLineAsync()
    while ([DateTime]::UtcNow -lt $Deadline) {
        if (-not $Task.Wait([TimeSpan]::FromSeconds(1))) {
            continue
        }
        if ($null -eq $Task.Result) {
            throw 'MCP server exited before replying'
        }
        $Message = $Task.Result | ConvertFrom-Json
        if ($Message.id -eq $Id) {
            return $Message
        }
    }
    throw "Timed out waiting for MCP response $Id"
}

try {
    Send-Request @{ jsonrpc = '2.0'; id = 1; method = 'initialize'; params = @{ protocolVersion = '2025-06-18'; capabilities = @{}; clientInfo = @{ name = 'eggsearch-release-smoke'; version = '1' } } }
    $Initialize = Receive-Response 1
    if ($Initialize.result.serverInfo.name -ne 'eggsearch' -or $Initialize.result.serverInfo.version -ne $ExpectedVersion) {
        throw "Unexpected server info: $($Initialize.result.serverInfo | ConvertTo-Json -Compress)"
    }
    Send-Request @{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = @{} }
    Send-Request @{ jsonrpc = '2.0'; id = 2; method = 'tools/list'; params = @{} }
    $Tools = Receive-Response 2
    $ExpectedTools = @('web_search', 'web_fetch', 'batch_fetch', 'provider_status', 'repo_search', 'repo_fetch', 'repo_map', 'security_search', 'research_search', 'build_evidence_bundle')
    $ActualTools = @($Tools.result.tools | ForEach-Object { $_.name } | Sort-Object)
    if ((Compare-Object ($ExpectedTools | Sort-Object) $ActualTools).Length -gt 0) {
        throw "Unexpected MCP tool set: $($ActualTools -join ', ')"
    }
} finally {
    if (-not $Process.HasExited) {
        $Process.Kill()
    }
    $Process.Dispose()
}
