# Verified model knowledge cutoffs

The catalog is the offline, provenance-bearing source for model knowledge-cutoff resolution. Its manifest lives at `src/meta/knowledge_delta/catalog.json`.

`src/meta/knowledge_delta/catalog.json` contains the checked-in records; `src/meta/knowledge_delta/snapshot.rs` contains only records whose status is `verified`. The catalog is parsed once from the embedded manifest. Runtime resolution does not fetch model metadata.

As of 2026-11-20, 11 of 16 listed model IDs have verified official cutoffs across OpenAI, Anthropic, and xAI. Other listed IDs are explicitly recorded as having no published cutoff; they are not added to the bundled resolver snapshot.

## Statuses

- `verified`: an official source explicitly states a model cutoff at the recorded precision. Only these entries may appear in the bundled snapshot.
- `no_published_cutoff`: an official model page was checked but publishes no cutoff; date is empty and precision is `unknown`.
- `unresolved_conflict`: official sources disagree; no disputed value is used for resolution.
- `alias_unconfirmed`: a possible alias cannot be tied to a canonical model ID from an official source.

Non-verified entries never populate the resolver snapshot. The current reviewed sources did not produce a cutoff conflict or a required alias-unconfirmed record.

## Resolution precedence

Resolution uses caller override > operator config > provider_official > models_dev > openrouter > unknown. Within a bundled source tier, the model ID or a listed alias must match. Callers can provide a cutoff when a provider has not published one.

## Verification procedure

1. Start from an official provider model index or a source URL already established by one. Confirm the exact model ID on the official source.
2. Record a cutoff only when the official page states it verbatim. Do not infer from release dates, version numbers, publication dates, or secondary catalog values.
3. Preserve source granularity: a month-only statement is recorded as `YYYY-MM` with `month` precision; a day statement is `YYYY-MM-DD` with `day` precision. Year-only statements use `year`.
4. Put only the short exact cutoff wording or compact table row in `note`; record the canonical HTTPS page URL and the verification date in `verified_at`.
5. If no cutoff is published, use `no_published_cutoff`, empty `date`, and `unknown` precision. If official sources disagree, use `unresolved_conflict` and cite both URLs in the short note. Keep unconfirmed names out of aliases.

## Adding or superseding an entry

Add or update the model record in `catalog.json`, keeping entries sorted by `model_id`. Keep aliases exact and unique across IDs. A verified entry must be copied field-for-field into `snapshot.rs`; non-verified entries must not be copied there. When a source changes, recheck the official page and update the manifest and snapshot together, preserving the source-backed precision. Run the offline validator and formatting check:

`RUSTUP_TOOLCHAIN=1.89.0 cargo test --locked --test knowledge_delta_catalog`

`RUSTUP_TOOLCHAIN=1.89.0 cargo fmt`

## Redistribution and terms

The manifest stores normalized model IDs, dates, precision, status, source links, and compact evidence notes rather than model descriptions or copied documentation. Keep evidence to the minimum short citation needed to support the fact, and review the source provider's current terms before redistribution.

## Coverage

The table below mirrors every record in `catalog.json` as of 2026-11-20. “Not published” means the official page was checked without a cutoff value; it is not a cutoff date.

| Model ID | Cutoff | Precision | Origin | Status |
| --- | --- | --- | --- | --- |
| `claude-fable-5-2` | 2026-06 | month | provider_official | verified |
| `claude-haiku-5-5` | 2026-06 | month | provider_official | verified |
| `claude-opus-5-5` | 2026-06 | month | provider_official | verified |
| `claude-sonnet-5-5` | 2026-06 | month | provider_official | verified |
| `deepseek-flash` | not published | unknown | provider_official | no_published_cutoff |
| `deepseek-v4-pro` | not published | unknown | provider_official | no_published_cutoff |
| `gemini-4-carbon` | not published | unknown | provider_official | no_published_cutoff |
| `glm-5.2` | not published | unknown | provider_official | no_published_cutoff |
| `glm-5.3` | not published | unknown | provider_official | no_published_cutoff |
| `gpt-5.5` | 2025-12-01 | day | provider_official | verified |
| `gpt-6-astra` | 2026-04-30 | day | provider_official | verified |
| `gpt-6-luna` | 2026-05-18 | day | provider_official | verified |
| `gpt-6.1-terra` | 2026-06-01 | day | provider_official | verified |
| `gpt-6.2-astra` | 2026-05-18 | day | provider_official | verified |
| `gpt-6.2-sol` | 2026-06-01 | day | provider_official | verified |
| `grok-4.7` | 2026-05 | month | provider_official | verified |
