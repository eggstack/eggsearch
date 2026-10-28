import argparse
import asyncio
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import hashlib
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import tracemalloc
from importlib.metadata import distribution
from pathlib import Path
from zipfile import ZipFile
from resource import getrusage, RUSAGE_SELF

from eggsearch import AsyncEggsearch, EggsearchError, tool_names

QUALIFIED_VERSION = "0.4.4"
QUALIFIED_SOURCE_SHA = "1135608af5d625cd72db6704adf3bcabd1a58ab1"


def percentile(values, fraction):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, round((len(ordered) - 1) * fraction))]


def summarize(values):
    return {
        "count": len(values),
        "median_ms": statistics.median(values),
        "p95_ms": percentile(values, 0.95),
        "min_ms": min(values),
        "max_ms": max(values),
    }


def memory_snapshot():
    values: dict[str, int | str] = {"max_rss_kib": getrusage(RUSAGE_SELF).ru_maxrss}
    status = Path("/proc/self/status")
    if status.exists():
        for line in status.read_text().splitlines():
            if line.startswith(("VmRSS:", "VmHWM:", "Threads:")):
                key, value = line.split(":", 1)
                values[key.lower()] = value.strip()
    rollup = Path("/proc/self/smaps_rollup")
    if rollup.exists():
        for line in rollup.read_text().splitlines():
            if line.startswith(("Rss:", "Pss:")):
                key, value = line.split(":", 1)
                values[key.lower()] = value.strip()
    fd_dir = Path("/proc/self/fd")
    if fd_dir.exists():
        values["open_fds"] = len(list(fd_dir.iterdir()))
    return values


def system_snapshot(installed):
    disk = shutil.disk_usage(Path.cwd())
    values = {
        "uname": platform.uname()._asdict(),
        "cpu_count": os.cpu_count(),
        "libc": platform.libc_ver(),
        "filesystem_total_bytes": disk.total,
        "filesystem_free_bytes": disk.free,
        "installed_package_bytes": sum(
            Path(installed.locate_file(path)).stat().st_size
            for path in installed.files or []
            if Path(installed.locate_file(path)).is_file()
        ),
    }
    for path, key in (
        (Path("/etc/os-release"), "os_release"),
        (Path("/proc/meminfo"), "memory_info"),
        (Path("/proc/cpuinfo"), "cpu_info"),
        (Path("/proc/swaps"), "swap_info"),
    ):
        if path.exists():
            values[key] = path.read_text(errors="replace")
    if "memory_info" in values:
        available = next(
            (line.split()[1] for line in values["memory_info"].splitlines() if line.startswith("MemAvailable:")),
            None,
        )
        if available is not None:
            values["memory_available_kib"] = int(available)
    model = Path("/proc/device-tree/model")
    if model.exists():
        values["device_model"] = model.read_bytes().rstrip(b"\0").decode(errors="replace")
    thermal = Path("/sys/class/thermal")
    if thermal.exists():
        values["thermal_zones"] = {
            path.name: path.read_text().strip()
            for zone in thermal.glob("thermal_zone*")
            for path in (zone / "temp",)
            if path.exists()
        }
    frequencies = {}
    for path in Path("/sys/devices/system/cpu").glob("cpu[0-9]*/cpufreq/scaling_cur_freq"):
        frequencies[path.parents[1].name] = path.read_text().strip()
    if frequencies:
        values["cpu_frequency_khz"] = frequencies
    throttling = {}
    for path in Path("/sys/devices/system/cpu").glob("cpu[0-9]*/thermal_throttle/*_throttle_count"):
        throttling[str(path)] = path.read_text().strip()
    if throttling:
        values["thermal_throttle_counts"] = throttling
    return values


def cold_import_ms():
    started = time.perf_counter_ns()
    subprocess.run(
        [sys.executable, "-c", "import eggsearch"],
        check=True,
        stdout=subprocess.DEVNULL,
    )
    return (time.perf_counter_ns() - started) / 1_000_000


def local_config(directory, fixture):
    config = directory / "eggsearch.toml"
    config.write_text(
        "[search]\nmode = 'off'\ndefault_max_results = 100\n"
        "max_results_cap = 100\nmax_query_chars = 512\ntimeout_ms = 8000\n"
        "default_providers = ['duckduckgo']\nsanitize_output = true\n\n"
        "[search.providers]\nduckduckgo = true\n\n"
        "[fetch]\nenabled = true\ntimeout_ms = 8000\nmax_bytes = 2000000\n"
        "max_chars_default = 12000\nmax_chars_cap = 50000\nredirect_limit = 5\n"
        "allow_private_network = false\nallow_localhost = true\n"
        "sanitize_output = true\n\n"
        f"[local]\nenabled = true\nroots = ['{fixture.as_posix()}']\n",
        encoding="utf-8",
    )
    return config


class FixtureHandler(BaseHTTPRequestHandler):
    fixture = ""

    def do_GET(self):
        body = self.fixture.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        return None


