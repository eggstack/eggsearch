import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import threading
import tempfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

CANDIDATE_SHA = "62387208ac8bc20efb545b68f0704f466bab7295"
ASSET = "eggsearch-aarch64-apple-darwin"


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--evidence-dir", type=Path, default=Path(__file__).resolve().parent)
    return parser.parse_args()


def main():
    args = parse_args()
    binary = args.binary.resolve()
    source_root = args.source_root.resolve()
    evidence_dir = args.evidence_dir.resolve()
    evidence_dir.mkdir(parents=True, exist_ok=True)
    if platform.system() != "Darwin" or platform.machine() not in ("arm64", "aarch64"):
        raise RuntimeError(f"installer smoke requires macOS arm64, got {platform.system()}/{platform.machine()}")
    source_sha = subprocess.check_output(["git", "-C", str(source_root), "rev-parse", "HEAD"], text=True).strip()
    if source_sha != CANDIDATE_SHA:
        raise RuntimeError(f"source SHA mismatch: expected {CANDIDATE_SHA}, got {source_sha}")
    if subprocess.check_output(["git", "-C", str(source_root), "status", "--porcelain"], text=True).strip():
        raise RuntimeError("candidate source worktree is not clean")
    version_output = subprocess.check_output([str(binary), "--version"], text=True).strip()
    match = re.search(r"^eggsearch\s+(\S+)", version_output, re.MULTILINE)
    if not match:
        raise RuntimeError(f"candidate did not report eggsearch version: {version_output}")
    version = match.group(1)
    asset_bytes = binary.read_bytes()
    digest = hashlib.sha256(asset_bytes).hexdigest()
    checksum = f"{digest}  {ASSET}\n".encode()

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            path = urlsplit(self.path).path.rsplit("/", 1)[-1]
            if path == ASSET:
                body = asset_bytes
            elif path == f"{ASSET}.sha256":
                body = checksum
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, format_string, *values):
            return

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()
    base_url = f"http://127.0.0.1:{server.server_port}/releases/download/v{version}"
    try:
        with tempfile.TemporaryDirectory(prefix="eggsearch-m003-installer-") as temp_root:
            home = Path(temp_root) / "home"
            home.mkdir()
            environment = os.environ.copy()
            environment["HOME"] = str(home)
            environment["EGGSEARCH_INSTALL_TEST_MODE"] = "1"
            environment["EGGSEARCH_INSTALL_TEST_BASE_URL"] = base_url
            installer = subprocess.run(
                ["bash", str(source_root / "packaging/install.sh"), "--version", version],
                cwd=source_root,
                env=environment,
                capture_output=True,
                text=True,
                timeout=120,
            )
            if installer.returncode != 0:
                raise RuntimeError(f"installer failed: {installer.stdout}\n{installer.stderr}")
            installed = home / ".local/bin/eggsearch"
            if not installed.is_file() or not os.access(installed, os.X_OK):
                raise RuntimeError("installer did not produce an executable in the temporary home")
            installed_digest = hashlib.sha256(installed.read_bytes()).hexdigest()
            if installed_digest != digest:
                raise RuntimeError("installed binary digest differs from the exact candidate asset")
            installed_version = subprocess.check_output([str(installed), "--version"], text=True).strip()
            if installed_version != version_output:
                raise RuntimeError(f"installed binary identity mismatch: {installed_version}")
            smoke = subprocess.run(
                ["bash", str(source_root / "packaging/release-smoke.sh"), str(installed), version],
                cwd=source_root,
                env=environment,
                capture_output=True,
                text=True,
                timeout=180,
            )
            if smoke.returncode != 0:
                raise RuntimeError(f"installed release smoke failed: {smoke.stdout}\n{smoke.stderr}")
            report = {
                "schema_version": 1,
                "qualification": "browser-artifact-availability-m003-macos-arm64-installer-smoke-001",
                "candidate_sha": CANDIDATE_SHA,
                "source_sha": source_sha,
                "source_worktree_clean": True,
                "host": {"platform": platform.platform(), "system": platform.system(), "machine": platform.machine()},
                "target": "aarch64-apple-darwin",
                "asset": ASSET,
                "version": version,
                "binary_sha256": digest,
                "installer": {
                    "script": "packaging/install.sh",
                    "download_source": "temporary loopback HTTP server",
                    "checksum_verified": True,
                    "temporary_home": True,
                    "cargo_fallback_used": False,
                    "outcome": "passed",
                    "stdout": installer.stdout.strip(),
                },
                "release_smoke": {
                    "script": "packaging/release-smoke.sh",
                    "stdio_and_loopback_http_mcp": True,
                    "outcome": "passed",
                    "stdout_tail": smoke.stdout[-1200:].strip(),
                },
                "outcome": "passed",
            }
            output = evidence_dir / "installer-smoke-macos-arm64.json"
            output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
            print(json.dumps({"outcome": "passed", "target": report["target"], "version": version, "binary_sha256": digest, "evidence": str(output)}, indent=2))
    finally:
        server.shutdown()
        server.server_close()
        server_thread.join(timeout=2)


if __name__ == "__main__":
    main()
