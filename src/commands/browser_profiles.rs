use std::sync::Arc;

use eggsearch::core::config::AppConfig;
use eggsearch::fetch::browser::{discover_browser, parse_browser_major_version, ProfileManager};

pub async fn run(cfg: &AppConfig, subcmd: &BrowserProfilesCmd) {
    let bp = &cfg.fetch.browser.persistent_profiles;
    let mgr = match ProfileManager::new(
        bp.profiles_dir.as_deref(),
        bp.enabled,
        bp.allowed_profiles.clone(),
    ) {
        Ok(m) => Arc::new(m),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    match subcmd {
        BrowserProfilesCmd::List => run_list(&mgr),
        BrowserProfilesCmd::Inspect { name } => run_inspect(&mgr, name),
        BrowserProfilesCmd::Remove { name } => run_remove(&mgr, name),
    }
}

fn run_list(mgr: &ProfileManager) {
    if !mgr.profiles_enabled() {
        println!("persistent browser profiles are disabled");
        println!("enable [fetch.browser].persistent_profiles_enabled in config");
        return;
    }

    let profiles = match mgr.list_profiles() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error listing profiles: {e}");
            std::process::exit(1);
        }
    };

    if profiles.is_empty() {
        println!("no browser profiles found");
        println!("create one with: eggsearch browser-login <origin> --profile <name>");
        return;
    }

    println!(
        "{:<20} {:<30} {:<10} {:<20}",
        "NAME", "ORIGIN", "STATE", "LAST USED"
    );
    println!("{}", "-".repeat(80));
    for p in &profiles {
        let state = if p.browser_family.is_empty() {
            "incomplete"
        } else {
            "ready"
        };
        let last_used = p
            .last_used_at
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "never".to_string());
        println!(
            "{:<20} {:<30} {:<10} {:<20}",
            p.display_name, p.allowed_origin, state, last_used
        );
    }
    println!("\n{} profile(s) total", profiles.len());
}

fn run_inspect(mgr: &ProfileManager, name: &str) {
    if !mgr.profiles_enabled() {
        eprintln!("error: persistent browser profiles are disabled");
        std::process::exit(1);
    }

    let meta = match mgr.resolve_by_name(name) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    let profile_dir = mgr.profile_dir_for(&meta.id);
    let chrome_data = mgr.chrome_data_dir_for(&meta.id);

    let dir_size = compute_dir_size(&profile_dir);
    let chrome_size = compute_dir_size(&chrome_data);

    let lock_path = profile_dir.join(".lock");
    let lock_state = if lock_path.exists() {
        "locked"
    } else {
        "unlocked"
    };

    let discovery = discover_browser(None);
    let current_major = discovery
        .as_ref()
        .and_then(|d| parse_browser_major_version(&d.version));

    let compat_warning = match (meta.browser_major_version, current_major) {
        (Some(profile_ver), Some(browser_ver)) if profile_ver > browser_ver => Some(format!(
            "WARNING: profile was created with browser v{profile_ver} \
                 but current browser is v{browser_ver}; profile may be incompatible"
        )),
        (Some(profile_ver), Some(browser_ver)) => Some(format!(
            "compatible (profile v{profile_ver}, browser v{browser_ver})"
        )),
        _ => None,
    };

    println!("Profile: {}", meta.display_name);
    println!("  ID:               {}", meta.id);
    println!("  Allowed Origin:   {}", meta.allowed_origin);
    println!(
        "  Created:          {}",
        meta.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!(
        "  Last Used:        {}",
        meta.last_used_at
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "never".to_string())
    );
    println!(
        "  Browser Family:   {}",
        if meta.browser_family.is_empty() {
            "unknown"
        } else {
            &meta.browser_family
        }
    );
    println!(
        "  Browser Version:  {}",
        meta.browser_major_version
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!("  Schema Version:   {}", meta.schema_version);
    println!("  Lock State:       {lock_state}");
    println!("  Profile Dir:      {}", profile_dir.display());
    println!("  Chrome Data Dir:  {}", chrome_data.display());
    println!("  Profile Size:     {}", format_bytes(dir_size));
    println!("  Chrome Data Size: {}", format_bytes(chrome_size));
    println!("  Cache Scope:      {}", meta.id);
    if let Some(warning) = compat_warning {
        println!("  Compatibility:    {warning}");
    }
}

fn run_remove(mgr: &ProfileManager, name: &str) {
    if !mgr.profiles_enabled() {
        eprintln!("error: persistent browser profiles are disabled");
        std::process::exit(1);
    }

    match mgr.remove_profile(name) {
        Ok(removed_id) => {
            println!("profile '{name}' removed (id: {removed_id})");
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

fn compute_dir_size(path: &std::path::Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += compute_dir_size(&p);
            } else if let Ok(meta) = std::fs::metadata(&p) {
                total += meta.len();
            }
        }
    }
    total
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum BrowserProfilesCmd {
    /// List all browser profiles.
    List,
    /// Show detailed information about a profile.
    Inspect {
        /// Profile name to inspect.
        name: String,
    },
    /// Remove a browser profile and its data.
    Remove {
        /// Profile name to remove.
        name: String,
    },
}