def start_fetch_fixture():
    handler = type("BoundFixtureHandler", (FixtureHandler,), {"fixture": "local fetch fixture " * 1000})
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, thread, f"http://127.0.0.1:{server.server_port}/fixture.txt"


async def timed_search(client, index):
    started = time.perf_counter_ns()
    result = await client.repo_search(query=f"retry fixture {index}", include_local=True)
    elapsed = (time.perf_counter_ns() - started) / 1_000_000
    local_cards(result)
    return elapsed


def local_cards(result):
    if not isinstance(result.get("groups"), list):
        raise RuntimeError("local Mode-Off search returned no groups field")
    cards = [
        card
        for group in result["groups"]
        for card in group.get("results", [])
    ]
    if not cards or any(card.get("trust") != "local_trusted" for card in cards):
        raise RuntimeError("Mode-Off smoke did not return only trusted local results")
    return cards


async def sequential_event_loop_sample(config, index):
    client = AsyncEggsearch(str(config))
    try:
        return await timed_search(client, index)
    finally:
        await client.aclose()


async def run_benchmarks(config, repeats):
    results = {"search": {}, "cancellation": {}, "memory": {"before": memory_snapshot()}}
    default_started = time.perf_counter_ns()
    default_client = AsyncEggsearch()
    results["default_client_init_ms"] = (time.perf_counter_ns() - default_started) / 1_000_000
    await default_client.aclose()
    results["memory"]["after_default_client_close"] = memory_snapshot()
    explicit_started = time.perf_counter_ns()
    client = AsyncEggsearch(str(config))
    results["explicit_mode_off_client_init_ms"] = (time.perf_counter_ns() - explicit_started) / 1_000_000
    initialization_started = time.perf_counter_ns()
    await timed_search(client, 0)
    results["first_operation_ms"] = (time.perf_counter_ns() - initialization_started) / 1_000_000
    results["memory"]["initialized"] = memory_snapshot()
    validation_started = time.perf_counter_ns()
    try:
        await client.web_search(query="")
    except EggsearchError as error:
        if not error.code:
            raise RuntimeError("malformed-input error did not include a code") from error
        results["malformed_input"] = {
            "rejected": True,
            "error_code": error.code,
            "elapsed_ms": (time.perf_counter_ns() - validation_started) / 1_000_000,
        }
    else:
        raise RuntimeError("malformed empty query was not rejected")

    for concurrency in (1, 2, 4, 8, 16, 32):
        operations = max(repeats, concurrency)
        durations = []
        for offset in range(0, operations, concurrency):
            batch = range(offset, min(offset + concurrency, operations))
            durations.extend(await asyncio.gather(*(timed_search(client, i) for i in batch)))
        results["search"][str(concurrency)] = summarize(durations)
        results["memory"][f"after_{concurrency}"] = memory_snapshot()

    heavy_started = time.perf_counter_ns()
    heavy_result = await client.repo_search(
        query="retry fixture evidence",
        include_local=True,
        max_results=100,
        max_per_group=100,
        response_detail="diagnostic",
    )
    heavy_cards = local_cards(heavy_result)
    results["heavy_output"] = {
        "elapsed_ms": (time.perf_counter_ns() - heavy_started) / 1_000_000,
        "serialized_bytes": len(json.dumps(heavy_result, separators=(",", ":")).encode()),
        "group_count": len(heavy_result["groups"]),
        "result_count": len(heavy_cards),
        "requested_max_results": 100,
    }

    fixture_server, fixture_thread, fixture_url = start_fetch_fixture()
    try:
        fetch_client = AsyncEggsearch(str(config))
        try:
            fetch_durations = []
            fetch_peak_allocated = 0
            fetch_result = {}
            for index in range(repeats):
                fetch_started = time.perf_counter_ns()
                tracemalloc.start()
                fetch_result = await fetch_client.web_fetch(
                    url=f"{fixture_url}?sample={index}", max_chars=12000
                )
                _, peak_allocated = tracemalloc.get_traced_memory()
                tracemalloc.stop()
                fetch_peak_allocated = max(fetch_peak_allocated, peak_allocated)
                fetch_durations.append((time.perf_counter_ns() - fetch_started) / 1_000_000)
                if not fetch_result.get("fetched") or not fetch_result.get("text"):
                    raise RuntimeError(f"local web_fetch fixture failed: {fetch_result.get('errors')}")
            document = fetch_result.get("document", {})
            results["fetch"] = {
                **summarize(fetch_durations),
                "serialized_bytes": len(json.dumps(fetch_result, separators=(",", ":")).encode()),
                "text_chars": document.get("text_chars_returned", 0),
                "requested_max_chars": 12000,
                "tracemalloc_peak_bytes": fetch_peak_allocated,
                "fixture_scope": "loopback-only",
            }
        finally:
            await fetch_client.aclose()
    finally:
        fixture_server.shutdown()
        fixture_server.server_close()
        fixture_thread.join(timeout=2)
    results["memory"]["after_fetch"] = memory_snapshot()

    cancellation_ms = []
    for round_index in range(repeats):
        tasks = [
            asyncio.create_task(client.repo_search(query=f"cancel {round_index} {i}", include_local=True))
            for i in range(32)
        ]
        await asyncio.sleep(0)
        started = time.perf_counter_ns()
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        cancellation_ms.append((time.perf_counter_ns() - started) / 1_000_000)
    results["cancellation"]["cancel_32_and_drain_ms"] = summarize(cancellation_ms)
    results["cancellation"]["post_cancel_recovery_ms"] = await timed_search(client, repeats)
    close_started = time.perf_counter_ns()
    await client.aclose()
    results["client_close_ms"] = (time.perf_counter_ns() - close_started) / 1_000_000
    results["memory"]["after_close"] = memory_snapshot()
    return results


