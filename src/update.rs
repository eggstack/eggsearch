//! Binary-first self-update orchestration.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use futures::StreamExt;
use semver::Version;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

use crate::platform::{self, ReleaseTarget};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_REGISTRY_BODY_BYTES: usize = 64 * 1024;
const MAX_CHECKSUM_BODY_BYTES: usize = 4 * 1024;
const MAX_ASSET_BYTES: usize = 128 * 1024 * 1024;
const MAX_CANDIDATE_OUTPUT_BYTES: usize = 16 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const CANDIDATE_TIMEOUT: Duration = Duration::from_secs(10);
const CARGO_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// The result of a version comparison or completed replacement.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateOutcome {
    /// The installed version is already current.
    AlreadyCurrent { version: Version },
    /// The installed version is newer than the registry version.
    LocalVersionAhead { current: Version, registry: Version },
    /// A newer stable version is available and `--check` did not mutate anything.
    UpdateAvailable { current: Version, latest: Version },
    /// A verified GitHub Release binary replaced the current executable.
    UpdatedBinary { from: Version, to: Version },
    /// A verified exact-version Cargo build replaced the current executable.
    UpdatedFromCargo { from: Version, to: Version },
}

impl fmt::Display for UpdateOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyCurrent { version } => write!(formatter, "eggsearch {version} is already current"),
            Self::LocalVersionAhead { current, registry } => write!(
                formatter,
                "local eggsearch {current} is newer than registry {registry}; no downgrade performed"
            ),
            Self::UpdateAvailable { current, latest } => write!(
                formatter,
                "update available: eggsearch {current} -> {latest} (run `eggsearch update`)"
            ),
            Self::UpdatedBinary { from, to } => write!(formatter, "updated eggsearch {from} -> {to} from the verified release binary"),
            Self::UpdatedFromCargo { from, to } => write!(formatter, "updated eggsearch {from} -> {to} from an exact Cargo build"),
        }
    }
}

/// Errors produced by update discovery, verification, and replacement.
#[allow(missing_docs)]
#[derive(Debug, Error)]
pub enum UpdateError {
    /// The crates.io response could not be fetched or decoded.
    #[error("version lookup failed: {0}")]
    VersionLookup(String),
    /// A bounded response exceeded its permitted size.
    #[error("{resource} response body exceeds the {limit}-byte limit")]
    ResponseTooLarge {
        resource: &'static str,
        limit: usize,
    },
    /// A release request returned a non-success status.
    #[error("{resource} request returned HTTP {status}: {url}")]
    HttpStatus {
        resource: &'static str,
        status: u16,
        url: String,
    },
    /// The exact release asset was not published for this target.
    #[error("exact release asset is unavailable (HTTP 404): {url}")]
    AssetUnavailable { url: String },
    /// The release asset could not be downloaded.
    #[error("release asset download failed: {0}")]
    Download(String),
    /// The checksum file could not be fetched or parsed.
    #[error("checksum verification failed: {0}")]
    Checksum(String),
    /// The candidate did not identify as the exact requested eggsearch version.
    #[error("candidate identity/version check failed: {0}")]
    CandidateIdentity(String),
    /// The candidate could not be executed.
    #[error("candidate execution failed: {0}")]
    CandidateExecution(String),
    /// The current executable cannot be replaced without additional privilege.
    #[error("permission denied replacing {path}\nrerun:\n  {rerun}")]
    PermissionDenied { path: PathBuf, rerun: String },
    /// Cargo was required but is not available.
    #[error("Cargo is required to update eggsearch {version} on this host; install Rust from https://rustup.rs/ and retry")]
    CargoMissing { version: Version },
    /// Cargo failed to build the exact requested version.
    #[error("Cargo failed while building eggsearch {version}: {detail}")]
    CargoFailed { version: Version, detail: String },
    /// The replacement operation failed.
    #[error("executable replacement failed for {path}: {detail}")]
    Replacement { path: PathBuf, detail: String },
    /// An internal client could not be initialized.
    #[error("update client initialization failed: {0}")]
    Client(String),
    /// A filesystem operation needed by the updater failed.
    #[error("update filesystem operation failed: {0}")]
    Filesystem(#[from] io::Error),
}

struct UpdateEndpoints {
    registry_base_url: String,
    github_base_url: String,
}

struct UpdateClient {
    http: reqwest::Client,
    endpoints: UpdateEndpoints,
    current: Version,
}

impl UpdateClient {
    fn new() -> Result<Self, UpdateError> {
        let http = reqwest::Client::builder()
            .user_agent(format!("eggsearch/{CURRENT_VERSION} self-update"))
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| UpdateError::Client(error.to_string()))?;
        let current = Version::parse(CURRENT_VERSION).map_err(|error| {
            UpdateError::VersionLookup(format!("installed version is malformed: {error}"))
        })?;
        Ok(Self {
            http,
            endpoints: UpdateEndpoints {
                registry_base_url: platform::REGISTRY_BASE_URL.to_string(),
                github_base_url: platform::GITHUB_BASE_URL.to_string(),
            },
            current,
        })
    }

