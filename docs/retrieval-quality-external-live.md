# Expanded retrieval-quality live evidence

This is a later opt-in evidence window for the corrective M007 workstream. It
does not replace the closed M006 comparison or change its conclusions. Live
results are time-local diagnostics; live result content is not committed.

## Independently judged query corpus

`tests/fixtures/retrieval_quality/scifact_test_queries.jsonl` and
`tests/fixtures/retrieval_quality/scifact_test_qrels.jsonl` contain all 300
SciFact BEIR test queries and 339 positive query/document judgments. The
queries and judgments predate Eggsearch and were produced independently by the
SciFact authors. SciFact describes expert-written claims paired with evidence
abstracts and labels/rationales. The imported files contain query text, paper
IDs, titles, and qrels only; they omit abstracts and evidence passages. Query
claims/evidence annotations are CC BY 4.0; source paper titles are attributed to
the Semantic Scholar Open Research Corpus, licensed ODC-By 1.0. The exact BEIR
archive SHA-256 and further attribution are recorded in
`tests/fixtures/retrieval_quality/scifact_test_corpus.md`. See the [SciFact paper](https://aclanthology.org/2020.emnlp-main.609/),
[SciFact license](https://github.com/allenai/scifact/blob/master/LICENSE.md),
and [BEIR dataset listing](https://github.com/beir-cellar/beir/wiki/Datasets-available).

This corpus is held out from Eggsearch ranking work, but its claims cover
scientific fact verification only. Its judgments are positive evidence links,
not complete judgments over every document a live web engine may return. The
operational evaluator therefore reports title-based hit@k and recall over
known positive query/document pairs on successful result and zero-result
responses; it does not score unjudged results as irrelevant or claim broad
web-search quality. Provider errors and timeouts are excluded from quality
scores and retained as separate outcomes.

The opt-in runner uses the current Eggsearch CLI and records query IDs,
provider outcomes, matching relevant paper IDs/ranks, latency, and response
hashes. It never writes result titles, URLs, snippets, credentials, or query
text into the emitted report. Run it manually with a built candidate binary:

```bash
python3 scripts/run_retrieval_quality_external_live.py \
  --binary target/debug/eggsearch \
  --source-sha <exact-40-character-source-sha> \
  --provider duckduckgo \
  --query-count 20 \
  --repetitions 2 \
  --seed m007-cohort-001 \
  --max-results 5 \
  --max-provider-queries 40
```

The hard ceiling is 80 provider-query attempts, with at most 40 selected
queries, three repetitions, ten results, and 30 seconds per process. The
provider allowlist excludes credentialed/API providers. SearXNG requires an
explicit config with that provider selected; the config itself is not copied
into evidence. Cache state is uncontrolled because the CLI exposes no cache
control. Live reports are not routine CI inputs.

## Observations on candidate `92b0a27737e21fb42cdcf8a984cab6a04931121f`

The initial DuckDuckGo cohorts used an earlier evaluator that excluded zero-result
responses from scoring. Across the two frozen five-query cohorts (20 attempts),
all responses were zero-result; reports 003 and 004 preserve those historical
observations with their original runner identity. The corrected evaluator now
counts successful zero-result responses as misses and excludes provider errors
and timeouts. In its final repeated five-query cohort, DuckDuckGo returned two
zero-result responses and eight HTTP errors. The two valid trials matched no
known relevant titles (hit@5 and macro/micro recall 0.0; denominator: two valid
trials). This is too little usable data for a quality conclusion. See
`plans/closure/retrieval-quality-evidence-extension/evidence/scifact-ddg-cohort-005.json`.

A separate two-query diagnostic repeated each of the three enabled keyless
HTML providers twice (12 attempts total). DuckDuckGo returned five cards in all
four attempts, Startpage returned successful zero-result responses in all four,
and Yahoo returned HTTP 500 errors in all four. This small diagnostic cohort
does not rank providers. Sanitized outcomes and response hashes:
`plans/closure/retrieval-quality-evidence-extension/evidence/keyless-live-observation-001.json`.

## SearXNG and extraction arms

The local Eggsearch config had no SearXNG endpoint. Two probes against
instances listed by the public [SearXNG instance directory](https://searx.space/)
were unusable: one returned non-JSON content to the JSON API request and the
other returned HTTP 429. Since JSON is an instance-controlled format, a
temporary local SearXNG instance was installed from the official source in a
Python 3.11.15 virtual environment under `/tmp`, with JSON enabled and the
server bound only to `127.0.0.1`. It used SearXNG
`2026.10.29+c15647b` and four selected keyless engines: arXiv, Brave,
DuckDuckGo, and Wikipedia. Eggsearch called it through the configured
`searxng` adapter.

The local arm used the same five SciFact queries twice (10 Eggsearch-to-SearXNG
calls). In the earlier run, five calls returned cards and five returned no
cards. The corrected final cohort returned five cards in all 10 calls, with no
known relevant title matches (hit@5 and macro/micro recall 0.0 over 10 trials).
The SearXNG server logs showed DuckDuckGo CAPTCHA responses, Brave HTTP 429
responses, and result URL callback warnings. No challenge was bypassed. This
is an actual SearXNG baseline, but upstream throttling and the five-query sample
prevent any broad quality comparison. Configuration fingerprint, source
revision, upstream engine set, and trial metadata are in
`plans/closure/retrieval-quality-evidence-extension/evidence/searxng-local-instance-001.json`,
`.../scifact-searxng-cohort-001.json`, and
`.../scifact-searxng-cohort-002.json`.

Paired live extraction now covers the same official RFC 9110 HTML document in
HTTP-only and real Chrome browser modes, plus its official PDF representation
through the `pdf browser` feature build. All three produced sections 15.5.5 and
15.5.11 with the expected 404 and 410 status phrases. HTML HTTP and browser
outputs each returned five focused chunks (3,811 characters); the PDF returned
one 6,000-character focused chunk. The HTML source is server-rendered, so the
browser arm measures real execution and overhead but does not test JS-only
content. The PDF is text based and does not test OCR. Chrome was 156.0.8078.48;
`lopdf` was 0.42.0. Full response text is not committed; bounded response
hashes and extraction metadata are in
`plans/closure/retrieval-quality-evidence-extension/evidence/rfc9110-extraction-pair-001.json`.

The first PDF path attempted returned 404 and was corrected to the official
RFC Editor PDF URL, `https://www.rfc-editor.org/rfc/rfc9110.pdf`. The initial
5,000-character budget did not reach the target sections in the long HTML
document. The accepted paired run uses a shared 400,000-character request cap
and the same focus query for HTTP and browser extraction; the PDF uses the same
cap and focus query. Existing M002 four-build results remain historical.

## CodeGG consumer acceptance

Three independently verified OpenCode Go `muse-spark-1.3-contributor` CodeGG
turns completed on the exact CodeGG binary and Eggsearch sidecar identities
recorded in `plans/closure/eggsearch-retrieval-quality-consumer-acceptance/001-status.md`.
The clean isolated CodeGG checkout also passed its quick gate. These turns
verify one task's search/fetch/evidence-bundle path; they do not establish
provider attribution, comparative schema-byte savings in CodeGG, or general
retrieval lift.

## Disposition

At the time of this initial evidence report, M007 remained active. Subsequent
cross-domain, paired extraction/resource, and CodeGG evidence is recorded in
`docs/retrieval-quality-cross-domain-live.md`,
`docs/retrieval-quality-extraction-qualification.md`, and
`docs/retrieval-quality-consumer-acceptance.md`. The milestone is now closing
pending hosted Linux, macOS, and Windows CI on the final merged candidate. No
ranking, routing, fetch policy, or feature default changed.
