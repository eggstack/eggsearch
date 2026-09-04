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

$TcpListener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
$TcpListener.Start()
$HttpPort = ([System.Net.IPEndPoint]$TcpListener.LocalEndpoint).Port
$TcpListener.Stop()

$HttpStartInfo = New-Object System.Diagnostics.ProcessStartInfo
$HttpStartInfo.FileName = (Resolve-Path $Binary).Path
$HttpStartInfo.Arguments = "mcp serve --bind 127.0.0.1:$HttpPort"
$HttpStartInfo.UseShellExecute = $false
$HttpStartInfo.CreateNoWindow = $true
$HttpStartInfo.RedirectStandardError = $true
$HttpProcess = New-Object System.Diagnostics.Process
$HttpProcess.StartInfo = $HttpStartInfo
$HttpProcess.Start() | Out-Null
$HttpClient = [System.Net.Http.HttpClient]::new()

function Invoke-HttpMcp {
    param(
        [string]$Method,
        [string]$Path,
        [string]$Body,
        [string]$SessionId
    )
    $Request = [System.Net.Http.HttpRequestMessage]::new([System.Net.Http.HttpMethod]::new($Method), "http://127.0.0.1:$HttpPort$Path")
    $Request.Headers.Accept.ParseAdd('application/json, text/event-stream')
    if ($SessionId) {
        $Request.Headers.Add('MCP-Protocol-Version', '2025-06-18')
        $Request.Headers.Add('Mcp-Session-Id', $SessionId)
    }
    if ($Body) {
        $Request.Content = [System.Net.Http.StringContent]::new($Body, [System.Text.Encoding]::UTF8, 'application/json')
    }
    $Response = $HttpClient.SendAsync($Request).GetAwaiter().GetResult()
    $Text = $Response.Content.ReadAsStringAsync().GetAwaiter().GetResult()
    return @{ Response = $Response; Text = $Text }
}

try {
    $Health = $null
    for ($Attempt = 0; $Attempt -lt 50; $Attempt++) {
        try {
            $Result = Invoke-HttpMcp -Method Get -Path '/healthz' -Body '' -SessionId ''
            if ($Result.Response.StatusCode -eq 200) {
                $Health = $Result.Text | ConvertFrom-Json
                break
            }
        } catch {
        }
        Start-Sleep -Milliseconds 100
    }
    if ($null -eq $Health -or $Health.service -ne 'eggsearch' -or $Health.status -ne 'ready' -or $Health.version -ne $ExpectedVersion) {
        throw "Unexpected HTTP health response"
    }

    $InitializeBody = (@{
        jsonrpc = '2.0'; id = 1; method = 'initialize'; params = @{
            protocolVersion = '2025-06-18'; capabilities = @{}; clientInfo = @{ name = 'eggsearch-release-smoke'; version = '1' }
        }
    } | ConvertTo-Json -Compress -Depth 10)
    $Initialize = Invoke-HttpMcp -Method Post -Path '/mcp' -Body $InitializeBody -SessionId ''
    if ($Initialize.Response.StatusCode -ne 200) { throw "HTTP initialize failed: $($Initialize.Response.StatusCode)" }
    $SessionId = ($Initialize.Response.Headers.GetValues('Mcp-Session-Id') | Select-Object -First 1)
    if (-not $SessionId) { throw 'HTTP initialize did not return a session identifier' }
    $InitializeEvent = ($Initialize.Text -split "`n" | Where-Object { $_ -like 'data: *' -and $_.Trim() -ne 'data:' } | Select-Object -First 1).Substring(6) | ConvertFrom-Json
    if ($InitializeEvent.result.serverInfo.name -ne 'eggsearch' -or $InitializeEvent.result.serverInfo.version -ne $ExpectedVersion) {
        throw 'Unexpected HTTP server info'
    }

    $InitializedBody = (@{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = @{} } | ConvertTo-Json -Compress)
    $Initialized = Invoke-HttpMcp -Method Post -Path '/mcp' -Body $InitializedBody -SessionId $SessionId
    if ($Initialized.Response.StatusCode -ne 202) { throw "HTTP initialized notification failed: $($Initialized.Response.StatusCode)" }
    $ToolsBody = (@{ jsonrpc = '2.0'; id = 2; method = 'tools/list'; params = @{} } | ConvertTo-Json -Compress)
    $ToolsResponse = Invoke-HttpMcp -Method Post -Path '/mcp' -Body $ToolsBody -SessionId $SessionId
    if ($ToolsResponse.Response.StatusCode -ne 200) { throw "HTTP tools/list failed: $($ToolsResponse.Response.StatusCode)" }
    $ToolsEvent = ($ToolsResponse.Text -split "`n" | Where-Object { $_ -like 'data: *' -and $_.Trim() -ne 'data:' } | Select-Object -First 1).Substring(6) | ConvertFrom-Json
    $ActualTools = @($ToolsEvent.result.tools | ForEach-Object { $_.name } | Sort-Object)
    $ExpectedTools = @('web_search', 'web_fetch', 'batch_fetch', 'provider_status', 'repo_search', 'repo_fetch', 'repo_map', 'security_search', 'research_search', 'build_evidence_bundle') | Sort-Object
    if ((Compare-Object $ExpectedTools $ActualTools).Length -gt 0) { throw "Unexpected HTTP MCP tool set: $($ActualTools -join ', ')" }
} finally {
    $HttpClient.Dispose()
    if (-not $HttpProcess.HasExited) {
        if (-not $HttpProcess.CloseMainWindow()) {
            $HttpProcess.Kill()
        }
        if (-not $HttpProcess.WaitForExit(10000)) {
            $HttpProcess.Kill()
            $HttpProcess.WaitForExit(5000)
            throw 'HTTP MCP server did not stop cleanly'
        }
    }
    if ($HttpProcess.ExitCode -ne 0) {
        throw "HTTP MCP server exited with code $($HttpProcess.ExitCode)"
    }
    $HttpProcess.Dispose()
}
