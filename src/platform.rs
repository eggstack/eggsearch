//! Shared host and release-asset contract.

/// The published crate name.
pub const CRATE_NAME: &str = "eggsearch";

/// The public GitHub repository.
pub const REPOSITORY: &str = "eggstack/eggsearch";

/// The crates.io API base URL.
pub const REGISTRY_BASE_URL: &str = "https://crates.io";

/// The GitHub base URL.
pub const GITHUB_BASE_URL: &str = "https://github.com/eggstack/eggsearch";

/// A release target and its public executable asset name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseTarget {
    /// Rust target triple.
    pub rust_target: &'static str,
    /// Public GitHub Release asset name.
    pub asset: &'static str,
    /// Normalized host operating-system family.
    pub os: &'static str,
    /// Normalized host architecture.
    pub arch: &'static str,
}

/// The phase-6 release target contract.
pub const RELEASE_TARGETS: &[ReleaseTarget] = &[
    ReleaseTarget {
        rust_target: "x86_64-unknown-linux-gnu",
        asset: "eggsearch-x86_64-unknown-linux-gnu",
        os: "linux",
        arch: "x86_64",
    },
    ReleaseTarget {
        rust_target: "aarch64-unknown-linux-gnu",
        asset: "eggsearch-aarch64-unknown-linux-gnu",
        os: "linux",
        arch: "aarch64",
    },
    ReleaseTarget {
        rust_target: "armv7-unknown-linux-gnueabihf",
        asset: "eggsearch-armv7-unknown-linux-gnueabihf",
        os: "linux",
        arch: "armv7",
    },
    ReleaseTarget {
        rust_target: "x86_64-apple-darwin",
        asset: "eggsearch-x86_64-apple-darwin",
        os: "macos",
        arch: "x86_64",
    },
    ReleaseTarget {
        rust_target: "aarch64-apple-darwin",
        asset: "eggsearch-aarch64-apple-darwin",
        os: "macos",
        arch: "aarch64",
    },
    ReleaseTarget {
        rust_target: "x86_64-pc-windows-msvc",
        asset: "eggsearch-x86_64-pc-windows-msvc.exe",
        os: "windows",
        arch: "x86_64",
    },
    ReleaseTarget {
        rust_target: "aarch64-pc-windows-msvc",
        asset: "eggsearch-aarch64-pc-windows-msvc.exe",
        os: "windows",
        arch: "aarch64",
    },
];

/// Resolve a normalized or common host alias to a release target.
pub fn target_for_host(os: &str, arch: &str) -> Option<ReleaseTarget> {
    let normalized_os = os.to_ascii_lowercase();
    let os = match normalized_os.as_str() {
        "darwin" => "macos",
        value => value,
    };
    let normalized_arch = arch.to_ascii_lowercase();
    let arch = match normalized_arch.as_str() {
        "amd64" | "x64" => "x86_64",
        "arm64" => "aarch64",
        "armv7l" | "armv7" => "armv7",
        value => value,
    };

    RELEASE_TARGETS
        .iter()
        .copied()
        .find(|target| target.os == os && target.arch == arch)
}

/// Resolve the current process host to a release target.
pub fn current_target() -> Option<ReleaseTarget> {
    target_for_host(std::env::consts::OS, std::env::consts::ARCH)
}

/// Resolve a release target triple to its public asset name.
pub fn asset_for_target(rust_target: &str) -> Option<&'static str> {
    RELEASE_TARGETS
        .iter()
        .find(|target| target.rust_target == rust_target)
        .map(|target| target.asset)
}

/// Construct the exact asset URL for a published version.
pub fn asset_url(github_base_url: &str, version: &str, asset: &str) -> String {
    format!(
        "{}/releases/download/v{version}/{asset}",
        github_base_url.trim_end_matches('/')
    )
}

/// Construct the exact checksum URL for a published version.
pub fn checksum_url(github_base_url: &str, version: &str, asset: &str) -> String {
    format!("{}.sha256", asset_url(github_base_url, version, asset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_contract_matches_phase_six_fixture() {
        let actual: Vec<_> = include_str!("../packaging/release-targets.txt")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.split('|').collect::<Vec<_>>())
            .collect();
        let expected: Vec<_> = RELEASE_TARGETS
            .iter()
            .map(|target| {
                vec![
                    target.rust_target,
                    target.asset,
                    if target.os == "macos" {
                        "darwin"
                    } else {
                        target.os
                    },
                    target.arch,
                ]
            })
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn host_aliases_cover_supported_targets() {
        assert_eq!(
            target_for_host("Linux", "amd64").unwrap().rust_target,
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            target_for_host("linux", "aarch64").unwrap().rust_target,
            "aarch64-unknown-linux-gnu"
        );
        assert_eq!(
            target_for_host("linux", "armv7l").unwrap().rust_target,
            "armv7-unknown-linux-gnueabihf"
        );
        assert_eq!(
            target_for_host("Darwin", "arm64").unwrap().rust_target,
            "aarch64-apple-darwin"
        );
        assert_eq!(
            target_for_host("windows", "x64").unwrap().rust_target,
            "x86_64-pc-windows-msvc"
        );
        assert_eq!(
            target_for_host("windows", "ARM64").unwrap().rust_target,
            "aarch64-pc-windows-msvc"
        );
        assert!(target_for_host("linux", "arm").is_none());
        assert!(target_for_host("freebsd", "x86_64").is_none());
    }

    #[test]
    fn exact_urls_use_tag_and_exe_contract() {
        assert_eq!(
            asset_url(GITHUB_BASE_URL, "0.3.9", "eggsearch-x86_64-unknown-linux-gnu"),
            "https://github.com/eggstack/eggsearch/releases/download/v0.3.9/eggsearch-x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            checksum_url(GITHUB_BASE_URL, "0.3.9", "eggsearch-x86_64-pc-windows-msvc.exe"),
            "https://github.com/eggstack/eggsearch/releases/download/v0.3.9/eggsearch-x86_64-pc-windows-msvc.exe.sha256"
        );
    }
}
