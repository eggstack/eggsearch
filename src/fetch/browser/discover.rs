use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::types::{BrowserDiscovery, BrowserDiscoveryState, BrowserFamily, BrowserSource};

pub fn discover_browser(configured_path: Option<&str>) -> BrowserDiscoveryState {
    if let Some(path) = configured_path {
        if !path.is_empty() {
            let p = PathBuf::from(path);
            return match try_validate(&p, BrowserSource::Configured) {
                CandidateValidation::Available(discovery) => {
                    BrowserDiscoveryState::Available(discovery)
                }
                CandidateValidation::VersionUnrecognized(version) => {
                    BrowserDiscoveryState::VersionUnrecognized { version }
                }
                CandidateValidation::Invalid => BrowserDiscoveryState::ExplicitPathInvalid {
                    path: path.to_string(),
                },
            };
        }
    }

    let mut unrecognized_version = None;
    for candidate in linux_candidates() {
        match try_validate(&candidate, BrowserSource::AutoDiscovered) {
            CandidateValidation::Available(discovery) => {
                return BrowserDiscoveryState::Available(discovery)
            }
            CandidateValidation::VersionUnrecognized(version) => {
                unrecognized_version.get_or_insert(version);
            }
            CandidateValidation::Invalid => {}
        }
    }

    for candidate in macos_candidates() {
        match try_validate_expanded(&candidate, BrowserSource::AutoDiscovered) {
            CandidateValidation::Available(discovery) => {
                return BrowserDiscoveryState::Available(discovery)
            }
            CandidateValidation::VersionUnrecognized(version) => {
                unrecognized_version.get_or_insert(version);
            }
            CandidateValidation::Invalid => {}
        }
    }

    for name in [
        "google-chrome-stable",
        "google-chrome",
        "chromium",
        "chromium-browser",
    ] {
        if let Some(path) = find_in_path(name) {
            match try_validate(&path, BrowserSource::AutoDiscovered) {
                CandidateValidation::Available(discovery) => {
                    return BrowserDiscoveryState::Available(discovery)
                }
                CandidateValidation::VersionUnrecognized(version) => {
                    unrecognized_version.get_or_insert(version);
                }
                CandidateValidation::Invalid => {}
            }
        }
    }

    unrecognized_version.map_or(BrowserDiscoveryState::NotFound, |version| {
        BrowserDiscoveryState::VersionUnrecognized { version }
    })
}

pub fn browser_discovery_reason(state: &BrowserDiscoveryState) -> Option<String> {
    match state {
        BrowserDiscoveryState::Available(_) => None,
        BrowserDiscoveryState::ExplicitPathInvalid { path } => Some(format!(
            "configured browser executable is invalid: {path}; set [fetch.browser].executable to a valid executable"
        )),
        BrowserDiscoveryState::NotFound => Some(
            "no Chrome/Chromium executable was found; install Chrome/Chromium or set [fetch.browser].executable".to_string(),
        ),
        BrowserDiscoveryState::VersionUnrecognized { version } => Some(format!(
            "browser version output is not recognized as Chrome/Chromium: {version}; set [fetch.browser].executable to a Chrome/Chromium executable that reports its version"
        )),
    }
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

enum CandidateValidation {
    Available(BrowserDiscovery),
    VersionUnrecognized(String),
    Invalid,
}

fn try_validate(path: &Path, source: BrowserSource) -> CandidateValidation {
    if !path.exists() || !path.is_file() || !is_executable(path) {
        return CandidateValidation::Invalid;
    }
    let Some(version) = run_version(path) else {
        return CandidateValidation::Invalid;
    };
    if !has_recognized_version_output(&version) {
        return CandidateValidation::VersionUnrecognized(version);
    }

    CandidateValidation::Available(BrowserDiscovery {
        path: path.to_path_buf(),
        family: detect_family(path),
        version,
        source,
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn try_validate_expanded(path_str: &str, source: BrowserSource) -> CandidateValidation {
    let expanded = if let Some(rest) = path_str.strip_prefix("~/") {
        let Some(home) = dirs::home_dir() else {
            return CandidateValidation::Invalid;
        };
        home.join(rest)
    } else {
        PathBuf::from(path_str)
    };
    try_validate(&expanded, source)
}

fn has_recognized_version_output(version: &str) -> bool {
    let version = version.trim();
    let recognized_product = ["Google Chrome", "Chrome", "Chromium"]
        .iter()
        .any(|product| version.starts_with(product));
    recognized_product && super::profiles::parse_browser_major_version(version).is_some()
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

const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

fn run_version(path: &Path) -> Option<String> {
    let mut command = Command::new(path);
    command.arg("--version");
    let output =
        crate::process::run_bounded_command(&mut command, VERSION_PROBE_TIMEOUT, 8192, 1024);
    if !output.status.is_some_and(|status| status.success())
        || output.timed_out
        || output.stdout_truncated
        || output.stderr_truncated
    {
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
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(name);
            if candidate.exists() && candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let mut command = Command::new("which");
    command.arg(name);
    let output = crate::process::run_bounded_command(
        &mut command,
        std::time::Duration::from_secs(5),
        4096,
        4096,
    );
    if !output.status.is_some_and(|status| status.success())
        || output.timed_out
        || output.stdout_truncated
        || output.stderr_truncated
    {
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
    fn version_output_requires_a_recognized_product_and_version() {
        assert!(has_recognized_version_output("Google Chrome 142.0.7431.0"));
        assert!(has_recognized_version_output("Chromium 120.0.0.0"));
        assert!(!has_recognized_version_output("Chrome unknown"));
        assert!(!has_recognized_version_output("Unknown browser 120.0.0.0"));
    }

    #[test]
    fn capability_report_no_browser() {
        let report = browser_capability_report(true, false, None);
        assert_eq!(report["browser_feature_compiled"], true);
        assert_eq!(report["browser_enabled_in_config"], false);
        assert_eq!(report["executable_discovered"], false);
        assert_eq!(report["usable"], false);
    }

    #[test]
    fn discovery_reasons_cover_every_state() {
        let available = BrowserDiscoveryState::Available(BrowserDiscovery {
            path: PathBuf::from("/synthetic/chrome"),
            family: BrowserFamily::Chrome,
            version: "Chrome 120.0.0.0".to_string(),
            source: BrowserSource::Configured,
        });
        assert_eq!(browser_discovery_reason(&available), None);
        assert_eq!(
            browser_discovery_reason(&BrowserDiscoveryState::ExplicitPathInvalid {
                path: "/synthetic/missing/chrome".to_string(),
            })
            .as_deref(),
            Some("configured browser executable is invalid: /synthetic/missing/chrome; set [fetch.browser].executable to a valid executable")
        );
        assert_eq!(
            browser_discovery_reason(&BrowserDiscoveryState::NotFound).as_deref(),
            Some("no Chrome/Chromium executable was found; install Chrome/Chromium or set [fetch.browser].executable")
        );
        assert_eq!(
            browser_discovery_reason(&BrowserDiscoveryState::VersionUnrecognized {
                version: "Chromium unknown".to_string(),
            })
            .as_deref(),
            Some("browser version output is not recognized as Chrome/Chromium: Chromium unknown; set [fetch.browser].executable to a Chrome/Chromium executable that reports its version")
        );
    }
}
