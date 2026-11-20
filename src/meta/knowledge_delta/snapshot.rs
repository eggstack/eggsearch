use crate::core::knowledge_delta::{CutoffOrigin, DatePrecision, SnapshotRow};

pub static BUNDLED_CUTOFF_SNAPSHOT: &[SnapshotRow] = &[SnapshotRow {
    model_id: "gpt-6-astra",
    aliases: &["openai/gpt-6-astra"],
    date: "2026-04-30",
    precision: DatePrecision::Day,
    source: "OpenAI API model documentation",
    source_url: "https://platform.openai.com/docs/models/gpt-6-astra.md",
    verified_at: "2026-11-20",
    origin: CutoffOrigin::ProviderOfficial,
}];
