//! The `eggsearch update` command.

use eggsearch::update;

pub async fn run(check: bool) -> anyhow::Result<()> {
    let outcome = update::run(check).await?;
    println!("{outcome}");
    Ok(())
}
