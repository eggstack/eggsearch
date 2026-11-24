import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import run_retrieval_quality_external_live as runner


BLOCKED_OUTPUT = {
    "results": [],
    "providers_failed": [
        {
            "id": "mojeek",
            "error_class": "blocked",
            "message": "engine 'mojeek' blocked: challenge page detected (automated queries) on HTTP 403",
        }
    ],
}
BAD_STATUS_OUTPUT = {
    "results": [],
    "providers_failed": [
        {"id": "mojeek", "error_class": "http_status", "message": "engine 'mojeek' returned status 503"}
    ],
}
RESULTS_OUTPUT = {
    "results": [{"title": "Example", "url": "https://example.com", "snippet": "s"}],
    "providers_failed": [],
}


def write_stub(directory, payload):
    path = Path(directory) / "stub-binary.sh"
    path.write_text("#!/bin/sh\necho '" + json.dumps(payload) + "'\n")
    path.chmod(path.stat().st_mode | stat.S_IEXEC)
    return path


def run_script(binary, extra):
    command = [
        sys.executable,
        str(Path(runner.__file__)),
        "--binary",
        str(binary),
        "--source-sha",
        "0" * 40,
        "--provider",
        "mojeek",
        "--dataset",
        "scifact",
        "--query-count",
        "2",
        "--repetitions",
        "2",
        "--max-results",
        "5",
        "--timeout-seconds",
        "5",
        *extra,
    ]
    completed = subprocess.run(command, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        raise AssertionError(completed.stderr)
    return json.loads(completed.stdout)


class ScheduleTests(unittest.TestCase):
    def test_interval_is_deterministic_and_bounded(self):
        first = runner.attempt_interval("m009-paced-v1", "mojeek", "scifact:1", 1, 25.0, 40.0)
        second = runner.attempt_interval("m009-paced-v1", "mojeek", "scifact:1", 1, 25.0, 40.0)
        self.assertEqual(first, second)
        self.assertGreaterEqual(first, 25.0)
        self.assertLessEqual(first, 40.0)

    def test_interval_varies_by_attempt_identity(self):
        values = {
            runner.attempt_interval("m009-paced-v1", "mojeek", f"scifact:{index}", repetition, 25.0, 40.0)
            for index in range(6)
            for repetition in (1, 2)
        }
        self.assertGreater(len(values), 1)


class StatusExtractionTests(unittest.TestCase):
    def test_bad_status_message(self):
        self.assertEqual(runner.http_status_from_errors(BAD_STATUS_OUTPUT["providers_failed"]), 503)

    def test_blocked_message_carries_status(self):
        self.assertEqual(runner.http_status_from_errors(BLOCKED_OUTPUT["providers_failed"]), 403)

    def test_absent_status_is_none(self):
        self.assertIsNone(runner.http_status_from_errors([{"id": "x", "error_class": "timeout"}]))

    def test_blocked_row_detection(self):
        self.assertTrue(runner.is_blocked_row({"outcome": "provider_error", "provider_errors": BLOCKED_OUTPUT["providers_failed"]}))
        self.assertFalse(runner.is_blocked_row({"outcome": "provider_error", "provider_errors": BAD_STATUS_OUTPUT["providers_failed"]}))
        self.assertFalse(runner.is_blocked_row({"outcome": "results", "provider_errors": []}))


class ProtocolTests(unittest.TestCase):
    def test_consecutive_blocks_abort_the_arm(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = write_stub(directory, BLOCKED_OUTPUT)
            report = run_script(
                binary,
                [
                    "--protocol-id", "m009-test",
                    "--min-interval-seconds", "0.05",
                    "--max-interval-seconds", "0.1",
                    "--consecutive-blocked-stop", "3",
                ],
            )
        self.assertTrue(report["aborted"])
        self.assertEqual(report["provider_query_attempts"], 3)
        self.assertEqual(report["refused_attempts"], 3)
        self.assertEqual(report["http_status_counts"], {"403": 3})
        self.assertEqual(report["protocol"]["id"], "m009-test")
        self.assertEqual(report["pacing"]["violations"], [])
        self.assertGreaterEqual(report["pacing"]["min_observed_interval_seconds"], 0.05)
        self.assertIsNone(report["rows"][0]["actual_interval_seconds"])

    def test_single_block_run_completes(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = write_stub(directory, RESULTS_OUTPUT)
            report = run_script(binary, ["--protocol-id", "m009-test", "--min-interval-seconds", "0.05", "--max-interval-seconds", "0.1"])
        self.assertFalse(report["aborted"])
        self.assertEqual(report["provider_query_attempts"], 4)
        self.assertEqual(report["refused_attempts"], 0)
        self.assertEqual(report["pacing"]["violations"], [])

    def test_unpaced_run_keeps_legacy_report(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = write_stub(directory, RESULTS_OUTPUT)
            report = run_script(binary, [])
        self.assertIsNone(report["protocol"])
        self.assertFalse(report["aborted"])
        self.assertEqual(report["provider_query_attempts"], 4)
        self.assertNotIn("actual_interval_seconds", report["rows"][0])


if __name__ == "__main__":
    unittest.main()
