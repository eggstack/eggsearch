import argparse
import hashlib
import json
import math
import platform
import selectors
import subprocess
import sys
import threading
import time
from collections import defaultdict
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

CANDIDATE_SHA = "62387208ac8bc20efb545b68f0704f466bab7295"
CHROME = Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
IDLE_TIMEOUT_MS = 5000
MAX_REQUESTS = 13


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--evidence-dir", type=Path, default=Path(__file__).resolve().parent)
    return parser.parse_args()


def descendants_snapshot(root_pid):
    output = subprocess.check_output(["ps", "-axo", "pid=,ppid=,rss=,comm="], text=True)
    rows = {}
    for line in output.splitlines():
        fields = line.strip().split(None, 3)
        if len(fields) != 4:
            continue
        pid, ppid, rss, command = fields
        rows[int(pid)] = {"ppid": int(ppid), "rss_kib": int(rss), "command": command}
    children = defaultdict(list)
    for pid, row in rows.items():
        children[row["ppid"]].append(pid)
    pending = list(children.get(root_pid, []))
    result = []
    seen = set()
    while pending:
        pid = pending.pop()
        if pid in seen or pid not in rows:
            continue
        seen.add(pid)
        row = rows[pid]
        result.append({"pid": pid, **row})
        pending.extend(children.get(pid, []))
    return {"count": len(result), "rss_kib": sum(row["rss_kib"] for row in result), "processes": result}


class ResourceSampler:
    def __init__(self, root_pid):
        self.root_pid = root_pid
        self.stop_event = threading.Event()
        self.peak = {"rss_kib": 0, "count": 0, "processes": []}
        self.thread = threading.Thread(target=self.run, daemon=True)

    def run(self):
        while not self.stop_event.is_set():
            try:
                sample = descendants_snapshot(self.root_pid)
                if sample["rss_kib"] > self.peak["rss_kib"]:
                    self.peak = sample
            except (OSError, subprocess.SubprocessError, ValueError):
                pass
            self.stop_event.wait(0.1)

    def start(self):
        self.thread.start()

    def stop(self):
        self.stop_event.set()
        self.thread.join(timeout=2)


class McpClient:
    def __init__(self, binary, config, cwd):
        self.process = subprocess.Popen(
            [str(binary), "--config", str(config), "mcp", "stdio"],
            cwd=cwd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)
        self.next_id = 1
        response = self.request(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "eggsearch-m003-realistic-mix", "version": "1"},
            },
        )
        server_info = response.get("result", {}).get("serverInfo", {})
        if server_info.get("name") != "eggsearch":
            raise RuntimeError(f"unexpected MCP server identity: {server_info}")
        self.version = server_info.get("version")
        self.notify("notifications/initialized", {})

    def receive(self, identifier, timeout_seconds=90):
        deadline = time.monotonic() + timeout_seconds
        while time.monotonic() < deadline:
            events = self.selector.select(max(0.0, deadline - time.monotonic()))
            if not events:
                continue
            line = self.process.stdout.readline()
            if not line:
                raise RuntimeError("MCP server exited before replying")
            message = json.loads(line)
            if message.get("id") == identifier:
                return message
        raise TimeoutError(f"timed out waiting for MCP response {identifier}")

    def request(self, method, params, timeout_seconds=90):
        identifier = self.next_id
        self.next_id += 1
        self.process.stdin.write(
            json.dumps({"jsonrpc": "2.0", "id": identifier, "method": method, "params": params}) + "\n"
        )
        self.process.stdin.flush()
        response = self.receive(identifier, timeout_seconds)
        if "error" in response:
            raise RuntimeError(f"MCP request failed: {response['error']}")
        return response

    def notify(self, method, params):
        self.process.stdin.write(json.dumps({"jsonrpc": "2.0", "method": method, "params": params}) + "\n")
        self.process.stdin.flush()

    def tool(self, name, arguments, timeout_seconds=90):
        response = self.request(
            "tools/call",
            {"name": name, "arguments": arguments},
            timeout_seconds,
        )
        result = response.get("result", {})
        if result.get("isError"):
            detail = result.get("content", [{}])[0].get("text", "MCP tool failed")
            raise RuntimeError(f"{name} failed: {detail}")
        payload = result.get("structuredContent")
        if isinstance(payload, dict):
            return payload
        for item in result.get("content", []):
            text = item.get("text")
            if not text:
                continue
            try:
                parsed = json.loads(text)
            except json.JSONDecodeError:
                continue
            if isinstance(parsed, dict):
                return parsed
        raise RuntimeError(f"{name} returned no structured JSON payload")

    def close(self):
        self.selector.close()
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        if self.process.stdin:
            self.process.stdin.close()
        if self.process.stdout:
            self.process.stdout.close()


