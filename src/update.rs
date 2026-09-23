//! Binary-first self-update orchestration.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use semver::Version;
#[cfg(test)]
use sha2::{Digest, Sha256};
#[cfg(test)]
use tempfile::NamedTempFile;
use tempfile::TempDir;
use thiserror::Error;

use eggup_acquisition::{
    AcquisitionRequest, AcquisitionTransport, CancelFlag, FetchLimits, FetchOutcome,
};
use eggup_core::{
    AbsentPolicy, ArtifactMember, ArtifactSet, CommitOwnership, ExactIdentityValidator,
    InstallPlan, IntegrityRequirement, MemberId, Ownership, OwnershipVerifier, PermissionsIntent,
    ProductId, ReleaseId, TransactionDisposition,
};
use eggup_eggfetch::{EggfetchConfig, EggfetchTransport, ProxyDecision};

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
    /// A verified update replaced the executable and restarted the managed service.
    UpdatedAndRestarted {
        from: Version,
        to: Version,
        method: crate::startup::StartupMethod,
    },
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
            Self::UpdatedAndRestarted { from, to, method } => write!(formatter, "updated eggsearch {from} -> {to} and restarted the {method} service"),
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
    /// Managed-service state could not be queried or verified.
    #[error("managed-service lifecycle operation failed: {0}")]
    Lifecycle(String),
    /// Replacement succeeded but the previously running service did not restart.
    #[error("eggsearch {to} was installed, but the {method} service did not restart: {detail}\nrerun:\n  {command}")]
    RestartFailed {
        from: Box<Version>,
        to: Box<Version>,
        method: crate::startup::StartupMethod,
        command: Box<String>,
        detail: Box<String>,
    },
}

struct UpdateEndpoints {
    registry_base_url: String,
    github_base_url: String,
}

struct UpdateClient {
    http: eggfetch_core::Client,
    endpoints: UpdateEndpoints,
    current: Version,
}

fn request_timeout() -> eggfetch_core::Timeout {
    eggfetch_core::Timeout {
        pool: Some(REQUEST_TIMEOUT),
        connect: Some(REQUEST_TIMEOUT),
        write: Some(REQUEST_TIMEOUT),
        read: Some(REQUEST_TIMEOUT),
        total: Some(REQUEST_TIMEOUT),
    }
}

fn bounded_fetch_error(
    error: eggfetch_core::Error,
    resource: &'static str,
    limit: usize,
) -> UpdateError {
    if matches!(error, eggfetch_core::Error::DecodedBodyTooLarge) {
        return UpdateError::ResponseTooLarge { resource, limit };
    }
    UpdateError::Download(error.to_string())
}

impl UpdateClient {
    fn new() -> Result<Self, UpdateError> {
        let http = eggfetch_core::Client::builder()
            .user_agent(&format!("eggsearch/{CURRENT_VERSION} self-update"))
            .timeout(request_timeout())
            .redirect_policy(eggfetch_core::RedirectPolicy::strict(10))
            .build();
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
        self.execute_with_lifecycle(check, destination, None, false)
            .await
    }

