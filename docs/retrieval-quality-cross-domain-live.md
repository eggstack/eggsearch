# M007 cross-domain live observation — 001

Status: partial operational qualification; no quality ranking or provider winner.

## Candidate and corpus identity

- Eggsearch source: `28545de1fa899281f41980f4de31046df3431f22`
- Release binary SHA-256: `46f309a2649fdd781d77e3459e7868f0c35e76e760470a558af52e56d6d5ca52`
- Runner source SHA-256: `2bb24640027801e9c29f2839e1f8e84b8007a66a3ceb4cb7d489e8f4c72767d3`
- Host: macOS 26.6.2, x86_64
- Fixed seed: `m007-cross-domain-provider-v1`; three selected queries per dataset, two repetitions each, ten results maximum, 18 second per-attempt timeout.
- Each report stores exact query IDs, source and binary fingerprints, per-result hashes, per-query outcomes, latencies, and positive-judgment denominators. Cache state is uncontrolled by the CLI.

## Outcomes

Each provider/dataset row below represents six attempts over the same three selected query IDs. A successful empty response is kept distinct from a provider error. For all successful-result or successful-empty rows, no relevant page matched by URL or normalized title.

| Dataset | DuckDuckGo | Startpage | Yahoo | Local SearXNG |
|---|---|---|---|---|
| SciFact | 6 successful empty | 6 successful empty | 6 provider errors | 6 result responses; 0 matched positives |
| NQ | 6 successful empty | 6 successful empty | 6 provider errors | 6 result responses; 0 matched positives |
| FEVER | 6 successful empty | 6 successful empty | 6 provider errors | 6 result responses; 0 matched positives |

The successful-empty rows have hit@10 and macro recall@10 of 0 across six valid query trials. SearXNG returned results for every trial, with 0 matches over the 18 repeated query trials. Yahoo has no valid retrieval denominator because all attempts failed. These are small, positive-only qrel cohorts; unjudged results are not treated as irrelevant and the observations do not establish general provider quality.

Median process latency per six-attempt cohort was 238/229/228 ms for DuckDuckGo, 250/237/239 ms for Startpage, 1,071/1,058/1,071 ms for Yahoo, and 96/80/101 ms for SearXNG (SciFact/NQ/FEVER order). These are CLI process durations, not time-to-first-useful-text or cache-controlled fetch measurements.

Touché 2020 is retained as the fourth published task family but is omitted from live scoring: its qrels use opaque argument IDs and safe live-result matching would require retaining document metadata whose rights are not needed for this opt-in cohort.

## SearXNG baseline identity

The baseline was a real local SearXNG 2026.10.29 instance at loopback with JSON enabled. Its settings fingerprint, Eggsearch adapter configuration fingerprint, source revision, and selected engine list are recorded in `plans/closure/retrieval-quality-evidence-extension/evidence/searxng-cross-domain-instance-001.json`. The selected engines were arxiv, Brave, DuckDuckGo, and Wikipedia. Runtime logs showed DuckDuckGo CAPTCHA responses and SearXNG URL callback warnings; result-producing trials therefore do not identify which configured upstream supplied each result. The temporary server was stopped after the cohort.

## Per-run evidence

The exact dataset/provider reports are in `plans/closure/retrieval-quality-evidence-extension/evidence/` as `*-cross-domain-001.json`. Earlier SciFact-only trials remain separate historical observations. No result ranking, provider schedule, or retrieval policy was changed.
