//! The `eggsearch update` command.

use eggsearch::update;

pub async fn run(check: bool, config: Option<&std::path::Path>) -> anyhow::Result<()> {
    let outcome = update::run_with_config(check, config).await?;
    println!("{outcome}");
    Ok(())
}
