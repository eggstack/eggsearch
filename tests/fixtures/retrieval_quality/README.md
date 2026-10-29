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