    async fn execute(
        &self,
        check: bool,
        destination: Option<PathBuf>,
    ) -> Result<UpdateOutcome, UpdateError> {
        let latest = self.latest_stable_version().await?;
        match self.current.cmp(&latest) {
            std::cmp::Ordering::Equal => Ok(UpdateOutcome::AlreadyCurrent {
                version: self.current.clone(),
            }),
            std::cmp::Ordering::Greater => Ok(UpdateOutcome::LocalVersionAhead {
                current: self.current.clone(),
                registry: latest,
            }),
            std::cmp::Ordering::Less if check => Ok(UpdateOutcome::UpdateAvailable {
                current: self.current.clone(),
                latest,
            }),
            std::cmp::Ordering::Less => {
                let destination = match destination {
                    Some(path) => path,
                    None => std::env::current_exe().map_err(UpdateError::Filesystem)?,
                };
                ensure_replacement_permission(&destination)?;
                if let Some(target) = platform::current_target() {
                    match self.download_release(&latest, target, &destination).await {
                        Ok(candidate) => {
                            verify_candidate_file(
                                &candidate,
                                target.asset,
                                &latest,
                                &self.http,
                                &self.endpoints.github_base_url,
                            )
                            .await?;
                            replace_candidate(&candidate, &destination)?;
                            Ok(UpdateOutcome::UpdatedBinary {
                                from: self.current.clone(),
                                to: latest,
                            })
                        }
                        Err(UpdateError::AssetUnavailable { .. }) => {
                            self.update_from_cargo(&latest, &destination).await
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    self.update_from_cargo(&latest, &destination).await
                }
            }
        }
    }

    async fn latest_stable_version(&self) -> Result<Version, UpdateError> {
        let url = format!(
            "{}/api/v1/crates/{}",
            self.endpoints.registry_base_url.trim_end_matches('/'),
            platform::CRATE_NAME
        );
        let body = bounded_get(&self.http, &url, MAX_REGISTRY_BODY_BYTES, "registry")
            .await
            .map_err(|error| match error {
                UpdateError::HttpStatus { status, url, .. } => {
                    UpdateError::VersionLookup(format!("HTTP {status}: {url}"))
                }
                UpdateError::Download(detail) => UpdateError::VersionLookup(detail),
                error => error,
            })?;
        let payload: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|error| UpdateError::VersionLookup(format!("invalid JSON: {error}")))?;
        let raw = payload
            .get("crate")
            .and_then(|crate_data| crate_data.get("max_stable_version"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                UpdateError::VersionLookup("missing crate.max_stable_version".to_string())
            })?;
        parse_stable_version(raw).map_err(|error| {
            UpdateError::VersionLookup(format!("invalid max_stable_version: {error}"))
        })
    }

    async fn download_release(
        &self,
        version: &Version,
        target: ReleaseTarget,
        destination: &Path,
    ) -> Result<NamedTempFile, UpdateError> {
        let url = platform::asset_url(
            &self.endpoints.github_base_url,
            &version.to_string(),
            target.asset,
        );
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|error| UpdateError::Download(error.to_string()))?;
        if response.status().as_u16() == 404 {
            return Err(UpdateError::AssetUnavailable { url });
        }
        if !response.status().is_success() {
            return Err(UpdateError::HttpStatus {
                resource: "release asset",
                status: response.status().as_u16(),
                url,
            });
        }
        let parent = destination
            .parent()
            .ok_or_else(|| UpdateError::Replacement {
                path: destination.to_path_buf(),
                detail: "destination has no parent directory".to_string(),
            })?;
        let mut candidate = NamedTempFile::new_in(parent).map_err(UpdateError::Filesystem)?;
        if let Some(content_length) = response.content_length() {
            if content_length > MAX_ASSET_BYTES as u64 {
                return Err(UpdateError::ResponseTooLarge {
                    resource: "release asset",
                    limit: MAX_ASSET_BYTES,
                });
            }
        }
        let mut body = response.bytes_stream();
        let mut total = 0usize;
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|error| UpdateError::Download(error.to_string()))?;
            total = total.saturating_add(chunk.len());
            if total > MAX_ASSET_BYTES {
                return Err(UpdateError::ResponseTooLarge {
                    resource: "release asset",
                    limit: MAX_ASSET_BYTES,
                });
            }
            candidate
                .as_file_mut()
                .write_all(&chunk)
                .map_err(UpdateError::Filesystem)?;
        }
        candidate
            .as_file_mut()
            .flush()
            .map_err(UpdateError::Filesystem)?;
        candidate
            .as_file()
            .sync_all()
            .map_err(UpdateError::Filesystem)?;
        Ok(candidate)
    }

    async fn update_from_cargo(
        &self,
        version: &Version,
        destination: &Path,
    ) -> Result<UpdateOutcome, UpdateError> {
        let cargo = find_on_path("cargo").ok_or_else(|| UpdateError::CargoMissing {
            version: version.clone(),
        })?;
        let root = tempfile::tempdir().map_err(UpdateError::Filesystem)?;
        let command = cargo_command(&cargo, version, root.path());
        let mut command = tokio::process::Command::from(command);
        let mut child = command
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| UpdateError::CargoFailed {
                version: version.clone(),
                detail: error.to_string(),
            })?;
        let status = match tokio::time::timeout(CARGO_TIMEOUT, child.wait()).await {
            Ok(result) => result.map_err(|error| UpdateError::CargoFailed {
                version: version.clone(),
                detail: error.to_string(),
            })?,
            Err(_) => {
                let _ = child.kill().await;
                return Err(UpdateError::CargoFailed {
                    version: version.clone(),
                    detail: "timed out after 30 minutes".to_string(),
                });
            }
        };
        if !status.success() {
            return Err(UpdateError::CargoFailed {
                version: version.clone(),
                detail: format!("process exited with {status}"),
            });
        }
        let candidate = root.path().join("bin").join(executable_name());
        if !candidate.is_file() {
            return Err(UpdateError::CargoFailed {
                version: version.clone(),
                detail: format!("Cargo completed without producing {}", candidate.display()),
            });
        }
        verify_candidate(&candidate, version).await?;
        replace_candidate_path(&candidate, destination)?;
        Ok(UpdateOutcome::UpdatedFromCargo {
            from: self.current.clone(),
            to: version.clone(),
        })
    }
}

