use std::sync::Arc;

use anyhow::{anyhow, Result};
use eggsearch::core::config::AppConfig;
use eggsearch::fetch::browser::ProfileManager;

pub async fn run(cfg: &AppConfig, origin: &str, profile_name: Option<&str>) -> Result<()> {
    let bp = &cfg.fetch.browser.persistent_profiles;

    if !bp.enabled {
        return Err(anyhow!(
            "persistent browser profiles are disabled; \
             enable [fetch.browser].persistent_profiles_enabled in config"
        ));
    }

    let mgr = ProfileManager::new(
        bp.profiles_dir.as_deref(),
        true,
        bp.allowed_profiles.clone(),
    )
    .map(Arc::new)
    .map_err(|e| anyhow!("error initializing profile manager: {e}"))?;

    let display_name = profile_name.unwrap_or("default");

    // Discover the browser before creating any profile state so an
    // invalid configured executable cannot leave an orphaned profile.
    let discovery = match eggsearch::fetch::browser::discover_browser(
        cfg.fetch.browser.executable.as_deref(),
    ) {
        eggsearch::fetch::browser::BrowserDiscoveryState::Available(discovery) => Some(discovery),
        eggsearch::fetch::browser::BrowserDiscoveryState::ExplicitPathInvalid { path } => {
            return Err(anyhow!("configured browser executable is invalid: {path}"));
        }
        _ => None,
    };

    let meta = mgr
        .create_profile(display_name, origin)
        .map_err(|e| anyhow!("error creating profile: {e}"))?;

    let profile_dir = mgr.profile_dir_for(&meta.id);
    let chrome_data = mgr.chrome_data_dir_for(&meta.id);

    if !chrome_data.exists() {
        tokio::fs::create_dir_all(&chrome_data)
            .await
            .map_err(|e| anyhow!("error creating chrome-data directory: {e}"))?;
    }

    println!("Browser Profile: {}", meta.display_name);
    println!("  Origin:   {}", meta.allowed_origin);
    println!("  Profile:  {}", profile_dir.display());
    println!();

    if let Some(ref discovery) = discovery {
        println!(
            "Chrome discovered. Launching headed browser at {}...",
            meta.allowed_origin
        );
        println!();

        let result =
            launch_headed_browser(cfg, discovery, &chrome_data, &meta.allowed_origin).await;

        match result {
            Ok(()) => {
                let mut updated = meta.clone();
                let _ = mgr.update_last_used(&mut updated);

                let _ = mgr.update_browser_info(
                    &mut updated,
                    &format!("{:?}", discovery.family),
                    parse_major_version(&discovery.version),
                );

                println!();
                println!("Session setup complete for '{display_name}'.");
                println!(
                    "Use with web_fetch: {{ \"url\": \"{}\", \"browser_profile\": \"{display_name}\" }}",
                    meta.allowed_origin
                );
            }
            Err(e) => {
                return Err(anyhow!(
                    "browser session ended: {e}; \
                     profile '{display_name}' was created but may need re-login"
                ));
            }
        }
    } else {
        println!("No Chrome/Chromium executable found.");
        println!("Install Chrome or set [fetch.browser].executable in config.");
        println!();
        println!("Profile '{display_name}' has been created. Once Chrome is available,");
        println!("run this command again to establish a session.");
    }

    Ok(())
}

async fn launch_headed_browser(
    cfg: &AppConfig,
    discovery: &eggsearch::fetch::browser::BrowserDiscovery,
    chrome_data_dir: &std::path::Path,
    origin: &str,
) -> anyhow::Result<()> {
    use tokio::io::AsyncBufReadExt;

    let mut cmd = tokio::process::Command::new(&discovery.path);
    cmd.arg(origin);
    cmd.arg(format!("--user-data-dir={}", chrome_data_dir.display()));
    cmd.arg("--no-first-run");
    cmd.arg("--no-default-browser-check");
    cmd.arg("--disable-extensions");
    cmd.arg("--disable-component-extensions-with-background-pages");
    cmd.arg("--disable-default-apps");
    cmd.arg("--disable-dev-shm-usage");
    cmd.arg("--disable-sync");
    cmd.arg("--disable-background-networking");
    cmd.arg("--password-store=basic");
    cmd.arg("--use-mock-keychain");

    let timeout_ms = cfg
        .fetch
        .browser
        .persistent_profiles
        .profile_process_timeout_ms;
    let timeout = std::time::Duration::from_millis(timeout_ms);

    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow!("failed to launch browser: {e}"))?;

    println!("Browser launched. Complete your login/verification in the browser window.");
    println!(
        "Press Enter here when done (or wait {}s for timeout)...",
        timeout_ms / 1000
    );

    let mut input = String::new();
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let result = tokio::time::timeout(timeout, stdin.read_line(&mut input)).await;

    match result {
        Ok(Ok(_)) => {
            let _ = child.kill().await;
            Ok(())
        }
        Ok(Err(e)) => {
            let _ = child.kill().await;
            Err(anyhow!("signal error: {e}"))
        }
        Err(_) => {
            let _ = child.kill().await;
            Err(anyhow!("timeout reached"))
        }
    }
}

fn parse_major_version(version: &str) -> Option<u32> {
    version.split('.').next().and_then(|s| s.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_major_version_works() {
        assert_eq!(parse_major_version("120.0.6099.109"), Some(120));
        assert_eq!(parse_major_version("1"), Some(1));
        assert_eq!(parse_major_version("abc"), None);
    }
}
