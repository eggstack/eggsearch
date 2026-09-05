#![allow(missing_docs)]

pub mod claude;
pub mod codegg;
pub mod codex;
mod common;
pub mod cursor;
pub mod opencode;
pub mod vscode;
pub mod zed;

pub use common::{
    render, run, summaries, Client, IntegrationReport, IntegrationSummary, Transport,
};
