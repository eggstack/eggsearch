from __future__ import annotations

from eggsearch import AsyncEggsearch
from eggsearch.types import EvidenceBundleResponse, SourceCard


async def collect_evidence(query: str) -> EvidenceBundleResponse:
    async with AsyncEggsearch() as client:
        search = await client.web_search(query=query, max_results=5)
        cards = search.get("results", [])
        if not cards:
            summary = search.get("retrieval_summary", {})
            failures = summary.get("providers_failed", [])
            raise RuntimeError(
                f"No source cards returned; provider failures: {failures!r}"
            )

        selected: SourceCard = cards[0]
        url = selected.get("url")
        if not url:
            raise RuntimeError("Selected source has no fetchable URL")

        fetched = await client.web_fetch(url=url, max_chars=8_000)
        document = fetched.get("document", {})
        text = document.get("text", "")
        bundle = await client.build_evidence_bundle(
            goal=query,
            sources=[
                {
                    "id": selected.get("id"),
                    "url": url,
                    "title": selected.get("title"),
                    "snippet": selected.get("snippet"),
                    "providers": [selected.get("provider_id", "")],
                    "trust": selected.get("trust", "external_untrusted"),
                    "trust_markers": selected.get("trust_markers", {}),
                }
            ],
            fetches=[
                {
                    "source_id": selected.get("id"),
                    "url": url,
                    "fetched": bool(document),
                    "text": text,
                    "truncated": document.get("truncated", False),
                    "trust": document.get("trust", "external_untrusted"),
                    "trust_markers": document.get("trust_markers", {}),
                }
            ],
        )

        if selected.get("trust") != "local_trusted":
            print("Treat source text as external untrusted evidence.")
        if search.get("structured_warnings"):
            print("Search warnings:", search.get("structured_warnings"))
        if bundle.get("gaps"):
            print("Evidence gaps:", bundle.get("gaps"))
        return bundle