    async fn execute_with_lifecycle(
        &self,
        check: bool,
        destination: Option<PathBuf>,
        config: Option<&Path>,
        manage_lifecycle: bool,
    ) -> Result<UpdateOutcome, UpdateError> {
        let lifecycle = if !check && manage_lifecycle {
            Some(
                crate::startup::startup_state(config)
                    .await
                    .map_err(|error| UpdateError::Lifecycle(error.to_string()))?,
            )
        } else {
            None
        };
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
                    match self.download_release(&latest, target).await {
                        Ok((candidate_root, candidate)) => {
                            let digest = verify_candidate_file(
                                &candidate,
                                target.asset,
                                &latest,
                                &self.endpoints.github_base_url,
                            )
                            .await?;
                            commit_candidate(&candidate, &destination, &latest, digest)?;
                            drop(candidate_root);
                            self.finish_lifecycle(
                                UpdateOutcome::UpdatedBinary {
                                    from: self.current.clone(),
                                    to: latest,
                                },
                                lifecycle.as_ref(),
                                config,
                            )
                            .await
                        }
                        Err(UpdateError::AssetUnavailable { .. }) => {
                            let outcome = self.update_from_cargo(&latest, &destination).await?;
                            self.finish_lifecycle(outcome, lifecycle.as_ref(), config)
                                .await
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    let outcome = self.update_from_cargo(&latest, &destination).await?;
                    self.finish_lifecycle(outcome, lifecycle.as_ref(), config)
                        .await
                }
            }
        }
    }

    async fn finish_lifecycle(
        &self,
        outcome: UpdateOutcome,
        lifecycle: Option<&crate::startup::StartupState>,
        config: Option<&Path>,
    ) -> Result<UpdateOutcome, UpdateError> {
        let Some(state) = lifecycle else {
            return Ok(outcome);
        };
        let Some(method) = state.method else {
            return Ok(outcome);
        };
        let should_restart = state.registered
            && state.healthy
            && (state.running || method == crate::startup::StartupMethod::Cron);
        if !should_restart {
            return Ok(outcome);
        }
        let (from, to) = match &outcome {
            UpdateOutcome::UpdatedBinary { from, to }
            | UpdateOutcome::UpdatedFromCargo { from, to } => (from.clone(), to.clone()),
            _ => return Ok(outcome),
        };
        let command = crate::startup::restart_command(config)
            .map_err(|error| UpdateError::Lifecycle(error.to_string()))?;
        if let Err(error) = crate::startup::restart(config).await {
            return Err(UpdateError::RestartFailed {
                from: Box::new(from),
                to: Box::new(to),
                method,
                command: Box::new(command),
                detail: Box::new(error.to_string()),
            });
        }
        Ok(UpdateOutcome::UpdatedAndRestarted { from, to, method })
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
    ) -> Result<(TempDir, PathBuf), UpdateError> {
        let url = platform::asset_url(
            &self.endpoints.github_base_url,
            &version.to_string(),
            target.asset,
        );
        let root = tempfile::tempdir().map_err(UpdateError::Filesystem)?;
        let candidate = root.path().join(target.asset);
        let request = AcquisitionRequest::new(url.clone())
            .map_err(|error| UpdateError::Download(error.to_string()))?;
        let limits = FetchLimits::new(
            MAX_CHECKSUM_BODY_BYTES,
            Some(MAX_ASSET_BYTES as u64),
            REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        )
        .map_err(|error| UpdateError::Download(error.to_string()))?;
        let transport = EggfetchTransport::strict(
            EggfetchConfig::strict()
                .user_agent(format!("eggsearch/{CURRENT_VERSION} self-update"))
                .timeouts(REQUEST_TIMEOUT, REQUEST_TIMEOUT)
                .max_redirects(10)
                .proxy(ProxyDecision::FromEnvironment),
        )
        .map_err(|error| UpdateError::Client(error.to_string()))?;
        let request_for_task = request.clone();
        let candidate_for_task = candidate.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            transport.fetch_artifact(
                &request_for_task,
                &candidate_for_task,
                limits,
                &CancelFlag::new(),
            )
        })
        .await
        .map_err(|error| UpdateError::Download(error.to_string()))?
        .map_err(|error| match error {
            eggup_acquisition::AcquisitionError::TooLarge { limit } => {
                UpdateError::ResponseTooLarge {
                    resource: "release asset",
                    limit: limit as usize,
                }
            }
            error => UpdateError::Download(error.to_string()),
        })?;
        match outcome {
            FetchOutcome::Success(_) => Ok((root, candidate)),
            FetchOutcome::NotFound => Err(UpdateError::AssetUnavailable { url }),
        }
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
        let digest = eggup_core::hash_file(&candidate)
            .map_err(|error| UpdateError::Checksum(error.to_string()))?;
        commit_candidate(&candidate, destination, version, digest)?;
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