/// Run `eggsearch update` or its non-mutating check mode.
pub async fn run(check: bool) -> Result<UpdateOutcome, UpdateError> {
    UpdateClient::new()?.execute(check, None).await
}

fn parse_stable_version(raw: &str) -> Result<Version, String> {
    let version = Version::parse(raw).map_err(|error| error.to_string())?;
    if !version.pre.is_empty() {
        return Err("pre-release versions are not eligible for automatic update".to_string());
    }
    Ok(version)
}

async fn bounded_get(
    client: &reqwest::Client,
    url: &str,
    limit: usize,
    resource: &'static str,
) -> Result<Vec<u8>, UpdateError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| UpdateError::Download(error.to_string()))?;
    if !response.status().is_success() {
        return Err(UpdateError::HttpStatus {
            resource,
            status: response.status().as_u16(),
            url: url.to_string(),
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(UpdateError::ResponseTooLarge { resource, limit });
    }
    let mut body = Vec::with_capacity(limit.min(64 * 1024));
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| UpdateError::Download(error.to_string()))?;
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err(UpdateError::ResponseTooLarge { resource, limit });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn verify_candidate_file(
    candidate: &NamedTempFile,
    asset: &str,
    version: &Version,
    client: &reqwest::Client,
    github_base_url: &str,
) -> Result<(), UpdateError> {
    let checksum_url = platform::checksum_url(github_base_url, &version.to_string(), asset);
    let checksum = bounded_get(client, &checksum_url, MAX_CHECKSUM_BODY_BYTES, "checksum")
        .await
        .map_err(|error| match error {
            UpdateError::Download(detail) => UpdateError::Checksum(detail),
            UpdateError::HttpStatus { status, url, .. } => {
                UpdateError::Checksum(format!("HTTP {status}: {url}"))
            }
            UpdateError::ResponseTooLarge { limit, .. } => {
                UpdateError::Checksum(format!("response body exceeds {limit} bytes"))
            }
            error => error,
        })?;
    let expected = parse_checksum(&checksum, asset)?;
    let actual = sha256_file(candidate.path())?;
    if actual != expected {
        return Err(UpdateError::Checksum(format!(
            "digest mismatch for {asset}"
        )));
    }
    set_executable(candidate.path())?;
    verify_candidate(candidate.path(), version).await
}

fn parse_checksum(body: &[u8], asset: &str) -> Result<String, UpdateError> {
    let text =
        std::str::from_utf8(body).map_err(|error| UpdateError::Checksum(error.to_string()))?;
    let mut lines = text.lines();
    let line = lines
        .next()
        .ok_or_else(|| UpdateError::Checksum("checksum file is empty".to_string()))?;
    if lines.next().is_some() {
        return Err(UpdateError::Checksum(
            "checksum file must contain exactly one line".to_string(),
        ));
    }
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.is_empty()
        || fields.len() > 2
        || fields[0].len() != 64
        || !fields[0].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(UpdateError::Checksum(format!(
            "invalid checksum file for {asset}"
        )));
    }
    if fields.get(1).is_some_and(|name| *name != asset) {
        return Err(UpdateError::Checksum(format!(
            "checksum filename does not match {asset}"
        )));
    }
    Ok(fields[0].to_ascii_lowercase())
}

