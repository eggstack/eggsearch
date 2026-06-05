//! eggsearch CLI entry point.

mod commands;
mod config;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "eggsearch", version, about = "Local-first MCP search server", long_about = None)]
struct Cli {
    /// Path to the config file.
    #[arg(long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Verbosity (-v, -vv).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Diagnose configuration and provider status.
    Doctor,
    /// Run a live metasearch and print compact source cards.
    Search {
        query: String,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long, default_value_t = 8)]
        max_results: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
        #[arg(long, default_value_t = 0)]
        fetch_top_n: usize,
    },
    /// Fetch and extract a known URL.
    Fetch {
        url: String,
        #[arg(long)]
        max_bytes: Option<usize>,
        #[arg(long, default_value = "readability")]
        extract_mode: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Manage the local search index.
    Index {
        #[command(subcommand)]
        cmd: IndexCmd,
    },
    /// Run the MCP server.
    Mcp {
        #[command(subcommand)]
        cmd: McpCmd,
    },
}

#[derive(Subcommand, Debug)]
enum IndexCmd {
    /// Add a file or directory to the local index.
    Add {
        path: String,
        #[arg(long)]
        tag: Vec<String>,
    },
    /// Search the local index.
    Search {
        query: String,
        #[arg(long, default_value_t = 8)]
        max_results: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Show index statistics.
    Stats,
}

#[derive(Subcommand, Debug)]
enum McpCmd {
    /// Run the MCP server over stdio.
    Stdio,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let cfg = config::load(cli.config.as_deref())?;

    match cli.command {
        Commands::Doctor => commands::doctor::run(&cfg).await,
        Commands::Search { query, provider, max_results, json, fetch_top_n } => {
            commands::search::run(&cfg, &query, provider.as_deref(), max_results, json, fetch_top_n).await
        }
        Commands::Fetch { url, max_bytes, extract_mode, json } => {
            commands::fetch::run(&cfg, &url, max_bytes, &extract_mode, json).await
        }
        Commands::Index { cmd } => match cmd {
            IndexCmd::Add { path, tag } => commands::index::add(&cfg, &path, tag).await,
            IndexCmd::Search { query, max_results, json } => {
                commands::index::search(&cfg, &query, max_results, json).await
            }
            IndexCmd::Stats => commands::index::stats(&cfg).await,
        },
        Commands::Mcp { cmd } => match cmd {
            McpCmd::Stdio => commands::mcp::run_stdio(&cfg).await,
        },
    }
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(level))
        .with_target(false)
        .try_init();
}