def run_sequential_event_loop_benchmark(config, repeats):
    durations = []
    for index in range(repeats):
        started = time.perf_counter_ns()
        asyncio.run(sequential_event_loop_sample(config, index))
        durations.append((time.perf_counter_ns() - started) / 1_000_000)
    return summarize(durations)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--repeats", type=int, default=40)
    parser.add_argument("--wheel", type=Path, required=True)
    parser.add_argument("--pip-report", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("sbc-runtime.json"))
    args = parser.parse_args()
    if args.version != QUALIFIED_VERSION or args.source_sha != QUALIFIED_SOURCE_SHA:
        raise SystemExit("version or source SHA does not match the M003-qualified PyPI candidate")
    installed = distribution("eggsearch")
    if installed.version != args.version:
        raise SystemExit(f"installed eggsearch {installed.version} does not match {args.version}")
    wheel_sha256 = hashlib.sha256(args.wheel.read_bytes()).hexdigest()
    report = json.loads(args.pip_report.read_text(encoding="utf-8"))
    package_install = next(
        (
            item
            for item in report.get("install", [])
            if item.get("metadata", {}).get("name", "").lower() == "eggsearch"
        ),
        None,
    )
    if package_install is None or package_install.get("metadata", {}).get("version") != args.version:
        raise SystemExit("pip report does not identify the requested eggsearch version")
    download = package_install.get("download_info", {})
    report_sha256 = download.get("archive_info", {}).get("hashes", {}).get("sha256")
    if not report_sha256 or report_sha256 != wheel_sha256:
        raise SystemExit("downloaded wheel does not match the public pip install report digest")
    if not download.get("url", "").startswith("https://files.pythonhosted.org/"):
        raise SystemExit("pip report does not identify a public PyPI wheel URL")
    with ZipFile(args.wheel) as archive:
        native_path = next(
            path
            for path in archive.namelist()
            if path.startswith("eggsearch/_native.") and path.endswith((".so", ".pyd"))
        )
        installed_native = Path(installed.locate_file(native_path))
        if not installed_native.is_file():
            raise SystemExit("installed native extension is missing")
        if hashlib.sha256(installed_native.read_bytes()).digest() != hashlib.sha256(
            archive.read(native_path)
        ).digest():
            raise SystemExit("installed native extension differs from the published wheel")
    if not sys.platform.startswith("linux") or platform.machine() not in {"armv7l", "armv8l", "aarch64"}:
        raise SystemExit("run this benchmark on a native Linux SBC installation")
    if args.repeats < 3:
        raise SystemExit("use at least three repeats")

    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        workspace = root / "workspace"
        workspace.mkdir()
        for index in range(80):
            text = " ".join([f"retry fixture evidence {index}"] * 160)
            (workspace / f"fixture-{index:03}.md").write_text(text, encoding="utf-8")
        config = local_config(root, workspace)
        measured = asyncio.run(run_benchmarks(config, args.repeats))
        measured["sequential_asyncio_run_loop_ms"] = run_sequential_event_loop_benchmark(
            config, args.repeats
        )

    libc_name, libc_version = platform.libc_ver()
    available_tools = tool_names()
    if len(available_tools) != 10 or len(set(available_tools)) != 10:
        raise SystemExit("installed package did not expose ten unique tool names")
    result = {
        "package_version": installed.version,
        "qualified_source_sha": args.source_sha,
        "wheel": str(args.wheel),
        "wheel_sha256": wheel_sha256,
        "pypi_wheel_url": download["url"],
        "pip_report_sha256": report_sha256,
        "python": sys.version,
        "machine": platform.machine(),
        "platform": platform.platform(),
        "libc": {"name": libc_name, "version": libc_version},
        "system": system_snapshot(installed),
        "tool_count": len(available_tools),
        "tool_names": available_tools,
        "cold_import_ms": cold_import_ms(),
        "wheel_bytes": args.wheel.stat().st_size,
        "usage": {"user_cpu_seconds": getrusage(RUSAGE_SELF).ru_utime, "system_cpu_seconds": getrusage(RUSAGE_SELF).ru_stime},
        "measurements": measured,
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