/// Run an update while preserving the lifecycle state of a managed service.
pub async fn run_with_config(
    check: bool,
    config: Option<&Path>,
) -> Result<UpdateOutcome, UpdateError> {
    UpdateClient::new()?
        .execute_with_lifecycle(check, None, config, true)
        .await
}

fn parse_stable_version(raw: &str) -> Result<Version, String> {
    let version = Version::parse(raw).map_err(|error| error.to_string())?;
    if !version.pre.is_empty() {
        return Err("pre-release versions are not eligible for automatic update".to_string());
    }
    Ok(version)
}

async fn bounded_get(
    client: &eggfetch_core::Client,
    url: &str,
    limit: usize,
    resource: &'static str,
) -> Result<Vec<u8>, UpdateError> {
    let mut response = client
        .get(url)
        .map_err(|error| UpdateError::Download(error.to_string()))?
        .timeout(request_timeout())
        .max_decoded_body_size(limit)
        .send()
        .await
        .map_err(|error| bounded_fetch_error(error, resource, limit))?;
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
    response
        .bytes()
        .await
        .map(|body| body.to_vec())
        .map_err(|error| bounded_fetch_error(error, resource, limit))
}

async fn verify_candidate_file(
    candidate: &Path,
    asset: &str,
    version: &Version,
    github_base_url: &str,
) -> Result<[u8; 32], UpdateError> {
    let checksum_url = platform::checksum_url(github_base_url, &version.to_string(), asset);
    let request = AcquisitionRequest::new(checksum_url)
        .map_err(|error| UpdateError::Checksum(error.to_string()))?;
    let limits = FetchLimits::new(
        MAX_CHECKSUM_BODY_BYTES,
        None,
        REQUEST_TIMEOUT,
        REQUEST_TIMEOUT,
    )
    .map_err(|error| UpdateError::Checksum(error.to_string()))?;
    let transport = EggfetchTransport::strict(
        EggfetchConfig::strict()
            .user_agent(format!("eggsearch/{CURRENT_VERSION} self-update"))
            .timeouts(REQUEST_TIMEOUT, REQUEST_TIMEOUT)
            .max_redirects(10)
            .proxy(ProxyDecision::FromEnvironment),
    )
    .map_err(|error| UpdateError::Client(error.to_string()))?;
    let request_for_task = request.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        transport.fetch_metadata(&request_for_task, limits, &CancelFlag::new())
    })
    .await
    .map_err(|error| UpdateError::Checksum(error.to_string()))?
    .map_err(|error| UpdateError::Checksum(error.to_string()))?;
    let checksum = match outcome {
        FetchOutcome::Success(bytes) => bytes.bytes().to_vec(),
        FetchOutcome::NotFound => {
            return Err(UpdateError::Checksum(format!(
                "checksum sidecar not found: {}",
                request.redacted()
            )))
        }
    };
    let text =
        std::str::from_utf8(&checksum).map_err(|error| UpdateError::Checksum(error.to_string()))?;
    let manifest = eggup_core::parse_sha256_sidecar(text)
        .map_err(|error| UpdateError::Checksum(error.to_string()))?;
    if manifest
        .filename()
        .is_some_and(|filename| filename != asset)
    {
        return Err(UpdateError::Checksum(format!(
            "checksum filename does not match {asset}"
        )));
    }
    eggup_core::verify_file(candidate, &manifest)
        .map_err(|error| UpdateError::Checksum(error.to_string()))
}

fn commit_candidate(
    candidate: &Path,
    destination: &Path,
    version: &Version,
    digest: [u8; 32],
) -> Result<(), UpdateError> {
    let verifier =
        CurrentExecutableVerifier::new().map_err(|error| replacement_error(destination, error))?;
    commit_candidate_with_verifier(candidate, destination, version, digest, &verifier)
}

