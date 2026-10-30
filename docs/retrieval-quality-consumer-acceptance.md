# M007 CodeGG consumer acceptance — 001

Status: multi-category task acceptance passed; provider attribution and exact
schema-byte measurement remain unavailable.

An additional verified turn was accepted on the explicitly authorized
`opencode_go/muse-spark-1.3-contributor` route. It used the existing CodeGG
Harness Lab, a separate FEVER claim case, the isolated CodeGG binary at
`f3c0a9e2e9903bb097d8bf56641138e03d964eb7`, and Eggsearch 0.4.4 sidecar SHA-256
`d80b5c22f6e31d3bf9df8934544b737d8df0bf657268cfabeac75b449005a2e3`. The
independent verifier confirmed the supported claim, the 1984 film identity,
Stephen Frears as director, search-to-fetch, one successful evidence bundle,
and the source's external-untrusted trust framing.

The accepted FEVER turn used 16 provider requests, 35,672 input tokens, 198
output tokens, and $0.004684 reported cost. Three earlier verified RFC 9110
turns used 24 requests and $0.018303 reported cost. Across the two cases, four
accepted turns used 40 requests, 144,936 input tokens, 1,047 output tokens,
and $0.022987 reported cost. The initial FEVER attempt is excluded: it spent
$0.004862 and failed its first verifier revision, which required an exact prose
phrase the answer did not use; its tool trajectory also included a failed
evidence-bundle call before a successful one. The verifier was corrected to
accept either serialization of the same trust marker, and the next bounded
turn passed without weakening the observed tool-sequence checks.

The bounded receipt does not expose serialized tool-definition bytes at the
CodeGG/provider boundary, and the search-tool receipt does not attribute the
retrieved result to an individual upstream provider. Eggsearch-side full/core
tool-definition byte estimates therefore remain separate from this consumer
acceptance. No provider ranking or consumer-quality superiority claim follows
from four successful turns.

Compact receipt hashes, exact fixture/verifier fingerprints, usage, and
limitations are recorded in
`plans/closure/retrieval-quality-evidence-extension/evidence/codegg-multi-category-acceptance-001.json`.