fn sha256_file(path: &Path) -> Result<String, UpdateError> {
    let mut file = File::open(path).map_err(UpdateError::Filesystem)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(UpdateError::Filesystem)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

async fn verify_candidate(path: &Path, expected: &Version) -> Result<(), UpdateError> {
    let mut child = Command::new(path)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| UpdateError::CandidateExecution(error.to_string()))?;
    let stdout = child.stdout.take().ok_or_else(|| {
        UpdateError::CandidateExecution("candidate stdout unavailable".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        UpdateError::CandidateExecution("candidate stderr unavailable".to_string())
    })?;
    let result = tokio::time::timeout(CANDIDATE_TIMEOUT, async move {
        let (stdout, stderr, status) =
            tokio::join!(read_bounded(stdout), read_bounded(stderr), child.wait());
        let status = status.map_err(|error| UpdateError::CandidateExecution(error.to_string()))?;
        Ok::<_, UpdateError>((stdout?, stderr?, status))
    })
    .await
    .map_err(|_| UpdateError::CandidateExecution("candidate --version timed out".to_string()))??;
    let (stdout, stderr, status) = result;
    let output = String::from_utf8_lossy(&stdout);
    if !status.success() {
        return Err(UpdateError::CandidateExecution(format!(
            "exit status {status}: {}",
            String::from_utf8_lossy(&stderr)
        )));
    }
    let reported = output
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            (fields.next() == Some(platform::CRATE_NAME))
                .then(|| fields.next().map(str::to_string))
                .flatten()
        })
        .ok_or_else(|| {
            UpdateError::CandidateIdentity(format!(
                "output did not identify {}: {output}",
                platform::CRATE_NAME
            ))
        })?;
    let reported = Version::parse(&reported)
        .map_err(|error| UpdateError::CandidateIdentity(error.to_string()))?;
    if &reported != expected {
        return Err(UpdateError::CandidateIdentity(format!(
            "expected {expected}, got {reported}"
        )));
    }
    Ok(())
}

async fn read_bounded<R>(mut reader: R) -> Result<Vec<u8>, UpdateError>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0u8; 4096];
    let mut overflow = false;
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|error| UpdateError::CandidateExecution(error.to_string()))?;
        if read == 0 {
            break;
        }
        if output.len() < MAX_CANDIDATE_OUTPUT_BYTES {
            let remaining = MAX_CANDIDATE_OUTPUT_BYTES - output.len();
            output.extend_from_slice(&buffer[..read.min(remaining)]);
        }
        if output.len() >= MAX_CANDIDATE_OUTPUT_BYTES || read > MAX_CANDIDATE_OUTPUT_BYTES {
            overflow = true;
        }
    }
    if overflow {
        return Err(UpdateError::CandidateExecution(format!(
            "candidate output exceeds {MAX_CANDIDATE_OUTPUT_BYTES} bytes"
        )));
    }
    Ok(output)
}

