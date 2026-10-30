import argparse
import hashlib
import json
import platform
import re
import subprocess
import time
import unicodedata
from pathlib import Path


FIXTURE_DIRECTORY = Path(__file__).parents[1] / "tests/fixtures/retrieval_quality"
DATASETS = {
    "scifact": {
        "name": "SciFact BEIR test",
        "query_file": "scifact_test_queries.jsonl",
        "qrel_file": "scifact_test_qrels.jsonl",
        "archive_sha256": "536e14446a0ba56ed1398ab1055f39fe852686ecad24a6306c80c490fa8e0165",
    },
    "nq": {
        "name": "Natural Questions BEIR test",
        "query_file": "nq_test_queries.jsonl",
        "qrel_file": "nq_test_qrels.jsonl",
        "archive_sha256": "2553bf7bfbab47b1436ca00a34bce57320e18e611fd00999a2a3a1b4714be752",
        "source_fingerprints": {
            "beir_nq_archive_sha256": "2553bf7bfbab47b1436ca00a34bce57320e18e611fd00999a2a3a1b4714be752",
        },
    },
    "touche2020": {
        "name": "Touché 2020 test",
        "query_file": "touche2020_test_queries.jsonl",
        "qrel_file": "touche2020_test_qrels.jsonl",
        "archive_sha256": "1a706ed9c03ecc75cf012495bd8e37b761f7b0350a939910d0f54862e26fc601",
        "source_fingerprints": {
            "beir_touche2020_archive_sha256": "1a706ed9c03ecc75cf012495bd8e37b761f7b0350a939910d0f54862e26fc601",
        },
    },
    "fever": {
        "name": "FEVER BEIR test",
        "query_file": "fever_test_queries.jsonl",
        "qrel_file": "fever_test_qrels.jsonl",
        "archive_sha256": None,
        "source_fingerprints": {
            "beir_fever_queries_parquet_sha256": "b9cf00001ef1aa07a7030118490ce5f8a840e783ad8d972b9afee8ef281d6093",
            "beir_fever_test_qrels_tsv_sha256": "6599dab4e2189ca5eb5216546021710c06dc8bdd1a0c59cc1fa52129c6e038b3",
            "beir_fever_revision": "426aea4d30c65c041f093ae072e25fb5a0e64659",
            "beir_fever_qrels_revision": "64cd0e4ef9b63a88ac8e69d1133a2f649acd4745",
        },
    },
}
KEYLESS_PROVIDERS = {"duckduckgo", "startpage", "yahoo", "brave", "mojeek", "searxng"}
MAX_PROVIDER_QUERIES = 80
MAX_RESULTS = 10
DATASET_ARCHIVE_SHA256 = "536e14446a0ba56ed1398ab1055f39fe852686ecad24a6306c80c490fa8e0165"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def normalize_title(value):
    normalized = unicodedata.normalize("NFKD", value).casefold()
    return "".join(char for char in normalized if char.isalnum())


def normalize_url(value):
    value = value.strip().casefold().split("#", 1)[0]
    return value.rstrip("/")


def load_corpus(dataset_id):
    manifest = DATASETS[dataset_id]
    queries_raw = (FIXTURE_DIRECTORY / manifest["query_file"]).read_bytes()
    qrels_raw = (FIXTURE_DIRECTORY / manifest["qrel_file"]).read_bytes()
    queries = [json.loads(line) for line in queries_raw.splitlines()]
    qrels = [json.loads(line) for line in qrels_raw.splitlines()]
    if not queries or len(queries) != len(qrels):
        raise ValueError("external corpus schema or case count differs from the frozen manifest")
    if [case["id"] for case in queries] != [case["id"] for case in qrels]:
        raise ValueError("external query and qrel IDs do not align")
    if dataset_id == "scifact" and (len(queries) != 300 or len(qrels) != 300):
        raise ValueError("SciFact case count differs from the frozen manifest")
    cases = [dict(query, relevant_documents=judgments["relevant_documents"]) for query, judgments in zip(queries, qrels)]
    if dataset_id == "scifact" and sum(len(case["relevant_documents"]) for case in cases) != 339:
        raise ValueError("external corpus judgment count differs from the frozen manifest")
    fingerprint = digest(queries_raw + b"\0" + qrels_raw)
    return {"dataset": manifest["name"], "cases": cases}, fingerprint, manifest


