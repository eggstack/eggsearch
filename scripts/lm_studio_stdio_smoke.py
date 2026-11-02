import json
import pathlib
import subprocess
import sys
import tempfile


binary = pathlib.Path(sys.argv[1]).resolve()


def run(profile):
    with tempfile.TemporaryDirectory(prefix="eggsearch-stdio-smoke-") as temp:
        config = pathlib.Path(temp) / "eggsearch.toml"
        config.write_text(f'[mcp]\nschema_profile = "{profile}"\nbrief_max_chars = 8000\n')
        proc = subprocess.Popen(
            [str(binary), "--config", str(config), "mcp", "stdio"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

        def request(message):
            proc.stdin.write(json.dumps(message) + "\n")
            proc.stdin.flush()
            line = proc.stdout.readline()
            if not line:
                raise RuntimeError(proc.stderr.read())
            response = json.loads(line)
            if "error" in response:
                raise RuntimeError(response["error"])
            return response["result"]

        request(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "eggsearch-installed-smoke", "version": "1"},
                },
            }
        )
        proc.stdin.write(
            json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n"
        )
        proc.stdin.flush()
        tools = request(
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}
        )["tools"]
        names = sorted(tool["name"] for tool in tools)
        if len(names) != 10:
            raise AssertionError(f"{profile}: expected 10 tools, got {names}")
        if profile == "compact":
            encoded_tools = json.dumps(tools, separators=(",", ":"))
            if len(encoded_tools.encode()) > 36_000:
                raise AssertionError(f"compact tools/list too large: {len(encoded_tools)} bytes")
            if any("outputSchema" in tool for tool in tools):
                raise AssertionError("compact profile unexpectedly contains outputSchema")
        call = request(
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "provider_status", "arguments": {"probe": False}},
            }
        )
        if call.get("isError"):
            raise AssertionError(f"{profile}: provider_status failed: {call}")
        proc.stdin.close()
        proc.wait(timeout=10)
        stderr = proc.stderr.read()
        print(
            json.dumps(
                {
                    "profile": profile,
                    "tool_count": len(names),
                    "tools_list_bytes": len(
                        json.dumps(tools, separators=(",", ":")).encode()
                    ),
                    "provider_status_ok": True,
                    "exit_code": proc.returncode,
                    "stderr_nonempty": bool(stderr),
                }
            )
        )
        return call.get("structuredContent")


full = run("full")
compact = run("compact")
if full != compact:
    raise AssertionError("full and compact profiles returned different provider_status payloads")