fn commit_candidate_with_verifier(
    candidate: &Path,
    destination: &Path,
    version: &Version,
    digest: [u8; 32],
    verifier: &dyn OwnershipVerifier,
) -> Result<(), UpdateError> {
    let root = destination
        .parent()
        .ok_or_else(|| UpdateError::Replacement {
            path: destination.to_path_buf(),
            detail: "destination has no parent directory".into(),
        })?;
    let name = destination
        .file_name()
        .ok_or_else(|| UpdateError::Replacement {
            path: destination.to_path_buf(),
            detail: "destination has no filename".into(),
        })?;
    let member =
        MemberId::new("eggsearch").map_err(|error| replacement_error(destination, error))?;
    let artifact = ArtifactMember::new(member.clone(), candidate, Path::new(name))
        .map_err(|error| replacement_error(destination, error))?
        .with_permissions(if cfg!(unix) {
            PermissionsIntent::Executable
        } else {
            PermissionsIntent::Preserve
        })
        .with_integrity(IntegrityRequirement::Sha256(digest));
    let plan = InstallPlan::new(
        ProductId::new("eggsearch").map_err(|error| replacement_error(destination, error))?,
        ReleaseId::new(version.to_string())
            .map_err(|error| replacement_error(destination, error))?,
        root,
        ArtifactSet::single(artifact).map_err(|error| replacement_error(destination, error))?,
    )
    .map_err(|error| replacement_error(destination, error))?;
    let prepared = plan
        .prepare()
        .and_then(|prepared| prepared.verify_integrity())
        .map_err(|error| replacement_error(destination, error))?;
    let validator = ExactIdentityValidator::new(member.clone(), format!("eggsearch {version}\n"))
        .args(["--version"])
        .timeout(CANDIDATE_TIMEOUT)
        .max_output_bytes(MAX_CANDIDATE_OUTPUT_BYTES);
    let validated = prepared
        .validate(&validator)
        .map_err(|error| UpdateError::CandidateIdentity(error.to_string()))?;

    #[cfg(windows)]
    if current_executable_matches(destination) {
        let staged = validated
            .staged_path(&member)
            .map_err(|error| replacement_error(destination, error))?;
        self_replace::self_replace(staged).map_err(|error| UpdateError::Replacement {
            path: destination.to_path_buf(),
            detail: error.to_string(),
        })?;
        return Ok(());
    }

    let receipt = validated
        .commit(CommitOwnership::new(verifier, AbsentPolicy::DenyCreate))
        .map_err(|error| replacement_error(destination, error))?;
    match receipt.disposition() {
        TransactionDisposition::Committed => Ok(()),
        TransactionDisposition::RolledBack => Err(UpdateError::Replacement {
            path: destination.to_path_buf(),
            detail: format!(
                "replacement rolled back: {}",
                receipt
                    .failure()
                    .map_or("unknown", |failure| failure.detail())
            ),
        }),
        TransactionDisposition::RecoveryRequired => Err(UpdateError::Replacement {
            path: destination.to_path_buf(),
            detail: format!(
                "recovery required at {}: {}",
                receipt
                    .recovery_path()
                    .map_or_else(|| "unknown path".into(), |path| path.display().to_string()),
                receipt
                    .failure()
                    .map_or("unknown failure", |failure| failure.detail())
            ),
        }),
    }
}

fn replacement_error(path: &Path, error: impl fmt::Display) -> UpdateError {
    UpdateError::Replacement {
        path: path.to_path_buf(),
        detail: error.to_string(),
    }
}

#[cfg(windows)]
fn current_executable_matches(path: &Path) -> bool {
    std::env::current_exe()
        .and_then(fs::canonicalize)
        .ok()
        .zip(fs::canonicalize(path).ok())
        .is_some_and(|(current, destination)| current == destination)
}

struct CurrentExecutableVerifier {
    expected: PathBuf,
}

