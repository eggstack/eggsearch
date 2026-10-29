# SciFact BEIR test cohort

This derived cohort includes the 300 SciFact test claims and 339 positive
query/document relevance judgments from the BEIR `scifact` archive. Queries and
qrels are split into `scifact_test_queries.jsonl` and
`scifact_test_qrels.jsonl`; each file is below the repository's ordinary-file
size limit. The archive SHA-256 is
`536e14446a0ba56ed1398ab1055f39fe852686ecad24a6306c80c490fa8e0165`.

The query claims and evidence annotations are from SciFact and are licensed
CC BY 4.0. Paper titles are attributed to Semantic Scholar Open Research
Corpus, licensed ODC-By 1.0. No abstracts or evidence passages are included.
The qrels are published evidence links and provide positive judgments only;
unjudged search results must not be scored as irrelevant.

Sources:

- SciFact paper: <https://aclanthology.org/2020.emnlp-main.609/>
- SciFact data and license: <https://github.com/allenai/scifact>
- SciFact license: <https://github.com/allenai/scifact/blob/master/LICENSE.md>
- BEIR archive listing: <https://github.com/beir-cellar/beir/wiki/Datasets-available>
- Semantic Scholar Open Research Corpus: <https://www.semanticscholar.org/product/open-research-corpus>

The query sample for each run is selected from the fixed test split by sorting
the SHA-256 of `seed:query_id`. The seed and selected IDs are recorded in every
live report. These queries cover scientific fact verification and do not
represent all Eggsearch use cases.
