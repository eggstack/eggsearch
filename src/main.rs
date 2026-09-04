//! eggsearch CLI entry point.

mod commands;
mod config;

use anyhow::Result;
use clap::{Parser, Subcommand};
use eggsearch::mcp::{McpPath, ServeOptions};
use eggsearch::startup::StartupMethod;
use std::net::SocketAddr;

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
    Doctor {
        /// Probe each provider with a live query.
        #[arg(long, default_value_t = false)]
        probe: bool,
    },
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
    /// Fetch and extract content from a URL.
    Fetch {
        /// The URL to fetch.
        url: String,
        /// Maximum characters to extract.
        #[arg(long)]
        max_chars: Option<usize>,
        /// Timeout in milliseconds.
        #[arg(long)]
        timeout_ms: Option<u64>,
        /// Extract metadata only, not body text.
        #[arg(long)]
        metadata_only: bool,
        /// Render as Markdown instead of plain text.
        #[arg(long)]
        markdown: bool,
        /// Include extracted links in output.
        #[arg(long = "include-links", alias = "links")]
        include_links: bool,
        /// Output as JSON.
        #[arg(short, long)]
        json: bool,
    },
    /// Check for and install the latest stable release.
    Update {
        /// Only compare versions; never download, compile, or replace.
        #[arg(long, default_value_t = false)]
        check: bool,
    },
    /// Check whether a persistent service is healthy and start it if absent.
    Croncheck,
    /// Restart the registered persistent service.
    Restart,
    /// Manage persistent startup supervision.
    Startup {
        #[command(subcommand)]
        cmd: StartupCmd,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    WindowsService,
    /// Open a headed browser for manual login/verification.
    #[cfg(feature = "browser")]
    BrowserLogin {
        /// The HTTP(S) origin to open (e.g. https://example.com).
        origin: String,
        /// Profile name (default: "default").
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Manage persistent browser profiles.
    #[cfg(feature = "browser")]
    BrowserProfiles {
        #[command(subcommand)]
        cmd: commands::browser_profiles::BrowserProfilesCmd,
    },
}

#[derive(Subcommand, Debug)]
enum McpCmd {
    /// Run the MCP server over stdio.
    Stdio,
    /// Run the MCP server over persistent loopback Streamable HTTP.
    Serve {
        /// Loopback socket address to bind.
        #[arg(long, default_value = "127.0.0.1:11320")]
        bind: SocketAddr,
        /// MCP endpoint path.
        #[arg(long, default_value = "/mcp")]
        path: McpPath,
        /// Internal manager-owned PID record for cron control.
        #[arg(long, hide = true)]
        pid_file: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum StartupCmd {
    /// Show the registered manager and health state.
    Status {
        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Install or update the selected startup manager.
    Install {
        #[arg(long, default_value = "auto")]
        method: StartupMethod,
    },
    /// Render exact non-mutating setup instructions.
    Instructions {
        #[arg(long, default_value = "auto")]
        method: StartupMethod,
    },
    /// Remove the selected startup manager registration.
    Uninstall {
        #[arg(long, default_value = "auto")]
        method: StartupMethod,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cli.command {
        Commands::Update { check } => commands::update::run(check, cli.config.as_deref()).await,
        Commands::Croncheck => {
            println!(
                "{}",
                eggsearch::startup::croncheck(cli.config.as_deref()).await?
            );
            Ok(())
        }
        Commands::Restart => {
            println!(
                "{}",
                eggsearch::startup::restart(cli.config.as_deref()).await?
            );
            Ok(())
        }
        Commands::Startup { cmd } => match cmd {
            StartupCmd::Status { json } => {
                let state = eggsearch::startup::startup_state(cli.config.as_deref()).await?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&state)?);
                } else {
                    println!("{}", state.detail);
                    println!(
                        "registered={} running={} healthy={} conflict={}",
                        state.registered, state.running, state.healthy, state.conflict
                    );
                }
                Ok(())
            }
            StartupCmd::Install { method } => {
                println!(
                    "{}",
                    eggsearch::startup::install(cli.config.as_deref(), method).await?
                );
                Ok(())
            }
            StartupCmd::Instructions { method } => {
                println!(
                    "{}",
                    eggsearch::startup::instructions(cli.config.as_deref(), method)?
                );
                Ok(())
            }
            StartupCmd::Uninstall { method } => {
                println!(
                    "{}",
                    eggsearch::startup::uninstall(cli.config.as_deref(), method).await?
                );
                Ok(())
            }
        },
        #[cfg(windows)]
        Commands::WindowsService => eggsearch::startup::run_windows_service(cli.config.as_deref()),
        command => {
            let cfg = config::load(cli.config.as_deref())?;
            match command {
                Commands::Doctor { probe } => {
                    commands::doctor::run(&cfg, cli.config.as_ref(), probe).await
                }
                Commands::Search {
                    query,
                    max_results,
                    json,
                    providers,
                } => commands::search::run(&cfg, &query, max_results, json, &providers).await,
                Commands::Mcp { cmd } => match cmd {
                    McpCmd::Stdio => commands::mcp::run_stdio(&cfg).await,
                    McpCmd::Serve {
                        bind,
                        path,
                        pid_file,
                    } => {
                        commands::mcp::run_http_with_pid(
                            &cfg,
                            ServeOptions { bind, path },
                            pid_file,
                        )
                        .await
                    }
                },
                Commands::Providers { json } => commands::providers::run(&cfg, json),
                Commands::Fetch {
                    url,
                    max_chars,
                    timeout_ms,
                    metadata_only,
                    markdown,
                    include_links,
                    json,
                } => {
                    commands::fetch::run(
                        &cfg,
                        &url,
                        max_chars,
                        timeout_ms,
                        metadata_only,
                        markdown,
                        include_links,
                        json,
                    )
                    .await
                }
                #[cfg(feature = "browser")]
                Commands::BrowserLogin { origin, profile } => {
                    commands::browser_login::run(&cfg, &origin, profile.as_deref()).await
                }
                #[cfg(feature = "browser")]
                Commands::BrowserProfiles { cmd } => {
                    commands::browser_profiles::run(&cfg, &cmd).await
                }
                Commands::Update { .. } => {
                    unreachable!("update is handled before configuration loading")
                }
                Commands::Croncheck | Commands::Restart | Commands::Startup { .. } => {
                    unreachable!("startup commands are handled before configuration loading")
                }
                #[cfg(windows)]
                Commands::WindowsService => {
                    unreachable!("Windows service is handled before configuration loading")
                }
            }
        }
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
        .with_writer(std::io::stderr)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_serve_defaults_are_stable() {
        let cli = Cli::try_parse_from(["eggsearch", "mcp", "serve"]).unwrap();
        let Commands::Mcp {
            cmd: McpCmd::Serve { bind, path, .. },
        } = cli.command
        else {
            panic!("expected mcp serve");
        };
        assert_eq!(bind, eggsearch::mcp::http::DEFAULT_BIND);
        assert_eq!(path.as_str(), eggsearch::mcp::http::DEFAULT_PATH);
    }

    #[test]
    fn mcp_serve_accepts_typed_socket_and_path_values() {
        let cli = Cli::try_parse_from([
            "eggsearch",
            "mcp",
            "serve",
            "--bind",
            "[::1]:12345",
            "--path",
            "/local/mcp",
        ])
        .unwrap();
        let Commands::Mcp {
            cmd: McpCmd::Serve { bind, path, .. },
        } = cli.command
        else {
            panic!("expected mcp serve");
        };
        assert_eq!(bind, "[::1]:12345".parse().unwrap());
        assert_eq!(path.as_str(), "/local/mcp");
    }
}
