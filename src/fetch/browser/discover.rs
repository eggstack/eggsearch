use std::path::{Path, PathBuf};
use std::process::Command;

use super::types::{BrowserDiscovery, BrowserDiscoveryState, BrowserFamily, BrowserSource};

pub fn discover_browser(configured_path: Option<&str>) -> BrowserDiscoveryState {
    if let Some(path) = configured_path {
        if !path.is_empty() {
            let p = PathBuf::from(path);
            match try_validate(&p, BrowserSource::Configured) {
                Some(disc) => return BrowserDiscoveryState::Available(disc),
                None => {
                    return BrowserDiscoveryState::ExplicitPathInvalid {
                        path: path.to_string(),
                    };
                }
            }
        }
    }

    for candidate in linux_candidates() {
        if let Some(disc) = try_validate(&candidate, BrowserSource::AutoDiscovered) {
            return BrowserDiscoveryState::Available(disc);
        }
    }

    for candidate in macos_candidates() {
        if let Some(disc) = try_validate_expanded(&candidate, BrowserSource::AutoDiscovered) {
            return BrowserDiscoveryState::Available(disc);
        }
    }

    if let Some(path) = find_in_path("google-chrome-stable") {
        return BrowserDiscoveryState::Available(BrowserDiscovery {
            path,
            family: BrowserFamily::Chrome,
            version: String::new(),
            source: BrowserSource::AutoDiscovered,
        });
    }
    if let Some(path) = find_in_path("google-chrome") {
        return BrowserDiscoveryState::Available(BrowserDiscovery {
            path,
            family: BrowserFamily::Chrome,
            version: String::new(),
            source: BrowserSource::AutoDiscovered,
        });
    }
    if let Some(path) = find_in_path("chromium") {
        return BrowserDiscoveryState::Available(BrowserDiscovery {
            path,
            family: BrowserFamily::Chromium,
            version: String::new(),
            source: BrowserSource::AutoDiscovered,
        });
    }
    if let Some(path) = find_in_path("chromium-browser") {
        return BrowserDiscoveryState::Available(BrowserDiscovery {
            path,
            family: BrowserFamily::Chromium,
            version: String::new(),
            source: BrowserSource::AutoDiscovered,
        });
    }

    BrowserDiscoveryState::NotFound
}

fn linux_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/bin/google-chrome-stable"),
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/bin/chromium"),
        PathBuf::from("/usr/bin/chromium-browser"),
        PathBuf::from("/snap/bin/chromium"),
    ]
}

fn macos_candidates() -> Vec<String> {
    vec![
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
        "~/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
        "/Applications/Chromium.app/Contents/MacOS/Chromium".into(),
    ]
}

fn try_validate(path: &Path, source: BrowserSource) -> Option<BrowserDiscovery> {
    if !path.exists() {
        return None;
    }
    if !path.is_file() {
        return None;
    }

    let family = detect_family(path);
    let version = run_version(path).unwrap_or_default();

    Some(BrowserDiscovery {
        path: path.to_path_buf(),
        family,
        version,
        source,
    })
}

fn try_validate_expanded(path_str: &str, source: BrowserSource) -> Option<BrowserDiscovery> {
    let expanded = if let Some(rest) = path_str.strip_prefix("~/") {
        dirs::home_dir()?.join(rest)
    } else {
        PathBuf::from(path_str)
    };
    try_validate(&expanded, source)
}

fn detect_family(path: &Path) -> BrowserFamily {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if name.contains("chrome") && !name.contains("chromium") {
        BrowserFamily::Chrome
    } else if name.contains("chromium") {
        BrowserFamily::Chromium
    } else {
        BrowserFamily::Unknown(name.to_string())
    }
}

fn run_version(path: &Path) -> Option<String> {
    let output = Command::new(path).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?.trim().to_string();
    if line.is_empty() {
        return None;
    }
    Some(line)
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let output = Command::new("which").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path_str = stdout.trim();
    if path_str.is_empty() {
        return None;
    }
    let path = PathBuf::from(path_str);
    if path.exists() && path.is_file() {
        Some(path)
    } else {
        None
    }
}

pub fn browser_capability_report(
    compiled: bool,
    enabled: bool,
    discovery: Option<&BrowserDiscovery>,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert(
        "browser_feature_compiled".into(),
        serde_json::Value::Bool(compiled),
    );
    map.insert(
        "browser_enabled_in_config".into(),
        serde_json::Value::Bool(enabled),
    );
    match discovery {
        Some(disc) => {
            map.insert(
                "executable_discovered".into(),
                serde_json::Value::Bool(true),
            );
            map.insert(
                "path_source".into(),
                serde_json::Value::String(format!("{:?}", disc.source)),
            );
            map.insert(
                "browser_family".into(),
                serde_json::Value::String(format!("{:?}", disc.family)),
            );
            if !disc.version.is_empty() {
                map.insert(
                    "browser_version".into(),
                    serde_json::Value::String(disc.version.clone()),
                );
            }
            map.insert("usable".into(), serde_json::Value::Bool(true));
        }
        None => {
            map.insert(
                "executable_discovered".into(),
                serde_json::Value::Bool(false),
            );
            map.insert("usable".into(), serde_json::Value::Bool(false));
            map.insert(
                "unavailable_reason".into(),
                serde_json::Value::String("no Chrome/Chromium executable found".into()),
            );
        }
    }
    serde_json::Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_family_chrome() {
        assert_eq!(
            detect_family(Path::new("/usr/bin/google-chrome-stable")),
            BrowserFamily::Chrome
        );
    }

    #[test]
    fn detect_family_chromium() {
        assert_eq!(
            detect_family(Path::new("/usr/bin/chromium")),
            BrowserFamily::Chromium
        );
    }

    #[test]
    fn detect_family_chromium_browser() {
        assert_eq!(
            detect_family(Path::new("/usr/bin/chromium-browser")),
            BrowserFamily::Chromium
        );
    }

    #[test]
    fn capability_report_no_browser() {
        let report = browser_capability_report(true, false, None);
        assert_eq!(report["browser_feature_compiled"], true);
        assert_eq!(report["browser_enabled_in_config"], false);
        assert_eq!(report["executable_discovered"], false);
        assert_eq!(report["usable"], false);
    }
}