fn set_executable(path: &Path) -> Result<(), UpdateError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(UpdateError::Filesystem)?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(UpdateError::Filesystem)?;
    }
    Ok(())
}

fn ensure_replacement_permission(path: &Path) -> Result<(), UpdateError> {
    let parent = path.parent().ok_or_else(|| UpdateError::PermissionDenied {
        path: path.to_path_buf(),
        rerun: rerun_command(path),
    })?;
    let probe = parent.join(format!(".eggsearch-update-{}", std::process::id()));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            Ok(())
        }
        Err(_) => Err(UpdateError::PermissionDenied {
            path: path.to_path_buf(),
            rerun: rerun_command(path),
        }),
    }
}

fn rerun_command(path: &Path) -> String {
    #[cfg(unix)]
    {
        format!("sudo {} update", path.display())
    }
    #[cfg(windows)]
    {
        format!(
            "run an elevated PowerShell prompt, then invoke & '{}' update",
            path.display()
        )
    }
}

fn replace_candidate(candidate: &NamedTempFile, destination: &Path) -> Result<(), UpdateError> {
    if std::env::current_exe().ok().as_deref() == Some(destination) {
        self_replace::self_replace(candidate.path()).map_err(|error| UpdateError::Replacement {
            path: destination.to_path_buf(),
            detail: error.to_string(),
        })?;
        return Ok(());
    }
    replace_candidate_path(candidate.path(), destination)
}

fn replace_candidate_path(candidate: &Path, destination: &Path) -> Result<(), UpdateError> {
    if std::env::current_exe().ok().as_deref() == Some(destination) {
        self_replace::self_replace(candidate).map_err(|error| UpdateError::Replacement {
            path: destination.to_path_buf(),
            detail: error.to_string(),
        })?;
        return Ok(());
    }
    let parent = destination
        .parent()
        .ok_or_else(|| UpdateError::Replacement {
            path: destination.to_path_buf(),
            detail: "destination has no parent directory".to_string(),
        })?;
    let mut staged = NamedTempFile::new_in(parent).map_err(UpdateError::Filesystem)?;
    let mut source = File::open(candidate).map_err(UpdateError::Filesystem)?;
    io::copy(&mut source, staged.as_file_mut()).map_err(UpdateError::Filesystem)?;
    staged
        .as_file_mut()
        .flush()
        .map_err(UpdateError::Filesystem)?;
    staged
        .as_file()
        .sync_all()
        .map_err(UpdateError::Filesystem)?;
    set_executable(staged.path())?;
    staged
        .persist(destination)
        .map_err(|error| UpdateError::Replacement {
            path: destination.to_path_buf(),
            detail: error.error.to_string(),
        })?;
    Ok(())
}

