use anyhow::Result;
use eggsearch::integrations::{self, Client, Transport};
use std::path::PathBuf;

pub async fn run(command: IntegrateCommand, json: bool, executable: Option<PathBuf>) -> Result<()> {
    match command {
        IntegrateCommand::List => {
            let summaries = integrations::summaries();
            if json {
                println!("{}", serde_json::to_string_pretty(&summaries)?);
            } else {
                println!("client\tavailable\tstdio\thttp\tapply-mode");
                for summary in summaries {
                    println!(
                        "{}\t{}\t{}\t{}\t{}",
                        summary.client,
                        summary.available,
                        summary.stdio,
                        summary.http,
                        summary.apply_mode
                    );
                }
            }
            Ok(())
        }
        IntegrateCommand::Client {
            client,
            transport,
            apply,
        } => integrations::run(client, transport, apply, json, executable).await,
    }
}

#[derive(Debug)]
pub enum IntegrateCommand {
    List,
    Client {
        client: Client,
        transport: Transport,
        apply: bool,
    },
}
