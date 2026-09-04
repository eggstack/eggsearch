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
import selectors
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
PY