fn find_on_path(program: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&paths) {
        let candidate = directory.join(program);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = directory.join(format!("{program}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "eggsearch.exe"
    } else {
        "eggsearch"
    }
}

fn cargo_command(cargo: &Path, version: &Version, root: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(cargo);
    command
        .arg("install")
        .arg(platform::CRATE_NAME)
        .arg("--version")
        .arg(format!("={version}"))
        .arg("--locked")
        .arg("--root")
        .arg(root);
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn test_client(server: &MockServer, current: &str) -> UpdateClient {
        UpdateClient {
            http: reqwest::Client::builder().build().unwrap(),
            endpoints: UpdateEndpoints {
                registry_base_url: server.base_url(),
                github_base_url: server.base_url(),
            },
            current: Version::parse(current).unwrap(),
        }
    }

    fn registry(server: &MockServer, body: &str) {
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/crates/eggsearch");
            then.status(200)
                .header("content-type", "application/json")
                .body(body);
        });
    }

    #[cfg(unix)]
    fn candidate_bytes(version: &str) -> Vec<u8> {
        format!("#!/bin/sh\nprintf 'eggsearch {version}\\n'\n").into_bytes()
    }

    fn checksum(bytes: &[u8], asset: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}  {asset}\n", hasher.finalize())
    }

    #[test]
    fn stable_versions_and_prereleases_are_classified() {
        assert_eq!(
            parse_stable_version("1.2.3").unwrap(),
            Version::new(1, 2, 3)
        );
        assert!(parse_stable_version("1.2.3-rc.1").is_err());
        assert!(parse_stable_version("not-semver").is_err());
    }

    #[tokio::test]
    async fn comparison_outcomes_do_not_download() {
        let server = MockServer::start();
        registry(&server, r#"{"crate":{"max_stable_version":"0.3.9"}}"#);
        let client = test_client(&server, "0.3.8");
        assert_eq!(
            client.execute(true, None).await.unwrap(),
            UpdateOutcome::UpdateAvailable {
                current: Version::parse("0.3.8").unwrap(),
                latest: Version::parse("0.3.9").unwrap()
            }
        );

        let server = MockServer::start();
        registry(&server, r#"{"crate":{"max_stable_version":"0.3.8"}}"#);
        assert_eq!(
            test_client(&server, "0.3.8")
                .execute(true, None)
                .await
                .unwrap(),
            UpdateOutcome::AlreadyCurrent {
                version: Version::parse("0.3.8").unwrap()
            }
        );

        let server = MockServer::start();
        registry(&server, r#"{"crate":{"max_stable_version":"0.3.7"}}"#);
        assert_eq!(
            test_client(&server, "0.3.8")
                .execute(true, None)
                .await
                .unwrap(),
            UpdateOutcome::LocalVersionAhead {
                current: Version::parse("0.3.8").unwrap(),
                registry: Version::parse("0.3.7").unwrap()
            }
        );
    }

    #[tokio::test]
    async fn malformed_registry_and_body_cap_fail_closed() {
        let server = MockServer::start();
        registry(&server, r#"{"crate":{}}"#);
        assert!(matches!(
            test_client(&server, "0.3.8").execute(true, None).await,
            Err(UpdateError::VersionLookup(_))
        ));

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/crates/eggsearch");
            then.status(200)
                .body("x".repeat(MAX_REGISTRY_BODY_BYTES + 1));
        });
        assert!(matches!(
            test_client(&server, "0.3.8").execute(true, None).await,
            Err(UpdateError::ResponseTooLarge { .. })
        ));
    }

    #[test]
    fn checksum_parser_accepts_documented_forms_only() {
        assert_eq!(
            parse_checksum(&[b'a'; 63], "asset")
                .unwrap_err()
                .to_string(),
            "checksum verification failed: invalid checksum file for asset"
        );
        let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(parse_checksum(digest.as_bytes(), "asset").unwrap(), digest);
        assert_eq!(
            parse_checksum(format!("{digest}  asset\n").as_bytes(), "asset").unwrap(),
            digest
        );
        assert!(parse_checksum(format!("{digest}  other\n").as_bytes(), "asset").is_err());
        assert!(parse_checksum(format!("{digest}\nextra\n").as_bytes(), "asset").is_err());
    }

    #[test]
    fn cargo_command_is_exact_and_isolated() {
        let command = cargo_command(
            Path::new("cargo"),
            &Version::new(0, 3, 9),
            Path::new("/tmp/root"),
        );
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "install",
                "eggsearch",
                "--version",
                "=0.3.9",
                "--locked",
                "--root",
                "/tmp/root"
            ]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn exact_asset_checksum_identity_and_fixture_replacement_work() {
        let server = MockServer::start();
        let asset = "eggsearch-x86_64-unknown-linux-gnu";
        let bytes = candidate_bytes("0.3.9");
        registry(&server, r#"{"crate":{"max_stable_version":"0.3.9"}}"#);
        server.mock(|when, then| {
            when.method(GET)
                .path(format!("/releases/download/v0.3.9/{asset}"));
            then.status(200).body(bytes.clone());
        });
        server.mock(|when, then| {
            when.method(GET)
                .path(format!("/releases/download/v0.3.9/{asset}.sha256"));
            then.status(200).body(checksum(&bytes, asset));
        });
        let directory = tempdir().unwrap();
        let destination = directory.path().join("eggsearch");
        fs::write(&destination, b"old").unwrap();
        let mut permissions = fs::metadata(&destination).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&destination, permissions).unwrap();
        let client = test_client(&server, "0.3.8");
        let target = ReleaseTarget {
            rust_target: "x86_64-unknown-linux-gnu",
            asset,
            os: "linux",
            arch: "x86_64",
        };
        let candidate = client
            .download_release(&Version::new(0, 3, 9), target, &destination)
            .await
            .unwrap();
        verify_candidate_file(
            &candidate,
            asset,
            &Version::new(0, 3, 9),
            &client.http,
            &client.endpoints.github_base_url,
        )
        .await
        .unwrap();
        replace_candidate(&candidate, &destination).unwrap();
        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "#!/bin/sh\nprintf 'eggsearch 0.3.9\\n'\n"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn checksum_mismatch_and_candidate_mismatch_stop_before_replacement() {
        let server = MockServer::start();
        let asset = "eggsearch-x86_64-unknown-linux-gnu";
        let bytes = candidate_bytes("0.3.8");
        registry(&server, r#"{"crate":{"max_stable_version":"0.3.9"}}"#);
        server.mock(|when, then| {
            when.method(GET)
                .path(format!("/releases/download/v0.3.9/{asset}"));
            then.status(200).body(bytes.clone());
        });
        server.mock(|when, then| {
            when.method(GET)
                .path(format!("/releases/download/v0.3.9/{asset}.sha256"));
            then.status(200).body(checksum(b"not-the-candidate", asset));
        });
        let directory = tempdir().unwrap();
        let destination = directory.path().join("eggsearch");
        fs::write(&destination, b"old").unwrap();
        let client = test_client(&server, "0.3.8");
        let target = ReleaseTarget {
            rust_target: "x86_64-unknown-linux-gnu",
            asset,
            os: "linux",
            arch: "x86_64",
        };
        let candidate = client
            .download_release(&Version::new(0, 3, 9), target, &destination)
            .await
            .unwrap();
        let error = verify_candidate_file(
            &candidate,
            asset,
            &Version::new(0, 3, 9),
            &client.http,
            &client.endpoints.github_base_url,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, UpdateError::Checksum(_)));
        assert_eq!(fs::read(&destination).unwrap(), b"old");

        let candidate = NamedTempFile::new_in(directory.path()).unwrap();
        fs::write(candidate.path(), candidate_bytes("0.3.8")).unwrap();
        set_executable(candidate.path()).unwrap();
        assert!(matches!(
            verify_candidate(candidate.path(), &Version::new(0, 3, 9)).await,
            Err(UpdateError::CandidateIdentity(_))
        ));
    }

    #[tokio::test]
    async fn asset_404_is_the_only_download_failure_eligible_for_cargo() {
        let server = MockServer::start();
        registry(&server, r#"{"crate":{"max_stable_version":"0.3.9"}}"#);
        server.mock(|when, then| {
            when.method(GET)
                .path("/releases/download/v0.3.9/eggsearch-x86_64-unknown-linux-gnu");
            then.status(404);
        });
        let directory = tempdir().unwrap();
        let result = test_client(&server, "0.3.8")
            .download_release(
                &Version::new(0, 3, 9),
                ReleaseTarget {
                    rust_target: "x86_64-unknown-linux-gnu",
                    asset: "eggsearch-x86_64-unknown-linux-gnu",
                    os: "linux",
                    arch: "x86_64",
                },
                &directory.path().join("eggsearch"),
            )
            .await;
        assert!(matches!(result, Err(UpdateError::AssetUnavailable { .. })));

        let server = MockServer::start();
        registry(&server, r#"{"crate":{"max_stable_version":"0.3.9"}}"#);
        server.mock(|when, then| {
            when.method(GET)
                .path("/releases/download/v0.3.9/eggsearch-x86_64-unknown-linux-gnu");
            then.status(500);
        });
        let result = test_client(&server, "0.3.8")
            .download_release(
                &Version::new(0, 3, 9),
                ReleaseTarget {
                    rust_target: "x86_64-unknown-linux-gnu",
                    asset: "eggsearch-x86_64-unknown-linux-gnu",
                    os: "linux",
                    arch: "x86_64",
                },
                &directory.path().join("eggsearch"),
            )
            .await;
        assert!(matches!(
            result,
            Err(UpdateError::HttpStatus { status: 500, .. })
        ));
    }

    #[test]
    fn permission_error_contains_resolved_elevated_command() {
        let error = UpdateError::PermissionDenied {
            path: PathBuf::from("/usr/local/bin/eggsearch"),
            rerun: "sudo /usr/local/bin/eggsearch update".to_string(),
        };
        assert!(error
            .to_string()
            .contains("sudo /usr/local/bin/eggsearch update"));
    }
}
