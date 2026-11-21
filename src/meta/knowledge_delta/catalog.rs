use std::collections::HashSet;
use std::fmt;
use std::sync::OnceLock;

use crate::core::knowledge_delta::{parse_calendar_date, DatePrecision, SnapshotRow};

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    pub model_id: String,
    pub aliases: Vec<String>,
    pub date: String,
    pub precision: String,
    pub source: String,
    pub source_url: String,
    pub verified_at: String,
    pub origin: String,
    pub status: String,
    pub note: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(transparent)]
pub struct Catalog(pub Vec<CatalogEntry>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogProblem {
    pub model_id: String,
    message: String,
}

impl CatalogProblem {
    fn new(model_id: &str, details: impl fmt::Display) -> Self {
        Self {
            model_id: model_id.to_string(),
            message: format!("model_id '{model_id}': {details}"),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CatalogProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Catalog {
    pub fn embedded() -> &'static Self {
        static CATALOG: OnceLock<Catalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            serde_json::from_str(include_str!("catalog.json"))
                .expect("bundled knowledge-delta catalog JSON must parse")
        })
    }

    pub fn verified_rows(&self) -> impl Iterator<Item = &CatalogEntry> {
        self.0.iter().filter(|entry| entry.status == "verified")
    }

    pub fn validate(&self) -> Result<(), Vec<CatalogProblem>> {
        let mut problems = Vec::new();
        if self.0.len() > 128 {
            let model_id = self
                .0
                .get(128)
                .map_or("<catalog>", |entry| entry.model_id.as_str());
            problems.push(CatalogProblem::new(
                model_id,
                format!("catalog has {} entries; maximum is 128", self.0.len()),
            ));
        }

        let mut model_ids = HashSet::new();
        let mut aliases = HashSet::new();
        let mut previous_model_id = None;
        for entry in &self.0 {
            if entry.model_id.trim().is_empty() {
                problems.push(CatalogProblem::new(
                    &entry.model_id,
                    "model_id must be non-empty",
                ));
            }
            if !model_ids.insert(entry.model_id.as_str()) {
                problems.push(CatalogProblem::new(
                    &entry.model_id,
                    "model_id is duplicated",
                ));
            }
            if previous_model_id.is_some_and(|previous| previous > entry.model_id.as_str()) {
                problems.push(CatalogProblem::new(
                    &entry.model_id,
                    "entries must be sorted by model_id",
                ));
            }
            previous_model_id = Some(entry.model_id.as_str());

            if entry.aliases.len() > 8 {
                problems.push(CatalogProblem::new(
                    &entry.model_id,
                    format!("entry has {} aliases; maximum is 8", entry.aliases.len()),
                ));
            }
            for alias in &entry.aliases {
                if alias.trim().is_empty()
                    || alias.chars().count() > 80
                    || alias.chars().any(char::is_whitespace)
                {
                    problems.push(CatalogProblem::new(
                        &entry.model_id,
                        format!("alias '{alias}' must be non-empty, at most 80 characters, and contain no whitespace"),
                    ));
                }
                if !aliases.insert(alias.as_str()) {
                    problems.push(CatalogProblem::new(
                        &entry.model_id,
                        format!("alias '{alias}' is duplicated across the catalog"),
                    ));
                }
            }

            if !matches!(
                entry.status.as_str(),
                "verified" | "no_published_cutoff" | "unresolved_conflict" | "alias_unconfirmed"
            ) {
                problems.push(CatalogProblem::new(
                    &entry.model_id,
                    format!("unsupported status '{}'", entry.status),
                ));
            }
            if !matches!(
                entry.origin.as_str(),
                "provider_official" | "models_dev" | "openrouter"
            ) {
                problems.push(CatalogProblem::new(
                    &entry.model_id,
                    format!("unsupported origin '{}'", entry.origin),
                ));
            }

            let is_verified = entry.status == "verified";
            match DatePrecision::parse(&entry.precision)
                .filter(|precision| precision.as_str() == entry.precision)
            {
                Some(DatePrecision::Unknown) => {
                    if is_verified || !entry.date.is_empty() {
                        problems.push(CatalogProblem::new(
                            &entry.model_id,
                            "precision 'unknown' requires a non-verified entry with an empty date",
                        ));
                    }
                }
                Some(precision) => {
                    let valid_date = parse_calendar_date(&entry.date, precision)
                        .and_then(|date| date.normalized())
                        .is_some_and(|date| date == entry.date);
                    if !valid_date {
                        problems.push(CatalogProblem::new(
                            &entry.model_id,
                            format!(
                                "date '{}' does not parse at precision '{}'",
                                entry.date, entry.precision
                            ),
                        ));
                    }
                }
                None => problems.push(CatalogProblem::new(
                    &entry.model_id,
                    format!("unsupported precision '{}'", entry.precision),
                )),
            }

            if entry.note.chars().count() > 200 {
                problems.push(CatalogProblem::new(
                    &entry.model_id,
                    "note exceeds 200 characters",
                ));
            }
            if is_verified {
                if entry.source.trim().is_empty() {
                    problems.push(CatalogProblem::new(
                        &entry.model_id,
                        "verified entry requires a non-empty source",
                    ));
                }
                if !entry.source_url.starts_with("https://") || entry.source_url.len() <= 8 {
                    problems.push(CatalogProblem::new(
                        &entry.model_id,
                        "verified entry requires an https source_url",
                    ));
                }
                if !valid_verified_at(&entry.verified_at) {
                    problems.push(CatalogProblem::new(
                        &entry.model_id,
                        "verified entry requires a valid verified_at date in YYYY-MM-DD format",
                    ));
                }
            }
        }

        for entry in &self.0 {
            for alias in &entry.aliases {
                if alias != &entry.model_id && model_ids.contains(alias.as_str()) {
                    problems.push(CatalogProblem::new(
                        &entry.model_id,
                        format!("alias '{alias}' shadows another entry's model_id"),
                    ));
                }
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }

    pub fn verify_snapshot(&self, snapshot: &[SnapshotRow]) -> Result<(), Vec<CatalogProblem>> {
        let verified_entries: Vec<_> = self.verified_rows().collect();
        let mut problems = Vec::new();
        let mut snapshot_ids = HashSet::new();
        for row in snapshot {
            if !snapshot_ids.insert(row.model_id) {
                problems.push(CatalogProblem::new(
                    row.model_id,
                    "snapshot model_id is duplicated",
                ));
            }
        }

        for entry in &verified_entries {
            let Some(row) = snapshot.iter().find(|row| row.model_id == entry.model_id) else {
                problems.push(CatalogProblem::new(
                    &entry.model_id,
                    "verified catalog entry is missing from the bundled snapshot",
                ));
                continue;
            };
            let mut mismatches = Vec::new();
            if row.aliases.len() != entry.aliases.len()
                || !row
                    .aliases
                    .iter()
                    .zip(&entry.aliases)
                    .all(|(row_alias, alias)| *row_alias == alias.as_str())
            {
                mismatches.push("aliases");
            }
            if row.date != entry.date {
                mismatches.push("date");
            }
            if row.precision.as_str() != entry.precision {
                mismatches.push("precision");
            }
            if row.source != entry.source {
                mismatches.push("source");
            }
            if row.source_url != entry.source_url {
                mismatches.push("source_url");
            }
            if row.verified_at != entry.verified_at {
                mismatches.push("verified_at");
            }
            if row.origin.as_str() != entry.origin {
                mismatches.push("origin");
            }
            if !mismatches.is_empty() {
                problems.push(CatalogProblem::new(
                    &entry.model_id,
                    format!("bundled snapshot differs in {}", mismatches.join(", ")),
                ));
            }
        }

        for row in snapshot {
            if !verified_entries
                .iter()
                .any(|entry| entry.model_id == row.model_id)
            {
                problems.push(CatalogProblem::new(
                    row.model_id,
                    "snapshot row has no matching verified catalog entry",
                ));
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }
}

fn valid_verified_at(raw: &str) -> bool {
    if raw.len() != 10 {
        return false;
    }
    raw.bytes().enumerate().all(|(index, byte)| match index {
        4 | 7 => byte == b'-',
        _ => byte.is_ascii_digit(),
    }) && parse_calendar_date(raw, DatePrecision::Day).is_some()
}