def select_cases(cases, query_count, seed):
    ordered = sorted(cases, key=lambda case: hashlib.sha256(f"{seed}:{case['id']}".encode()).digest())
    return ordered[:query_count]


def run_one(binary, config, provider, case, repetition, max_results, timeout):
    command = [str(binary)]
    if config:
        command.extend(["--config", str(config)])
    command.extend(
        ["search", "--json", "--providers", provider, "--max-results", str(max_results), case["query"]]
    )
    started = time.monotonic()
    try:
        completed = subprocess.run(command, capture_output=True, timeout=timeout, check=False)
        elapsed_ms = round((time.monotonic() - started) * 1000)
        raw = completed.stdout + completed.stderr
        try:
            response = json.loads(completed.stdout)
        except json.JSONDecodeError:
            response = {}
        results = response.get("results", []) if isinstance(response, dict) else []
        errors = response.get("providers_failed", []) if isinstance(response, dict) else []
        relevant_ids = {item["corpus_id"] for item in case["relevant_documents"]}
        relevant_titles = {
            normalize_title(item["title"]): item["corpus_id"]
            for item in case["relevant_documents"]
            if item.get("title")
        }
        relevant_urls = {
            normalize_url(item["url"]): item["corpus_id"]
            for item in case["relevant_documents"]
            if item.get("url")
        }
        matches = []
        matched_ids = set()
        for rank, result in enumerate(results[:max_results], 1):
            title = result.get("title", "") if isinstance(result, dict) else ""
            url = result.get("url", "") if isinstance(result, dict) else ""
            matched_id = relevant_urls.get(normalize_url(url)) if url else None
            if matched_id is None:
                matched_id = relevant_titles.get(normalize_title(title))
            if matched_id is not None and matched_id not in matched_ids:
                matched_ids.add(matched_id)
                matches.append({"corpus_id": matched_id, "rank": rank})
        if completed.returncode != 0:
            outcome = "command_error"
        elif errors:
            outcome = "provider_error"
        elif results:
            outcome = "results"
        else:
            outcome = "zero_results"
        return {
            "query_id": case["id"],
            "repetition": repetition,
            "outcome": outcome,
            "provider_errors": [
                {"id": item.get("id"), "error_class": item.get("error_class")}
                for item in errors
                if isinstance(item, dict)
            ],
            "result_count": len(results),
            "matched_relevant_documents": matches,
            "relevant_document_count": len(relevant_ids),
            "matched_relevant_document_count": len(matches),
            "latency_ms": elapsed_ms,
            "raw_response_sha256": digest(raw),
        }
    except subprocess.TimeoutExpired:
        return {
            "query_id": case["id"],
            "repetition": repetition,
            "outcome": "timeout",
            "result_count": 0,
            "matched_relevant_documents": [],
            "relevant_document_count": len(case["relevant_documents"]),
            "matched_relevant_document_count": 0,
            "latency_ms": round(timeout * 1000),
            "raw_response_sha256": None,
        }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--provider", required=True, choices=sorted(KEYLESS_PROVIDERS))
    parser.add_argument("--dataset", choices=sorted(DATASETS), default="scifact")
    parser.add_argument("--config", type=Path)
    parser.add_argument("--query-count", type=int, default=10)
    parser.add_argument("--repetitions", type=int, default=2)
    parser.add_argument("--seed", default="retrieval-quality-live-v1")
    parser.add_argument("--max-results", type=int, default=5)
    parser.add_argument("--timeout-seconds", type=int, default=18)
    parser.add_argument("--max-provider-queries", type=int, default=80)
    args = parser.parse_args()

    if not re.fullmatch(r"[0-9a-fA-F]{40}", args.source_sha):
        parser.error("--source-sha must be an exact 40-character commit SHA")
    if not args.binary.is_file():
        parser.error("--binary must identify an existing executable file")
    if args.config and not args.config.is_file():
        parser.error("--config must identify an existing file")
    if not 1 <= args.query_count <= 40 or not 1 <= args.repetitions <= 3:
        parser.error("--query-count must be 1..40 and --repetitions must be 1..3")
    if not 1 <= args.max_results <= MAX_RESULTS or not 1 <= args.timeout_seconds <= 30:
        parser.error("--max-results must be 1..10 and --timeout-seconds must be 1..30")
    if args.query_count * args.repetitions > args.max_provider_queries:
        parser.error("query_count x repetitions exceeds the provider-query ceiling")
    if args.max_provider_queries > MAX_PROVIDER_QUERIES:
        parser.error(f"--max-provider-queries cannot exceed {MAX_PROVIDER_QUERIES}")

    corpus, corpus_sha, dataset_manifest = load_corpus(args.dataset)
    selected = select_cases(corpus["cases"], args.query_count, args.seed)
    binary_sha = digest(args.binary.read_bytes())
    rows = []
    for repetition in range(1, args.repetitions + 1):
        for case in selected:
            rows.append(
                run_one(
                    args.binary,
                    args.config,
                    args.provider,
                    case,
                    repetition,
                    args.max_results,
                    args.timeout_seconds,
                )
            )

    expected_documents = sum(len(case["relevant_documents"]) for case in selected)
    valid_rows = [row for row in rows if row["outcome"] in {"results", "zero_results"}]
    known_documents_in_valid_trials = sum(row["relevant_document_count"] for row in valid_rows)
    found_documents = sum(row["matched_relevant_document_count"] for row in valid_rows)
    hit_rows = sum(bool(row["matched_relevant_documents"]) for row in valid_rows)
    macro_recall = (
        sum(row["matched_relevant_document_count"] / row["relevant_document_count"] for row in valid_rows)
        / len(valid_rows)
        if valid_rows
        else None
    )
    report = {
        "schema_version": 1,
        "dataset": corpus["dataset"],
        "dataset_sha256": corpus_sha,
        "dataset_archive_sha256": dataset_manifest["archive_sha256"],
        "source_component_sha256": dataset_manifest.get("source_fingerprints", {}),
        "source_sha": args.source_sha.lower(),
        "runner_sha256": digest(Path(__file__).read_bytes()),
        "binary_sha256": binary_sha,
        "host": platform.platform(),
        "provider": args.provider,
        "config_state": "explicit config supplied" if args.config else "binary defaults",
        "cache_state": "uncontrolled; CLI provides no cache-state control",
        "seed": args.seed,
        "query_ids": [case["id"] for case in selected],
        "query_count": len(selected),
        "repetitions": args.repetitions,
        "provider_query_attempts": len(rows),
        "max_results": args.max_results,
        "timeout_seconds_per_attempt": args.timeout_seconds,
        "outcomes": {outcome: sum(row["outcome"] == outcome for row in rows) for outcome in sorted({r["outcome"] for r in rows})},
        "judgment_denominators": {
            "selected_queries": len(selected),
            "repeated_valid_result_query_trials": len(valid_rows),
            "known_relevant_query_document_pairs_per_cohort": expected_documents,
            "known_relevant_query_document_pairs_across_valid_trials": known_documents_in_valid_trials,
            "matched_relevant_query_document_pairs_across_valid_trials": found_documents,
        },
        "hit_at_k_over_valid_result_trials": hit_rows / len(valid_rows) if valid_rows else None,
        "macro_recall_at_k_over_valid_result_trials": macro_recall,
        "micro_recall_at_k_over_valid_result_trials": (
            found_documents / known_documents_in_valid_trials if known_documents_in_valid_trials else None
        ),
        "rows": rows,
        "raw_search_results_included": False,
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