impl CurrentExecutableVerifier {
    fn new() -> io::Result<Self> {
        Ok(Self {
            expected: fs::canonicalize(std::env::current_exe()?)?,
        })
    }
}

impl OwnershipVerifier for CurrentExecutableVerifier {
    fn verify(&self, _member: &MemberId, destination: &Path) -> Ownership {
        let metadata = match fs::symlink_metadata(destination) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
            Ok(_) => return Ownership::Foreign,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ownership::Absent,
            Err(_) => return Ownership::Unknown,
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() != 1 {
                return Ownership::Foreign;
            }
        }
        match fs::canonicalize(destination) {
            Ok(path) if path == self.expected => Ownership::Owned,
            Ok(_) => Ownership::Foreign,
            Err(_) => Ownership::Unknown,
        }
    }
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
            http: eggfetch_core::Client::builder().build(),
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

    #[cfg(unix)]
    #[test]
    fn eggup_candidate_runner_enforces_output_bound() {
        let output = eggup_core::run_bounded(
            &eggup_core::CommandSpec::new("/bin/sh")
                .args(["-c", "printf ab"])
                .timeout(CANDIDATE_TIMEOUT)
                .max_output_bytes(1),
        )
        .unwrap();
        assert!(output.output_limited());
        assert!(!output.success());
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
    fn eggup_sidecar_parser_binds_the_selected_filename() {
        let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let manifest = eggup_core::parse_sha256_sidecar(&format!("{digest}  asset\n")).unwrap();
        assert_eq!(manifest.filename(), Some("asset"));
        assert!(eggup_core::parse_sha256_sidecar(&format!("{digest}  other\n")).is_ok());
        assert!(eggup_core::parse_sha256_sidecar(&format!("{digest}\nextra\n")).is_err());
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
        let (candidate_root, candidate) = client
            .download_release(&Version::new(0, 3, 9), target)
            .await
            .unwrap();
        let digest = verify_candidate_file(
            &candidate,
            asset,
            &Version::new(0, 3, 9),
            &client.endpoints.github_base_url,
        )
        .await
        .unwrap();
        assert_eq!(digest, eggup_core::hash_file(&candidate).unwrap());
        commit_candidate_with_verifier(
            &candidate,
            &destination,
            &Version::new(0, 3, 9),
            digest,
            &eggup_core::ExistingAsOwnedVerifier,
        )
        .unwrap();
        drop(candidate_root);
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
        let (_candidate_root, candidate) = client
            .download_release(&Version::new(0, 3, 9), target)
            .await
            .unwrap();
        let error = verify_candidate_file(
            &candidate,
            asset,
            &Version::new(0, 3, 9),
            &client.endpoints.github_base_url,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, UpdateError::Checksum(_)));
        assert_eq!(fs::read(&destination).unwrap(), b"old");

        let candidate_directory = tempdir().unwrap();
        let candidate = NamedTempFile::new_in(candidate_directory.path()).unwrap();
        fs::write(candidate.path(), candidate_bytes("0.3.8")).unwrap();
        candidate.as_file().sync_all().unwrap();
        let candidate = candidate.into_temp_path();
        let digest = eggup_core::hash_file(candidate.as_ref()).unwrap();
        let result = commit_candidate_with_verifier(
            candidate.as_ref(),
            &destination,
            &Version::new(0, 3, 9),
            digest,
            &eggup_core::ExistingAsOwnedVerifier,
        );
        assert!(
            matches!(result, Err(UpdateError::CandidateIdentity(_))),
            "wrong candidate should fail identity validation, got {result:?}"
        );
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
        let result = test_client(&server, "0.3.8")
            .download_release(
                &Version::new(0, 3, 9),
                ReleaseTarget {
                    rust_target: "x86_64-unknown-linux-gnu",
                    asset: "eggsearch-x86_64-unknown-linux-gnu",
                    os: "linux",
                    arch: "x86_64",
                },
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
            )
            .await;
        assert!(matches!(result, Err(UpdateError::Download(_))));
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
