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