def write_config(path):
    path.write_text(
        "\n".join(
            [
                "[fetch]",
                "enabled = true",
                "timeout_ms = 30000",
                "allow_private_network = true",
                "allow_localhost = true",
                "sanitize_output = true",
                "pdf_enabled = true",
                "",
                "[fetch.browser]",
                "enabled = true",
                'policy = "auto"',
                f"executable = {json.dumps(str(CHROME))}",
                "startup_timeout_ms = 15000",
                "navigation_timeout_ms = 30000",
                "post_load_wait_ms = 1500",
                "verification_wait_ms = 10000",
                f"max_requests = {MAX_REQUESTS}",
                f"idle_timeout_ms = {IDLE_TIMEOUT_MS}",
                "max_dom_bytes = 4000000",
                "global_concurrency = 1",
                "per_origin_concurrency = 1",
                "block_media = true",
                "",
            ]
        ),
        encoding="utf-8",
    )


def make_handler(routes):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            route = routes.get(urlsplit(self.path).path)
            if route is None:
                self.send_error(404)
                return
            content_type, body = route
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, format_string, *args):
            return

    return Handler


def payload_text(payload):
    return json.dumps(payload, ensure_ascii=False, sort_keys=True)


def percentile(values, fraction):
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)] if ordered else None


def summarize(samples):
    result = {}
    groups = sorted({sample["group"] for sample in samples})
    for group in groups:
        result[group] = {}
        for phase in ("cold", "warm"):
            values = [sample["latency_ms"] for sample in samples if sample["group"] == group and sample["phase"] == phase]
            result[group][phase] = {
                "n": len(values),
                "p50_ms": percentile(values, 0.50),
                "p95_ms": percentile(values, 0.95),
            }
    return result


def get_lifecycle(client):
    payload = client.tool("provider_status", {"probe": False, "recipe_detail": "none"})
    browser = payload.get("browser_capabilities", {})
    lifecycle = browser.get("lifecycle")
    if not isinstance(lifecycle, dict):
        raise RuntimeError(f"browser lifecycle status unavailable: {browser}")
    return browser, lifecycle


