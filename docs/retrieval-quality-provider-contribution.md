# Keyless Provider Contribution and Resilience

M003 evaluated provider contribution with a controlled synthetic fixture and existing mock-provider fault suites. No live provider query was made. The fixture assigns documents to providers to exercise the evaluator and is not an observation of DuckDuckGo, Startpage, Yahoo, Brave, Mojeek, or SearXNG output.

## Offline fixture result

Task `generic-web-provider-overlap-001` is one `generic_web` task with three judged relevant documents. The selected default-keyless provider set is DuckDuckGo, Startpage and Yahoo.

| Provider | Relevant fixture documents | Individual relevant hits | Outcome |
|---|---|---:|---|
| DuckDuckGo | `compression-guide` | 1/3 | success |
| Startpage | `compression-guide`, `compression-configuration` | 2/3 | success |
| Yahoo | `compression-protocol`, `compression-configuration` | 2/3 | success |
| Combined set | all three unique relevant documents | 3/3 recall | success |

Two relevant documents overlap across provider envelopes. The fixed provider order is recorded by the fixture. Because assignments are synthetic, these ratios validate only deterministic marginal/union arithmetic; they do not support provider ranking or staged dispatch.

Brave HTML and Mojeek are represented as `not_selected`; SearXNG is `not_configured`. They have no measured denominator and no quality score. Paid/API references are not part of this run.

## Failure conditioning and routing decision

The existing `dispatch_fault_injection`, `provider_routing`, `provider_probe_conformance`, `web_search_integration`, and `provider_request_contract` suites cover explicit selection, timeouts, provider errors, cooldown/isolation, empty success, partial results and request construction. Results: 193 passed across the first four suites; 36 passed in provider request contracts. The M001 corpus separately distinguishes expected absence from a simulated provider failure, and its degraded-ranking guard rejects a known quality regression.

There is no provider-attributed offline snapshot corpus with real response envelopes, and no opt-in live observation was run. Therefore latency, marginal recall, overlap and timeout impact for real upstreams remain unavailable. That evidence does not justify a staged scheduler or suppression rule. Production fan-out, explicit provider selection, cooldown and ledger behavior remain unchanged.

To collect live evidence later, use the ignored M001 live observation with an explicit provider/task allowlist and report each provider's selected/failed/skipped status, response hash/bytes, latency, unique judged hits and billable calls. A live failure must remain a failure observation and must not be converted to zero-results absence.
