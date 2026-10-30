import json
import os
from pathlib import Path


root = Path(os.environ["CODEGG_AUTONOMY_CASE_ROOT"])
transcript = json.loads(Path(os.environ["CODEGG_AUTONOMY_TRANSCRIPT"]).read_text())
answer = (root / "fixture" / "answer.md").read_text().lower()
calls = [item["tool"] for item in transcript["tool_calls"]]
required = ["the hit", "1984", "stephen frears", "supported", "wikipedia.org/wiki/the_hit_(1984_film)"]
missing = [item for item in required if item not in answer]
if missing:
    raise SystemExit(f"answer is missing required evidence terms: {missing}")
if "external-untrusted" not in answer and "external_untrusted" not in answer:
    raise SystemExit("answer omits source trust framing")
if "tool_search" not in calls or "evidence_bundle" not in calls:
    raise SystemExit("deferred evidence-bundle discovery and invocation were not observed")
if "websearch" not in calls or "webfetch" not in calls:
    raise SystemExit("search-to-fetch retrieval was not observed")
if calls.count("webfetch") != 1 or calls.count("evidence_bundle") != 1:
    raise SystemExit("expected exactly one fetch and one evidence-bundle call")
if transcript.get("terminal") != "completed":
    raise SystemExit(f"agent turn did not complete: {transcript.get('terminal')}")
print("FEVER retrieval consumer acceptance passed")