def main():
    args = parse_args()
    binary = args.binary.resolve()
    source_root = args.source_root.resolve()
    evidence_dir = args.evidence_dir.resolve()
    evidence_dir.mkdir(parents=True, exist_ok=True)
    if not binary.is_file() or not binary.stat().st_mode & 0o111:
        raise RuntimeError(f"candidate binary is missing or not executable: {binary}")
    if not CHROME.is_file() or not CHROME.stat().st_mode & 0o111:
        raise RuntimeError(f"Chrome is missing or not executable: {CHROME}")
    source_sha = subprocess.check_output(["git", "-C", str(source_root), "rev-parse", "HEAD"], text=True).strip()
    if source_sha != CANDIDATE_SHA:
        raise RuntimeError(f"source SHA mismatch: expected {CANDIDATE_SHA}, got {source_sha}")
    source_status = subprocess.check_output(["git", "-C", str(source_root), "status", "--porcelain"], text=True)
    if source_status.strip():
        raise RuntimeError("candidate source worktree is not clean")

    pdf_specs = [
        ("clean_text", "tests/fixtures/pdf/clean_text/single_column_notes.pdf", "Clean text sample"),
        ("two_column", "tests/fixtures/pdf/two_column_interleaved/arxiv_style_two_column.pdf", "Retrieval Fidelity Under Interleaved Columns"),
        ("standards_table", "tests/fixtures/pdf/standards_table/rfc_style_standards_table.pdf", "Internet Engineering Task Force"),
    ]
    scan_path = source_root / "tests/fixtures/pdf/image_only/scanned_placeholder.pdf"
    pdf_hashes = {}
    routes = {}
    cases = []

    for index in range(30):
        marker = f"static-fixture-marker-{index:02d}"
        text = f"{marker} This ordinary static HTML page contains enough meaningful words to stay on the HTTP extraction path."
        routes[f"/static/{index}"] = ("text/html; charset=utf-8", f"<!doctype html><html><head><title>Static {index}</title></head><body><article><p>{text}</p></article></body></html>".encode())
        cases.append({"group": "static_html", "path": f"/static/{index}", "marker": marker, "expected_transport": "http"})

    for index in range(10):
        marker = f"js{index:02d}"
        script = f"document.body.innerText='{marker}'"
        html = f"<!doctype html><html><head><title>X</title></head><body><div id=\"root\"></div><script>{script}</script></body></html>"
        routes[f"/javascript/{index}"] = ("text/html; charset=utf-8", html.encode())
        cases.append({"group": "javascript_shell", "path": f"/javascript/{index}", "marker": marker, "expected_transport": "browser"})

    for index, (label, relative_path, marker) in enumerate(pdf_specs):
        path = source_root / relative_path
        body = path.read_bytes()
        pdf_hashes[label] = hashlib.sha256(body).hexdigest()
        routes[f"/text-pdf/{index}"] = ("application/pdf", body)
        cases.append({"group": "text_pdf", "path": f"/text-pdf/{index}", "marker": marker, "expected_transport": "http", "fixture": relative_path})

    scan_bytes = scan_path.read_bytes()
    scan_hash = hashlib.sha256(scan_bytes).hexdigest()
    pdf_hashes["scanned_placeholder"] = scan_hash
    for index in range(3):
        routes[f"/scanned-pdf/{index}"] = ("application/pdf", scan_bytes)
        cases.append({"group": "scanned_pdf", "path": f"/scanned-pdf/{index}", "marker": None, "expected_transport": "http", "fixture": "tests/fixtures/pdf/image_only/scanned_placeholder.pdf", "fixture_sha256": scan_hash})

    challenge_titles = ["Just a moment", "Checking your browser", "Security verification"]
    for index, title in enumerate(challenge_titles):
        marker = f"verification-fixture-marker-{index:02d}"
        script = f"setTimeout(()=>{{document.title='Verified';document.body.innerHTML='<main><p>{marker} verification resolved without interaction.</p></main>'; }},100);"
        html = f"<!doctype html><html><head><title>{title}</title></head><body><main><p>Please wait while we verify your browser.</p><script>{script}</script></main></body></html>"
        routes[f"/verification/{index}"] = ("text/html; charset=utf-8", html.encode())
        cases.append({"group": "verification_wall", "path": f"/verification/{index}", "marker": marker, "expected_transport": "browser"})

    server = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(routes))
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()
    origin = f"http://127.0.0.1:{server.server_port}"
    config_path = evidence_dir / "run-config.toml"
    write_config(config_path)
    sampler = None
    client = None
    samples = []
    actual_browser_calls = 0
    try:
        client = McpClient(binary, config_path, source_root)
        if client.version is None:
            raise RuntimeError("MCP server did not report a version")
        browser_capabilities, initial_lifecycle = get_lifecycle(client)
        if not browser_capabilities.get("usable"):
            raise RuntimeError(f"browser capability is not usable: {browser_capabilities}")
        sampler = ResourceSampler(client.process.pid)
        sampler.start()
        preflight_start = time.perf_counter()
        preflight_payload = client.tool(
            "web_fetch",
            {
                "url": f"{origin}/javascript/0?preflight=1",
                "render": "auto",
                "cache_policy": "bypass",
                "timeout_ms": 30000,
                "max_chars": 5000,
                "response_detail": "diagnostic",
            },
        )
        preflight_latency_ms = round((time.perf_counter() - preflight_start) * 1000, 3)
        if preflight_payload.get("transport") != "browser" or not preflight_payload.get("browser_escalated"):
            raise RuntimeError(f"JavaScript-shell preflight did not escalate: {preflight_payload}")
        if "js00" not in payload_text(preflight_payload):
            raise RuntimeError("JavaScript-shell preflight did not extract its marker")
        for repetition in range(3):
            for case in cases:
                url = f"{origin}{case['path']}?pair={repetition}"
                for phase, policy in (("cold", "bypass"), ("warm", "default")):
                    start = time.perf_counter()
                    payload = client.tool(
                        "web_fetch",
                        {
                            "url": url,
                            "render": "auto",
                            "cache_policy": policy,
                            "timeout_ms": 30000,
                            "max_chars": 5000,
                            "response_detail": "diagnostic",
                        },
                    )
                    latency_ms = round((time.perf_counter() - start) * 1000, 3)
                    transport = payload.get("transport")
                    escalated = bool(payload.get("browser_escalated"))
                    marker_present = bool(case["marker"] and case["marker"].lower() in payload_text(payload).lower())
                    expected_browser = case["expected_transport"] == "browser"
                    if phase == "cold":
                        if transport != case["expected_transport"] or escalated != expected_browser:
                            raise RuntimeError(
                                f"unexpected cold routing for {case['group']} {case['path']}: transport={transport}, escalated={escalated}, title={payload.get('title')!r}, text={payload.get('text')!r}, warnings={payload.get('warnings')!r}"
                            )
                        if expected_browser:
                            actual_browser_calls += 1
                        if case["marker"] and not marker_present:
                            raise RuntimeError(f"expected extracted marker missing for {case['group']} {case['path']}")
                        if case["group"] == "scanned_pdf" and payload.get("text") not in (None, ""):
                            raise RuntimeError(f"scanned PDF unexpectedly yielded text: {case['path']}")
                    samples.append(
                        {
                            "group": case["group"],
                            "case": case["path"],
                            "repetition": repetition + 1,
                            "phase": phase,
                            "latency_ms": latency_ms,
                            "transport": transport,
                            "browser_escalated": escalated,
                            "cache_status": payload.get("cache_status"),
                            "status": payload.get("status"),
                            "marker_present": marker_present,
                            "text_chars": len(payload.get("text") or ""),
                        }
                    )

        _, after_matrix_lifecycle = get_lifecycle(client)
        if after_matrix_lifecycle.get("session_cap_teardowns", 0) < 3:
            raise RuntimeError(f"max_requests session cap was not exercised three times: {after_matrix_lifecycle}")
        before_idle = descendants_snapshot(client.process.pid)
        time.sleep(IDLE_TIMEOUT_MS / 1000 + 1.5)
        _, after_idle_lifecycle = get_lifecycle(client)
        after_idle_processes = descendants_snapshot(client.process.pid)
        if after_idle_lifecycle.get("resident"):
            raise RuntimeError(f"browser remained resident after idle timeout: {after_idle_lifecycle}")
        if after_idle_processes["count"]:
            raise RuntimeError(f"browser descendants remained after idle timeout: {after_idle_processes}")

        relaunch_case = {
            "group": "relaunch_probe",
            "path": "/javascript/0?relaunch=1",
            "marker": "js00",
            "expected_transport": "browser",
        }
        relaunch_start = time.perf_counter()
        relaunch_payload = client.tool(
            "web_fetch",
            {
                "url": f"{origin}{relaunch_case['path']}",
                "render": "auto",
                "cache_policy": "bypass",
                "timeout_ms": 30000,
                "max_chars": 5000,
                "response_detail": "diagnostic",
            },
        )
        relaunch_latency_ms = round((time.perf_counter() - relaunch_start) * 1000, 3)
        if relaunch_payload.get("transport") != "browser" or not relaunch_payload.get("browser_escalated"):
            raise RuntimeError(f"browser did not relaunch after idle teardown: {relaunch_payload}")
        if relaunch_case["marker"] not in payload_text(relaunch_payload):
            raise RuntimeError("relaunch probe did not extract the JavaScript marker")
        _, after_relaunch_lifecycle = get_lifecycle(client)
        if not after_relaunch_lifecycle.get("resident"):
            raise RuntimeError(f"browser was not resident after relaunch probe: {after_relaunch_lifecycle}")
        time.sleep(IDLE_TIMEOUT_MS / 1000 + 1.5)
        _, final_lifecycle = get_lifecycle(client)
        final_processes = descendants_snapshot(client.process.pid)
        if final_lifecycle.get("resident") or final_processes["count"]:
            raise RuntimeError(f"browser did not return to baseline after final idle timeout: {final_lifecycle}, {final_processes}")
    finally:
        if sampler:
            sampler.stop()
        if client:
            client.close()
        server.shutdown()
        server.server_close()
        server_thread.join(timeout=2)

    version_output = subprocess.check_output([str(binary), "--version"], text=True).strip()
    chrome_version = subprocess.check_output([str(CHROME), "--version"], text=True, stderr=subprocess.STDOUT).strip()
    report = {
        "schema_version": 1,
        "qualification": "browser-artifact-availability-m003-realistic-mix-001",
        "candidate_sha": CANDIDATE_SHA,
        "source_sha": source_sha,
        "source_worktree_clean": True,
        "binary": {
            "path": str(binary),
            "version": version_output,
            "size_bytes": binary.stat().st_size,
            "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        },
        "host": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "python": sys.version.split()[0],
            "chrome": chrome_version,
        },
        "fixture_origin": origin,
        "fixture_counts": {
            "static_html": 30,
            "javascript_shell": 10,
            "text_pdf": 3,
            "scanned_pdf_routes": 3,
            "noninteractive_verification": 3,
            "repetitions_per_case": 3,
            "cache_pairs_per_case": 2,
        },
        "configuration": {
            "allow_localhost": True,
            "allow_private_network": True,
            "permissions_scope": "temporary local qualification config only",
            "pdf_enabled": True,
            "browser_enabled": True,
            "browser_policy": "auto",
            "idle_timeout_ms": IDLE_TIMEOUT_MS,
            "max_requests": MAX_REQUESTS,
            "post_load_wait_ms": 1500,
            "browser_executable": str(CHROME),
        },
        "pdf_fixture_sha256": pdf_hashes,
        "sample_count": len(samples),
        "actual_cold_browser_escalations": actual_browser_calls,
        "javascript_preflight_latency_ms": preflight_latency_ms,
        "latency_ms": summarize(samples),
        "samples": samples,
        "process_tree": {
            "sampling_interval_ms": 100,
            "peak_descendant_rss_kib": sampler.peak["rss_kib"],
            "peak_descendant_count": sampler.peak["count"],
            "peak_descendant_processes": sampler.peak["processes"],
            "before_idle": before_idle,
            "after_idle": after_idle_processes,
            "after_final_idle": final_processes,
        },
        "browser_lifecycle": {
            "initial": initial_lifecycle,
            "after_matrix": after_matrix_lifecycle,
            "after_idle": after_idle_lifecycle,
            "after_relaunch": after_relaunch_lifecycle,
            "final": final_lifecycle,
        },
        "relaunch_probe_latency_ms": relaunch_latency_ms,
        "outcome": "passed",
        "limitations": [
            "The three scanned-PDF routes use the same captured synthetic image-only fixture bytes at distinct URLs.",
            "RSS is a 100 ms process-tree sample on this macOS arm64 host and is an environment observation, not a threshold.",
            "The max_requests and idle_timeout_ms values are shortened for this run to exercise teardown behavior efficiently.",
        ],
    }
    output_path = evidence_dir / "evidence.json"
    output_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps({"outcome": report["outcome"], "sample_count": report["sample_count"], "actual_cold_browser_escalations": actual_browser_calls, "latency_ms": report["latency_ms"], "browser_lifecycle": report["browser_lifecycle"], "peak_descendant_rss_kib": report["process_tree"]["peak_descendant_rss_kib"], "evidence": str(output_path)}, indent=2))


if __name__ == "__main__":
    main()
