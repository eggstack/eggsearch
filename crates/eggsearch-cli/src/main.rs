//! eggsearch CLI entry point.

mod commands;
mod config;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "eggsearch",
    version,
    about = "Lightweight MCP metasearch server",
    long_about = None
)]
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
        #[arg(long, default_value_t = 10)]
        max_results: usize,
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Specific provider IDs to query (empty = server defaults).
        #[arg(long, value_delimiter = ',')]
        providers: Vec<String>,
    },
    /// Run the MCP server.
    Mcp {
        #[command(subcommand)]
        cmd: McpCmd,
    },
    /// Show provider configuration and status.
    Providers {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
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
        Commands::Doctor => commands::doctor::run(&cfg, cli.config.as_ref()).await,
        Commands::Search {
            query,
            max_results,
            json,
            providers,
        } => commands::search::run(&cfg, &query, max_results, json, &providers).await,
        Commands::Mcp { cmd } => match cmd {
            McpCmd::Stdio => commands::mcp::run_stdio(&cfg).await,
        },
        Commands::Providers { json } => commands::providers::run(&cfg, json).await,
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
