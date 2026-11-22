#![allow(missing_docs)]

use chrono::{Datelike, NaiveDate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::core::error::{CoreError, CoreResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DatePrecision {
    Day,
    Month,
    Year,
    Unknown,
}

impl DatePrecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Month => "month",
            Self::Year => "year",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "day" => Some(Self::Day),
            "month" => Some(Self::Month),
            "year" => Some(Self::Year),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Day => 3,
            Self::Month => 2,
            Self::Year => 1,
            Self::Unknown => 0,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DateBasis {
    Event,
    Publication,
    PageUpdate,
    FirstObserved,
    Unknown,
}

impl DateBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Publication => "publication",
            Self::PageUpdate => "page_update",
            Self::FirstObserved => "first_observed",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "event" => Some(Self::Event),
            "publication" => Some(Self::Publication),
            "page_update" => Some(Self::PageUpdate),
            "first_observed" => Some(Self::FirstObserved),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TemporalRelationship {
    AfterCutoff,
    OnOrBeforeCutoff,
    Indeterminate,
}

impl TemporalRelationship {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AfterCutoff => "after_cutoff",
            Self::OnOrBeforeCutoff => "on_or_before_cutoff",
            Self::Indeterminate => "indeterminate",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CalendarDate {
    pub year: i32,
    pub month: Option<u8>,
    pub day: Option<u8>,
    pub precision: DatePrecision,
}

impl CalendarDate {
    pub fn normalized(&self) -> Option<String> {
        match self.precision {
            DatePrecision::Day => Some(format!(
                "{:04}-{:02}-{:02}",
                self.year, self.month?, self.day?
            )),
            DatePrecision::Month => Some(format!("{:04}-{:02}", self.year, self.month?)),
            DatePrecision::Year => Some(format!("{:04}", self.year)),
            DatePrecision::Unknown => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DatedEvidence {
    pub date: CalendarDate,
    pub basis: DateBasis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasisJudgment {
    pub basis: DateBasis,
    pub relationship: TemporalRelationship,
    pub conflicted: bool,
    pub date: Option<CalendarDate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Classification {
    pub judgments: Vec<BasisJudgment>,
    pub event_relationship: TemporalRelationship,
    pub event_missing: bool,
    pub event_conflicted: bool,
}

pub fn compare_cutoff(
    evidence: &CalendarDate,
    evidence_basis: DateBasis,
    cutoff: &CalendarDate,
) -> TemporalRelationship {
    if evidence_basis == DateBasis::Unknown
        || evidence.precision == DatePrecision::Unknown
        || cutoff.precision == DatePrecision::Unknown
    {
        return TemporalRelationship::Indeterminate;
    }
    let coarser = if evidence.precision.rank() <= cutoff.precision.rank() {
        evidence.precision
    } else {
        cutoff.precision
    };
    let Some(evidence_key) = period_key(evidence, coarser) else {
        return TemporalRelationship::Indeterminate;
    };
    let Some(cutoff_key) = period_key(cutoff, coarser) else {
        return TemporalRelationship::Indeterminate;
    };
    match evidence_key.cmp(&cutoff_key) {
        std::cmp::Ordering::Greater => TemporalRelationship::AfterCutoff,
        std::cmp::Ordering::Less => TemporalRelationship::OnOrBeforeCutoff,
        std::cmp::Ordering::Equal => {
            if coarser == DatePrecision::Day {
                TemporalRelationship::OnOrBeforeCutoff
            } else {
                TemporalRelationship::Indeterminate
            }
        }
    }
}

fn period_key(date: &CalendarDate, precision: DatePrecision) -> Option<(i32, u8, u8)> {
    match precision {
        DatePrecision::Year => Some((date.year, 0, 0)),
        DatePrecision::Month => Some((date.year, date.month?, 0)),
        DatePrecision::Day => Some((date.year, date.month?, date.day?)),
        DatePrecision::Unknown => None,
    }
}

pub fn classify(values: &[DatedEvidence], cutoff: &CalendarDate) -> Classification {
    let mut grouped: BTreeMap<DateBasis, Vec<CalendarDate>> = BTreeMap::new();
    for value in values {
        grouped.entry(value.basis).or_default().push(value.date);
    }
    let mut judgments = Vec::new();
    for basis in [
        DateBasis::Event,
        DateBasis::Publication,
        DateBasis::PageUpdate,
        DateBasis::FirstObserved,
        DateBasis::Unknown,
    ] {
        let Some(dates) = grouped.get(&basis) else {
            continue;
        };
        let mut normalized = Vec::new();
        let mut comparable = true;
        for date in dates {
            match date.normalized() {
                Some(value) => normalized.push((value, *date)),
                None => comparable = false,
            }
        }
        normalized.sort_by(|left, right| left.0.cmp(&right.0));
        normalized.dedup_by(|left, right| left.0 == right.0);
        if !comparable || normalized.len() != 1 {
            judgments.push(BasisJudgment {
                basis,
                relationship: TemporalRelationship::Indeterminate,
                conflicted: true,
                date: None,
            });
            continue;
        }
        let date = normalized[0].1;
        judgments.push(BasisJudgment {
            basis,
            relationship: compare_cutoff(&date, basis, cutoff),
            conflicted: false,
            date: Some(date),
        });
    }
    let event = judgments.iter().find(|item| item.basis == DateBasis::Event);
    let event_missing = event.is_none();
    let event_conflicted = event.is_some_and(|item| item.conflicted);
    let event_relationship = match event {
        Some(item) if !item.conflicted => item.relationship,
        _ => TemporalRelationship::Indeterminate,
    };
    Classification {
        judgments,
        event_relationship,
        event_missing,
        event_conflicted,
    }
}

pub fn reported_after_cutoff(classification: &Classification) -> Option<(DateBasis, CalendarDate)> {
    if let Some(event) = classification
        .judgments
        .iter()
        .find(|item| item.basis == DateBasis::Event)
    {
        if event.conflicted || event.relationship != TemporalRelationship::AfterCutoff {
            return None;
        }
        return event.date.map(|date| (DateBasis::Event, date));
    }
    for basis in [
        DateBasis::Publication,
        DateBasis::PageUpdate,
        DateBasis::FirstObserved,
    ] {
        if let Some(item) = classification
            .judgments
            .iter()
            .find(|item| item.basis == basis && !item.conflicted)
        {
            if item.relationship == TemporalRelationship::AfterCutoff {
                if let Some(date) = item.date {
                    return Some((basis, date));
                }
            }
        }
    }
    None
}

pub fn parse_calendar_date(raw: &str, precision: DatePrecision) -> Option<CalendarDate> {
    let trimmed = raw.trim();
    match precision {
        DatePrecision::Day => parse_day(trimmed),
        DatePrecision::Month => parse_month(trimmed),
        DatePrecision::Year => parse_year(trimmed),
        DatePrecision::Unknown => None,
    }
}

pub fn calendar_date_from_provider_timestamp(raw: &str) -> Option<CalendarDate> {
    let parsed = crate::core::source_card::parse_result_timestamp(raw)?;
    let date = parsed.get(..10)?;
    parse_day(date)
}

fn parse_day(raw: &str) -> Option<CalendarDate> {
    if raw.len() < 10 {
        return None;
    }
    let raw = &raw[..10];
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()?;
    Some(CalendarDate {
        year: date.year(),
        month: Some(date.month() as u8),
        day: Some(date.day() as u8),
        precision: DatePrecision::Day,
    })
}

fn parse_month(raw: &str) -> Option<CalendarDate> {
    if raw.len() != 7 || raw.as_bytes().get(4) != Some(&b'-') {
        return None;
    }
    let year: i32 = raw[..4].parse().ok()?;
    let month: u8 = raw[5..7].parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    Some(CalendarDate {
        year,
        month: Some(month),
        day: None,
        precision: DatePrecision::Month,
    })
}

fn parse_year(raw: &str) -> Option<CalendarDate> {
    if raw.len() != 4 {
        return None;
    }
    let year: i32 = raw.parse().ok()?;
    if !(1..=9999).contains(&year) {
        return None;
    }
    Some(CalendarDate {
        year,
        month: None,
        day: None,
        precision: DatePrecision::Year,
    })
}

pub fn next_day(date: NaiveDate) -> Option<NaiveDate> {
    date.succ_opt()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OperatorCutoff {
    pub date: String,
    pub precision: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeIndexConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default = "default_max_bytes")]
    pub max_bytes: u64,
    #[serde(default = "default_max_documents")]
    pub max_documents: u64,
    #[serde(default = "default_max_excerpt_chars")]
    pub max_excerpt_chars: usize,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_max_bytes() -> u64 {
    64 * 1024 * 1024
}

fn default_max_documents() -> u64 {
    10_000
}

fn default_max_excerpt_chars() -> usize {
    500
}

fn default_retention_days() -> u32 {
    365
}

impl Default for KnowledgeIndexConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: None,
            max_bytes: default_max_bytes(),
            max_documents: default_max_documents(),
            max_excerpt_chars: default_max_excerpt_chars(),
            retention_days: default_retention_days(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeDeltaSection {
    #[serde(default)]
    pub cutoffs: BTreeMap<String, OperatorCutoff>,
    #[serde(default)]
    pub index: KnowledgeIndexConfig,
}

impl KnowledgeDeltaSection {
    pub fn validate(&self) -> CoreResult<()> {
        for (model, cutoff) in &self.cutoffs {
            if model.trim().is_empty() {
                return Err(CoreError::Config(
                    "[knowledge_delta.cutoffs] model id must be non-empty".to_string(),
                ));
            }
            let precision = DatePrecision::parse(&cutoff.precision).ok_or_else(|| {
                CoreError::Config(format!(
                    "[knowledge_delta.cutoffs.{model}] precision must be day, month, year, or unknown"
                ))
            })?;
            if precision != DatePrecision::Unknown
                && parse_calendar_date(&cutoff.date, precision).is_none()
            {
                return Err(CoreError::Config(format!(
                    "[knowledge_delta.cutoffs.{model}] date does not match precision {}",
                    precision.as_str()
                )));
            }
            if cutoff.source.trim().is_empty() {
                return Err(CoreError::Config(format!(
                    "[knowledge_delta.cutoffs.{model}] source must be non-empty"
                )));
            }
        }
        if self.index.max_bytes == 0 {
            return Err(CoreError::Config(
                "[knowledge_delta.index] max_bytes must be > 0".to_string(),
            ));
        }
        if self.index.max_documents == 0 {
            return Err(CoreError::Config(
                "[knowledge_delta.index] max_documents must be > 0".to_string(),
            ));
        }
        if self.index.max_excerpt_chars == 0 {
            return Err(CoreError::Config(
                "[knowledge_delta.index] max_excerpt_chars must be > 0".to_string(),
            ));
        }
        if let Some(path) = &self.index.path {
            if path.trim().is_empty() || path.contains('\0') {
                return Err(CoreError::Config(
                    "[knowledge_delta.index] path must be non-empty when set".to_string(),
                ));
            }
        }
        Ok(())
    }
}

pub const DEFAULT_MAX_ITEMS: usize = 8;
pub const DEFAULT_MAX_CHARS: usize = 6_000;
pub const MAX_SUBQUERIES: usize = 8;
pub const DEFAULT_SUBQUERIES: usize = 4;
pub const EXCERPT_CHAR_CAP: usize = 500;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCutoff {
    pub date: CalendarDate,
    pub source: String,
    pub source_url: Option<String>,
    pub verified_at: Option<String>,
    pub origin: CutoffOrigin,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CutoffOrigin {
    CallerOverride,
    Operator,
    ProviderOfficial,
    ModelsDev,
    OpenRouter,
    Unknown,
}

impl CutoffOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CallerOverride => "caller_override",
            Self::Operator => "operator",
            Self::ProviderOfficial => "provider_official",
            Self::ModelsDev => "models_dev",
            Self::OpenRouter => "openrouter",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SnapshotRow {
    pub model_id: &'static str,
    pub aliases: &'static [&'static str],
    pub date: &'static str,
    pub precision: DatePrecision,
    pub source: &'static str,
    pub source_url: &'static str,
    pub verified_at: &'static str,
    pub origin: CutoffOrigin,
}

pub fn resolve_cutoff(
    model_id: Option<&str>,
    caller: Option<&ResolvedCutoff>,
    operator: &BTreeMap<String, OperatorCutoff>,
    snapshot: &[SnapshotRow],
) -> Option<ResolvedCutoff> {
    if let Some(caller) = caller {
        return Some(caller.clone());
    }
    if let Some(model_id) = model_id.map(str::trim).filter(|value| !value.is_empty()) {
        if let Some(entry) = operator.get(model_id) {
            let precision = DatePrecision::parse(&entry.precision)?;
            let date = parse_calendar_date(&entry.date, precision)?;
            return Some(ResolvedCutoff {
                date,
                source: entry.source.clone(),
                source_url: None,
                verified_at: entry.verified_at.clone(),
                origin: CutoffOrigin::Operator,
                reason: None,
            });
        }
        for origin in [
            CutoffOrigin::ProviderOfficial,
            CutoffOrigin::ModelsDev,
            CutoffOrigin::OpenRouter,
        ] {
            if let Some(row) = snapshot.iter().find(|row| {
                row.origin == origin
                    && (row.model_id == model_id || row.aliases.contains(&model_id))
            }) {
                let date = parse_calendar_date(row.date, row.precision)?;
                return Some(ResolvedCutoff {
                    date,
                    source: row.source.to_string(),
                    source_url: Some(row.source_url.to_string()),
                    verified_at: Some(row.verified_at.to_string()),
                    origin: row.origin,
                    reason: None,
                });
            }
        }
    }
    None
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CutoffOverrideInput {
    pub date: String,
    pub precision: String,
    pub source: String,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RefreshMode {
    Live,
    PreferLive,
    Index,
}

impl RefreshMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::PreferLive => "prefer_live",
            Self::Index => "index",
        }
    }

    pub fn reads_index(self) -> bool {
        matches!(self, Self::PreferLive | Self::Index)
    }

    pub fn searches_live(self) -> bool {
        matches!(self, Self::Live | Self::PreferLive)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedBriefRequest {
    pub topic: String,
    pub model_id: Option<String>,
    pub caller_cutoff: Option<ResolvedCutoff>,
    pub max_items: usize,
    pub max_chars: usize,
    pub refresh: RefreshMode,
    pub retain: bool,
    pub related_topics: bool,
    pub language: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BriefInputError {
    EmptyTopic,
    MalformedOverride(String),
    UnknownRefresh(String),
    NonPositiveBound(&'static str),
}

impl BriefInputError {
    pub fn message(&self) -> String {
        match self {
            Self::EmptyTopic => "topic must be non-empty".to_string(),
            Self::MalformedOverride(detail) => format!("cutoff_override: {detail}"),
            Self::UnknownRefresh(value) => {
                format!("refresh must be live, prefer_live, or index, not {value:?}")
            }
            Self::NonPositiveBound(field) => format!("{field} must be > 0"),
        }
    }
}

pub fn parse_refresh(raw: Option<&str>) -> Result<RefreshMode, BriefInputError> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("live") => Ok(RefreshMode::Live),
        Some("prefer_live") => Ok(RefreshMode::PreferLive),
        Some("index") => Ok(RefreshMode::Index),
        Some(other) => Err(BriefInputError::UnknownRefresh(other.to_string())),
    }
}

pub fn parse_caller_override(
    input: &CutoffOverrideInput,
) -> Result<ResolvedCutoff, BriefInputError> {
    if input.reason.trim().is_empty() {
        return Err(BriefInputError::MalformedOverride(
            "reason must be non-empty".to_string(),
        ));
    }
    if input.source.trim().is_empty() {
        return Err(BriefInputError::MalformedOverride(
            "source must be non-empty".to_string(),
        ));
    }
    let source_key = input.source.trim().to_ascii_lowercase().replace('-', "_");
    if source_key == "provider_official" || source_key == "official" {
        return Err(BriefInputError::MalformedOverride(
            "must not be labeled provider-official".to_string(),
        ));
    }
    let precision = DatePrecision::parse(&input.precision).ok_or_else(|| {
        BriefInputError::MalformedOverride(
            "precision must be day, month, year, or unknown".to_string(),
        )
    })?;
    let date = if precision == DatePrecision::Unknown {
        CalendarDate {
            year: 0,
            month: None,
            day: None,
            precision,
        }
    } else {
        parse_calendar_date(&input.date, precision).ok_or_else(|| {
            BriefInputError::MalformedOverride("date does not match precision".to_string())
        })?
    };
    Ok(ResolvedCutoff {
        date,
        source: input.source.trim().to_string(),
        source_url: None,
        verified_at: None,
        origin: CutoffOrigin::CallerOverride,
        reason: Some(input.reason.trim().to_string()),
    })
}

pub fn bound_items(raw: Option<usize>) -> Result<usize, BriefInputError> {
    match raw {
        None => Ok(DEFAULT_MAX_ITEMS),
        Some(0) => Err(BriefInputError::NonPositiveBound("max_items")),
        Some(value) => Ok(value.min(DEFAULT_MAX_ITEMS)),
    }
}

pub fn bound_chars(raw: Option<usize>) -> Result<usize, BriefInputError> {
    match raw {
        None => Ok(DEFAULT_MAX_CHARS),
        Some(0) => Err(BriefInputError::NonPositiveBound("max_chars")),
        Some(value) => Ok(value.min(DEFAULT_MAX_CHARS)),
    }
}
