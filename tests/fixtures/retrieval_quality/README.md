# Retrieval quality reference corpus

Schema version 1 lives in `cases.json`. Every result envelope and passage is
synthetic and written for this corpus; it contains no copied upstream text.
The fixture is therefore suitable for redistribution and does not imply that
any provider returned the result.

Each task has a stable ID, category, query and retrieval constraints. Judgments
identify relevant documents and passages by stable fixture identity, include
canonical URL aliases, and label expected retrieval state. The corpus includes
deliberate negatives and an adversarial prompt-injection string as inert source
content. `judgments_complete` controls whether unjudged results enter precision
and nDCG denominators.

The cases are a contract corpus, not an unbiased estimate of production web
quality. They have one synthetic annotation author and no independent
adjudication; scores must not be presented as live provider quality claims.
Held-out cases are explicitly marked `held_out`; they are not tuning guidance.

## External held-out cohorts

`scifact`, `nq`, `fever`, and `touche2020` are separate published test cohorts
for opt-in live observations. Each has its own query and positive-qrel JSONL
file. They retain only question/claim text and relevant document identifiers,
titles, and canonical URLs where supported by the source terms; no source
passages or corpus bodies are included.
The positive-only judgments mean an unlisted result is unjudged, not
irrelevant. Exact URL matching is attempted before normalized title matching.
These data are not four independently authored annotation projects: the
BEIR mappings inherit the judgment semantics and limitations of their source
datasets. Live title/URL matches are lower-bound observations, not complete
web relevance scores.

### Natural Questions (NQ)

The cohort is 100 deterministically selected queries from the BEIR NQ test
split, after excluding first-person/personal-form questions. It retains 100
positive query/document pairs spanning 100 distinct relevant pages. The complete BEIR archive SHA-256 is
`2553bf7bfbab47b1436ca00a34bce57320e18e611fd00999a2a3a1b4714be752`; the
source fields were streamed to extract only selected question text and
relevant Wikipedia page titles. Google releases Natural Questions under
CC BY-SA 3.0. The test set has five human annotations per example; the BEIR
mapping supplies positive retrieval qrels for this cohort. The deterministic
selection seed is `m007-nq-test-cohort-v1`.

