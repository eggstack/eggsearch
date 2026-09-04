#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ] || ! command -v shopt >/dev/null 2>&1; then
    echo "release smoke requires bash" >&2
    exit 2
fi

set -euo pipefail

if (($# < 2 || $# > 3)); then
    echo "Usage: $0 BINARY EXPECTED_VERSION [--skip-mcp]" >&2
    exit 2
fi

BINARY="$1"
EXPECTED_VERSION="$2"
SKIP_MCP="${3:-}"

VERSION_OUTPUT="$($BINARY --version)"
candidate_version="$(printf '%s\n' "$VERSION_OUTPUT" | awk '$1 == "eggsearch" { print $2; exit }')"
[[ -n "$candidate_version" && "$candidate_version" == "$EXPECTED_VERSION" ]] || {
    echo "version smoke failed: $VERSION_OUTPUT" >&2
    exit 1
}
$BINARY --help >/dev/null

if [[ "$SKIP_MCP" == "--skip-mcp" ]]; then
    exit 0
fi

command -v python3 >/dev/null 2>&1 || {
    echo "python3 is required for MCP release smoke" >&2
    exit 1
}

python3 - "$BINARY" "$EXPECTED_VERSION" <<'PY'
import json
import http.client
import selectors
import socket
import subprocess
import sys
import time

binary, expected_version = sys.argv[1:]
expected_tools = {
    "web_search",
    "web_fetch",
    "batch_fetch",
    "provider_status",
    "repo_search",
    "repo_fetch",
    "repo_map",
    "security_search",
    "research_search",
    "build_evidence_bundle",
}

process = subprocess.Popen(
    [binary, "mcp", "stdio"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    bufsize=1,
)
selector = selectors.DefaultSelector()
selector.register(process.stdout, selectors.EVENT_READ)

def receive(identifier):
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        events = selector.select(max(0, deadline - time.monotonic()))
        for _, _ in events:
            line = process.stdout.readline()
            if not line:
                raise RuntimeError("MCP server exited before replying")
            message = json.loads(line)
            if message.get("id") == identifier:
                return message
    raise RuntimeError(f"timed out waiting for MCP response {identifier}")

try:
    process.stdin.write(json.dumps({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "eggsearch-release-smoke", "version": "1"},
        },
    }) + "\n")
    process.stdin.flush()
    initialize = receive(1)
    result = initialize.get("result", {})
    server_info = result.get("serverInfo", {})
    if server_info.get("name") != "eggsearch" or server_info.get("version") != expected_version:
        raise RuntimeError(f"unexpected server info: {server_info}")
    process.stdin.write(json.dumps({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {},
    }) + "\n")
    process.stdin.flush()
    process.stdin.write(json.dumps({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {},
    }) + "\n")
    process.stdin.flush()
    tools = receive(2).get("result", {}).get("tools", [])
    names = {tool.get("name") for tool in tools}
    if names != expected_tools:
        raise RuntimeError(f"unexpected MCP tool set: {sorted(names)}")
finally:
    selector.close()
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)

with socket.socket() as probe:
    probe.bind(("127.0.0.1", 0))
    http_port = probe.getsockname()[1]

http_process = subprocess.Popen(
    [binary, "mcp", "serve", "--bind", f"127.0.0.1:{http_port}"],
    stdin=subprocess.DEVNULL,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.PIPE,
    text=True,
)

def http_request(method, path, body=None, headers=None):
    connection = http.client.HTTPConnection("127.0.0.1", http_port, timeout=15)
    connection.request(method, path, body=body, headers=headers or {})
    response = connection.getresponse()
    payload = response.read()
    response_headers = response.headers
    connection.close()
    return response, response_headers, payload

try:
    health = None
    for _ in range(50):
        try:
            candidate, _, payload = http_request("GET", "/healthz")
            if candidate.status == 200:
                health = json.loads(payload)
                break
        except (ConnectionError, OSError, TimeoutError):
            pass
        time.sleep(0.1)
    if health is None:
        raise RuntimeError("HTTP health endpoint did not become ready")
    if health.get("service") != "eggsearch" or health.get("status") != "ready" or health.get("version") != expected_version:
        raise RuntimeError(f"unexpected HTTP health response: {health}")

    common_headers = {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
    }
    initialize_body = json.dumps({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "eggsearch-release-smoke", "version": "1"},
        },
    })
    response, response_headers, payload = http_request("POST", "/mcp", initialize_body, common_headers)
    if response.status != 200:
        raise RuntimeError(f"HTTP initialize failed: {response.status}: {payload[:200]!r}")
    session_id = response_headers.get("Mcp-Session-Id")
    if not session_id:
        raise RuntimeError("HTTP initialize did not return a session identifier")
    events = [line[6:] for line in payload.decode().splitlines() if line.startswith("data: ") and line[6:]]
    initialize = json.loads(events[0])
    server_info = initialize.get("result", {}).get("serverInfo", {})
    if server_info.get("name") != "eggsearch" or server_info.get("version") != expected_version:
        raise RuntimeError(f"unexpected HTTP server info: {server_info}")

    headers = dict(common_headers)
    headers.update({"MCP-Protocol-Version": "2025-06-18", "Mcp-Session-Id": session_id})
    response, _, _ = http_request(
        "POST", "/mcp", json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}), headers
    )
    if response.status != 202:
        raise RuntimeError(f"HTTP initialized notification failed: {response.status}")
    response, _, payload = http_request(
        "POST", "/mcp", json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}), headers
    )
    if response.status != 200:
        raise RuntimeError(f"HTTP tools/list failed: {response.status}: {payload[:200]!r}")
    events = [line[6:] for line in payload.decode().splitlines() if line.startswith("data: ") and line[6:]]
    tools = json.loads(events[0]).get("result", {}).get("tools", [])
    names = {tool.get("name") for tool in tools}
    if names != expected_tools:
        raise RuntimeError(f"unexpected HTTP MCP tool set: {sorted(names)}")
finally:
    http_process.terminate()
    try:
        http_process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        http_process.kill()
        http_process.wait(timeout=5)
        raise RuntimeError("HTTP MCP server did not stop gracefully")
    if http_process.returncode != 0:
        stderr = http_process.stderr.read() if http_process.stderr else ""
        raise RuntimeError(f"HTTP MCP server exited with {http_process.returncode}: {stderr[-500:]}")
PY