Sources: [Natural Questions release and license](https://ai.google.com/research/NaturalQuestions/download),
[Natural Questions paper](https://aclanthology.org/Q19-1026/),
[BEIR dataset mapping](https://github.com/beir-cellar/beir/wiki/Datasets-available).

### FEVER

The cohort is 100 deterministically selected claims from BEIR's FEVER test
mapping, with 120 positive claim/page pairs. The queries source is the BEIR
`BeIR/fever` queries Parquet file (SHA-256
`b9cf00001ef1aa07a7030118490ce5f8a840e783ad8d972b9afee8ef281d6093`); qrels
are from `BeIR/fever-qrels` `test.tsv` (SHA-256
`6599dab4e2189ca5eb5216546021710c06dc8bdd1a0c59cc1fa52129c6e038b3`); the
pinned Hub revisions are `426aea4d30c65c041f093ae072e25fb5a0e64659` and
`64cd0e4ef9b63a88ac8e69d1133a2f649acd4745`, respectively. No
corpus file was imported. FEVER claims, annotations, and Wikipedia-derived
metadata are subject to the applicable Wikipedia license terms, or CC BY-SA
3.0 where article-specific terms do not apply. Titles and URLs are constructed
from the qrel page identifiers. The qrels derive from FEVER evidence pages;
they do not constitute exhaustive web-search judgments. Selection seed:
`m007-fever-test-v1`.

Sources: [FEVER data description](https://fever.ai/dataset/fever.html),
[FEVER license](https://fever.ai/download/fever/license.html),
[BEIR FEVER queries](https://huggingface.co/datasets/BeIR/fever),
[BEIR FEVER qrels](https://huggingface.co/datasets/BeIR/fever-qrels).

### Touché 2020

The cohort includes all 49 test topics and 932 positive query/document
judgments from the BEIR Touché 2020 archive (SHA-256
`1a706ed9c03ecc75cf012495bd8e37b761f7b0350a939910d0f54862e26fc601`). Only
topic text and opaque document IDs are retained. The Touché
topic/qrel release is CC BY 4.0 (Zenodo DOI `10.5281/zenodo.6873564`); the
args.me document collection is CC BY 4.0 with individual content rights still
applicable (Zenodo DOI `10.5281/zenodo.4139439`). No argument text is included.
The positive qrels are judged argument relevance, which is not equivalent to
topical webpage relevance. Opaque IDs cannot be reliably matched to live web
results without retaining source document metadata, so this cohort is
registered as offline task coverage and is not included in live score
comparisons. Query/qrels selection seed: `m007-touche-test-v1`.

Sources: [Touché 2020 topic and qrel record](https://zenodo.org/records/6873564),
[args.me corpus record and rights statement](https://zenodo.org/records/4139439),
[BEIR Touché data mapping](https://github.com/beir-cellar/beir/wiki/Datasets-available).

Every live report records the selected query IDs, local query/qrel fingerprint,
source archive/component fingerprint, seed, provider, executable hash, and
source commit. Dataset families are summarized separately.

## Candidate snapshot corpora

`candidate_snapshots.json` (schema version 2, tier `structured_synthetic`) and
`recorded_snapshots.json` (schema version 1, tier `recorded_real`) record
per-provider candidate lists instead of a single pre-merged result list. Each
case carries a `provider_lists` array of `{ provider_id, results: [{ rank,
title, url }] }`. The offline harness builds the real
`Vec<(provider_id, Vec<SearchResult>)>`, runs the production selection
sequence, and scores the selected rows: RRF fusion into the candidate pool,
URL-keyed deduplication, local domain filtering, bounded intent/freshness
reranking, then truncation to `max_results`. Cases without `provider_lists`
keep scoring `results` exactly as before.

Only provider ids, ranks, titles, and URLs are stored. No page body, snippet,
credential, or personal data is included in either file, so both are
redistributable.

### Authoring rules

- Write `relevant_documents` first, independently of any ranker or of the
  recorded provider order, then record the candidate lists. Grades are 3
  (authoritative primary source), 2 (supporting source), and 1 (partially
  useful). Re-grading a case so that a selector looks better invalidates the
  benchmark.
- `max_results`, `intent`, and `freshness` are explicit per case. The replay
  path requires `freshness: "any"` because recorded rows carry no timestamps;
  a freshness boost would otherwise consult the wall clock and break
  determinism.
- A case states which intent prior it depends on. `authoritative-promotion-001`
  and `code-intent-promotion-001` exercise an explicit caller intent; the four
  `*-derived-promotion-*` cases are the only tier-1 cases whose query must
  classify as the annotated intent, and the corpus test fails if it does not.
- Duplicate clusters must alias through the production dedup key
  `meta::engines::normalizer::normalize`: scheme/host case, fragments, default
  ports, tracking parameters, trailing slashes, locale prefixes, and index
  files collapse. `www.` is *not* collapsed by that key even though
  `core::identity::canonicalize_url` strips it, so a `www.` alias records a
  real duplicate slot rather than a deduplication case.
- Provider ids must be registered providers (`KNOWN_PROVIDER_IDS`).
- Ranks must be contiguous from 1 within each provider list.

### Tier 1 — structured synthetic (`candidate_snapshots.json`)

Eleven cases with controlled structure. Titles and URLs are authored for this
corpus; they are not provider output.

| Case | Structure | Intent prior it depends on |
|------|-----------|----------------------------|
| `rank-inversion-001` | one document at rank 4 and rank 1 across providers, a second at rank 2 and rank 3 | `web` (no lexical signal) |
| `duplicate-cluster-001` | one document reached by three alias URLs from three providers | `web` |
| `domain-concentration-001` | five top slots on one forum host | `web` |
| `provider-agreement-001` | four-provider consensus outranks a single-provider first place | `web` |
| `provider-disagreement-001` | the only relevant document loses to cross-provider consensus | `web` |
| `authoritative-promotion-001` | official docs outside the naive top-5, promoted by pool expansion plus reranking | explicit `docs` (the query derives `web`) |
| `code-intent-promotion-001` | source file outside the naive top-5, promoted by pool expansion plus reranking | explicit `code` (the query derives `web`) |
| `security-derived-promotion-001` | advisory outside the window, promoted by the derived intent | derived `security` |
| `releases-derived-promotion-001` | changelog outside the window, promoted by the derived intent | derived `releases` |
| `docs-derived-promotion-001` | official docs below a generic competitor, promoted to first place | derived `docs` |
| `code-derived-promotion-001` | source file outside the window, promoted by the derived intent | derived `code` |

The four `*-derived-promotion-*` cases exist because the M005 rollout gate was
otherwise vacuous: every earlier query classifies as `web`/`no_signal`, so
reranking with the derived intent is identical to reranking with `intent: web`
on every metric. Each of these four queries carries real lexical evidence for
exactly one intent class (a `CVE-<year>-<id>` identifier, release vocabulary, a
documentation term, and a `Type::method` path), and each places one
intent-appropriate document — `SecurityAdvisory`, `ReleaseNotes`,
`OfficialDocs`, and `SourceFile` per `core::source_card::classify_source_kind` —
where the neutral prior loses to a generic competitor.

The arithmetic is fixed by `apply_intent_reranking`: `boost_unit =
max_base * 0.10`, `+2` units for docs/code/releases/issues/news and `+3` units
for security. In the three window cases a neutral filler appears at rank 1 in
both providers (`2/61 = 0.032787`, the pool maximum), the generic competitor at
`1/62 = 0.016129`, and the intent document at `1/66 = 0.015152`; the boost
carries it to `0.024988` (security) or `0.021709` (releases, code), above the
competitor and inside the final window. In `docs-derived-promotion-001` three
documents appear at mirrored ranks (`2/61`, `2/62`, `2/63`), so the docs page
starts at index 2 below the competitor at index 1 and the boost moves it to
`0.038303`, ahead of both.

`tests/retrieval_quality.rs` asserts that each of these four cases derives the
intent it is annotated with — a classifier miss fails instead of silently
weakening the gate — and that the rank movement above happens in the replay, not
just on paper. The rollout gate then re-derives the applied/observe-only
decision from the recorded metrics.

### Tier 2 — recorded real (`recorded_snapshots.json`)

Intentionally `"cases": []`. The live recorder must populate this tier from an
actual bounded run; nothing here is fabricated. When populated, each case
records the source provider (`provider_lists[].provider_id`), the query, the
UTC observation date (`observed_at`), ranks, titles, and normalized URLs only.
The empty file names the recorder command in its `pending` field, and the
fixture test fails if an empty tier does not name its recorder.

### Tier 3 — untracked recorder output

An ignored test performs a bounded live run and writes a larger snapshot under
`target/snapshots/` (never tracked):

```
EGGSEARCH_RETRIEVAL_LIVE=1 \
EGGSEARCH_RETRIEVAL_PROVIDERS=duckduckgo,startpage \
EGGSEARCH_RETRIEVAL_PAID_PROVIDERS= \
EGGSEARCH_RETRIEVAL_CASES=<candidate_snapshots.json case ids> \
EGGSEARCH_RETRIEVAL_LOCALE=en-US \
EGGSEARCH_BENCH_SOURCE_SHA=<40-hex commit> \
EGGSEARCH_RETRIEVAL_SNAPSHOT_OUT=target/snapshots/candidate-snapshots.json \
cargo test --locked --features mock --test retrieval_quality -- \
  --ignored retrieval_quality_candidate_snapshot_recorder
```

The recorder refuses absolute paths, `.`/`..` components, non-`.json` names, and
any path outside `target/snapshots/`. Each provider is queried separately with
`intent: web` and `freshness: any` so the recorded order remains the provider's
native order, and recording runs with `sanitize_output = false` so stored
titles are provider titles rather than presentation frames. It writes nothing
when no provider returns a candidate row, and reports provider failures in the
output instead of silently substituting synthetic data.

## Fusion metrics

The offline report adds three metrics that fusion, dedup, and reranking defects
move first:

- `duplicate_slots` — selected rows whose normalized-URL cluster already
  appeared earlier in the same top-k (integer total, plus
  `duplicate_slot_rate` over `denominators.top_slots`).
- `distinct_domains_topk` — mean count of distinct hosts in the top-k. Distinct
  from `source_diversity_at_5`, which is the mean distinct-host *ratio*.
- `authoritative_survival` — fraction of grade >= 2 judged documents retained in
  the top-k, over `denominators.authoritative_documents`.

Every metric is deterministic: no wall clock, no randomness, and no hash-map
iteration order in any scored value.
