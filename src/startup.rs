//! Persistent startup supervision for the loopback MCP server.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::Duration;

use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;

use crate::core::config::default_config_path;
use crate::mcp::http::{McpPath, DEFAULT_BIND, DEFAULT_PATH, HEALTH_PATH};

const SERVICE_LABEL: &str = "com.eggstack.eggsearch";
const SERVICE_NAME: &str = "Eggsearch";
const CRON_MARKER: &str = "# eggsearch-managed";
const SYSTEMD_UNIT: &str = "/etc/systemd/system/eggsearch.service";
const SYSTEMD_CONFIG: &str = "/etc/eggsearch/eggsearch.toml";
const HEALTH_TIMEOUT: Duration = Duration::from_millis(800);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(100);
const HEALTH_POLL_ATTEMPTS: usize = 30;

const SYSTEMD_TEMPLATE: &str = include_str!("../packaging/systemd/eggsearch.service");
const LAUNCHD_TEMPLATE: &str = include_str!("../packaging/launchd/com.eggstack.eggsearch.plist");

/// The startup manager used by a persistent eggsearch instance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StartupMethod {
    /// Select the native manager for the current host.
    Auto,
    /// Linux system service managed by systemd.
    Systemd,
    /// Per-user macOS LaunchAgent managed by launchd.
    Launchd,
    /// User crontab watchdog fallback.
    Cron,
    /// Windows Service Control Manager.
    Windows,
}

impl fmt::Display for StartupMethod {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Auto => "auto",
            Self::Systemd => "systemd",
            Self::Launchd => "launchd",
            Self::Cron => "cron",
            Self::Windows => "windows",
        })
    }
}

impl std::str::FromStr for StartupMethod {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "systemd" => Ok(Self::Systemd),
            "launchd" => Ok(Self::Launchd),
            "cron" => Ok(Self::Cron),
            "windows" => Ok(Self::Windows),
            _ => Err(format!("unknown startup method: {value}")),
        }
    }
}

/// The host facts used by automatic manager selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostOs {
    /// Linux.
    Linux,
    /// macOS.
    Macos,
    /// Windows.
    Windows,
    /// Another Unix-like host.
    OtherUnix,
}

/// Injectable platform facts for startup policy tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformInfo {
    /// Operating-system family.
    pub os: HostOs,
    /// Whether systemd is actually running and usable.
    pub systemd_active: bool,
    /// Whether launchctl is available.
    pub launchd_available: bool,
    /// Whether the Windows SCM command is available.
    pub windows_scm_available: bool,
}

/// The canonical persistent server command and health endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSpec {
    /// Absolute executable path.
    pub executable: PathBuf,
    /// Absolute configuration path.
    pub config: PathBuf,
    /// Loopback bind address.
    pub bind: SocketAddr,
    /// MCP endpoint path.
    pub path: McpPath,
    /// Optional manager-owned PID record.
    pub pid_file: Option<PathBuf>,
}

impl RuntimeSpec {
    /// Construct the canonical runtime specification.
    pub fn new(executable: PathBuf, config: PathBuf, pid_file: Option<PathBuf>) -> Self {
        Self {
            executable,
            config,
            bind: DEFAULT_BIND,
            path: McpPath::from_str(DEFAULT_PATH).expect("default MCP path is valid"),
            pid_file,
        }
    }

    /// Construct a specification for the current executable.
    pub fn current(config: Option<&Path>, pid_file: Option<PathBuf>) -> io::Result<Self> {
        let executable = fs::canonicalize(std::env::current_exe()?)?;
        let configured = config
            .map(Path::to_path_buf)
            .unwrap_or_else(default_config_path);
        let config = fs::canonicalize(&configured).unwrap_or_else(|_| absolute_path(&configured));
        let spec = Self::new(executable, config, pid_file);
        if spec
            .executable
            .to_string_lossy()
            .chars()
            .chain(spec.config.to_string_lossy().chars())
            .any(|character| character == '\0' || character == '\n' || character == '\r')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "runtime paths contain forbidden control characters",
            ));
        }
        Ok(spec)
    }

    /// Return the argv used to launch the persistent server.
    pub fn args(&self) -> Vec<String> {
        let mut args = vec![
            "--config".to_string(),
            self.config.display().to_string(),
            "mcp".to_string(),
            "serve".to_string(),
            "--bind".to_string(),
            self.bind.to_string(),
            "--path".to_string(),
            self.path.to_string(),
        ];
        if let Some(pid_file) = &self.pid_file {
            args.push("--pid-file".to_string());
            args.push(pid_file.display().to_string());
        }
        args
    }

    /// Return the local health URL.
    pub fn health_url(&self) -> String {
        format!("http://{}{}", self.bind, HEALTH_PATH)
    }

    /// Render a shell-safe command line for operator instructions.
    pub fn command_line(&self) -> String {
        std::iter::once(shell_quote(&self.executable))
            .chain(self.args().iter().map(|arg| shell_quote(Path::new(arg))))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// The observed health state of the persistent endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HealthState {
    /// The endpoint identifies a ready eggsearch server.
    Healthy,
    /// No listener accepted the local connection.
    Refused,
    /// The local endpoint did not respond within the bound.
    Timeout,
    /// The listener returned another service identity.
    WrongService,
    /// The response was not valid eggsearch health JSON.
    Malformed,
    /// Eggsearch responded but was not ready.
    NonReady,
    /// Another local transport error occurred.
    Error(String),
}

impl fmt::Display for HealthState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Healthy => formatter.write_str("healthy"),
            Self::Refused => formatter.write_str("connection refused"),
            Self::Timeout => formatter.write_str("timeout"),
            Self::WrongService => formatter.write_str("wrong service identity"),
            Self::Malformed => formatter.write_str("malformed health response"),
            Self::NonReady => formatter.write_str("eggsearch is not ready"),
            Self::Error(detail) => formatter.write_str(detail),
        }
    }
}

/// The state of registered persistent startup supervision.
#[derive(Clone, Debug, Default, Serialize)]
pub struct StartupState {
    /// The registered manager, when exactly one is present.
    pub method: Option<StartupMethod>,
    /// Whether exactly one manager registration exists.
    pub registered: bool,
    /// Whether that manager reports a running instance.
    pub running: bool,
    /// Whether the configured health endpoint identifies a ready instance.
    pub healthy: bool,
    /// Whether more than one manager registration was found.
    pub conflict: bool,
    /// Human-readable diagnostic detail.
    pub detail: String,
}

/// A process-owned PID record guard for cron-managed instances.
pub struct PidFileGuard {
    path: PathBuf,
    contents: String,
}

impl PidFileGuard {
    /// Create or replace the current process's owned PID record.
    pub fn create(path: &Path) -> io::Result<Self> {
        let executable = fs::canonicalize(std::env::current_exe()?)?;
        let contents = process_record_contents(&executable);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if path.exists() {
            if let Ok(record) = read_pid_record(path) {
                if process_matches(&record) {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "an owned eggsearch process is already running",
                    ));
                }
            }
            fs::remove_file(path)?;
        }
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("PID record has no parent directory"))?;
        let mut staged = tempfile::NamedTempFile::new_in(parent)?;
        staged.write_all(contents.as_bytes())?;
        staged.as_file().sync_all()?;
        staged
            .persist(path)
            .map_err(|error| io::Error::other(error.error.to_string()))?;
        Ok(Self {
            path: path.to_path_buf(),
            contents,
        })
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        if fs::read_to_string(&self.path).ok().as_deref() == Some(self.contents.as_str()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Return the host facts used by auto selection.
#[allow(clippy::needless_return)]
pub fn platform_info() -> PlatformInfo {
    #[cfg(windows)]
    {
        return PlatformInfo {
            os: HostOs::Windows,
            systemd_active: false,
            launchd_available: false,
            windows_scm_available: find_program("sc.exe").is_some(),
        };
    }
    #[cfg(target_os = "macos")]
    {
        return PlatformInfo {
            os: HostOs::Macos,
            systemd_active: false,
            launchd_available: find_program("launchctl").is_some(),
            windows_scm_available: false,
        };
    }
    #[cfg(target_os = "linux")]
    {
        return PlatformInfo {
            os: HostOs::Linux,
            systemd_active: systemd_is_active(),
            launchd_available: false,
            windows_scm_available: false,
        };
    }
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "linux")))]
    {
        return PlatformInfo {
            os: HostOs::OtherUnix,
            systemd_active: false,
            launchd_available: false,
            windows_scm_available: false,
        };
    }
    #[cfg(not(unix))]
    {
        PlatformInfo {
            os: HostOs::OtherUnix,
            systemd_active: false,
            launchd_available: false,
            windows_scm_available: false,
        }
    }
}

/// Select a manager according to the documented platform policy.
pub fn select_method(requested: StartupMethod, info: PlatformInfo) -> io::Result<StartupMethod> {
    if requested != StartupMethod::Auto {
        validate_method(requested, info)?;
        return Ok(requested);
    }
    match info.os {
        HostOs::Windows => Ok(StartupMethod::Windows),
        HostOs::Macos => Ok(StartupMethod::Launchd),
        HostOs::Linux if info.systemd_active => Ok(StartupMethod::Systemd),
        HostOs::Linux | HostOs::OtherUnix => Ok(StartupMethod::Cron),
    }
}

/// Probe the configured loopback health endpoint without following redirects.
pub async fn probe_health(spec: &RuntimeSpec) -> HealthState {
    let connect = tokio::time::timeout(HEALTH_TIMEOUT, TcpStream::connect(spec.bind)).await;
    match connect {
        Ok(Ok(_)) => {}
        Ok(Err(error)) if error.kind() == io::ErrorKind::ConnectionRefused => {
            return HealthState::Refused
        }
        Ok(Err(error)) => return HealthState::Error(error.to_string()),
        Err(_) => return HealthState::Timeout,
    }
    let client = match reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(HEALTH_TIMEOUT)
        .build()
    {
        Ok(client) => client,
        Err(error) => return HealthState::Error(error.to_string()),
    };
    let response = match client.get(spec.health_url()).send().await {
        Ok(response) => response,
        Err(error) if error.is_timeout() => return HealthState::Timeout,
        Err(error) => return HealthState::Error(error.to_string()),
    };
    if !response.status().is_success() {
        return HealthState::NonReady;
    }
    let body = match response.bytes().await {
        Ok(body) if body.len() <= 256 => body,
        Ok(_) => return HealthState::Malformed,
        Err(_) => return HealthState::Malformed,
    };
    let health: HealthPayload = match serde_json::from_slice(&body) {
        Ok(health) => health,
        Err(_) => return HealthState::Malformed,
    };
    if health.service != "eggsearch" {
        return HealthState::WrongService;
    }
    if health.status != "ready" {
        return HealthState::NonReady;
    }
    HealthState::Healthy
}

/// Return the current startup registration and health state.
pub async fn startup_state(config: Option<&Path>) -> io::Result<StartupState> {
    let spec = RuntimeSpec::current(config, Some(cron_pid_path()?))?;
    let registrations = registered_methods()?;
    if registrations.len() != 1 {
        return Ok(StartupState {
            registered: !registrations.is_empty(),
            conflict: registrations.len() > 1,
            detail: if registrations.is_empty() {
                "no persistent startup manager is registered".to_string()
            } else {
                format!("multiple startup managers registered: {registrations:?}")
            },
            ..StartupState::default()
        });
    }
    let method = registrations[0];
    let running = manager_running(method)?;
    let health = probe_health(&spec).await;
    Ok(StartupState {
        method: Some(method),
        registered: true,
        running,
        healthy: health == HealthState::Healthy,
        conflict: false,
        detail: format!("{method}: manager running={running}, health={health}"),
    })
}

/// Render non-mutating instructions for a startup method.
pub fn instructions(config: Option<&Path>, requested: StartupMethod) -> io::Result<String> {
    let method = resolve_method(requested)?;
    let pid_file = (method == StartupMethod::Cron)
        .then(cron_pid_path)
        .transpose()?;
    let spec = runtime_spec(config, method, pid_file)?;
    let mut output = format!(
        "detected method: {method}\nexecutable: {}\nconfig: {}\nhealth: GET {}\nruntime: {}\n\n",
        spec.executable.display(),
        spec.config.display(),
        spec.health_url(),
        spec.command_line()
    );
    match method {
        StartupMethod::Systemd => {
            output.push_str(&format!("unit: {SYSTEMD_UNIT}\n\n"));
            output.push_str(&format!(
                "install:\n  sudo {} startup install --method systemd\n",
                shell_quote(&spec.executable)
            ));
            output.push_str(&format!(
                "uninstall:\n  sudo {} startup uninstall --method systemd\n",
                shell_quote(&spec.executable)
            ));
            output.push_str("verify:\n  curl --fail --silent ");
            output.push_str(&spec.health_url());
            output.push('\n');
        }
        StartupMethod::Launchd => {
            let plist = launchd_plist_path()?;
            output.push_str(&format!("plist: {}\n\n", plist.display()));
            output.push_str(&format!(
                "install:\n  {} startup install --method launchd\n",
                shell_quote(&spec.executable)
            ));
            output.push_str(&format!(
                "uninstall:\n  {} startup uninstall --method launchd\n",
                shell_quote(&spec.executable)
            ));
        }
        StartupMethod::Cron => {
            output.push_str(&format!("crontab entry:\n  {}\n\n", cron_line(&spec)));
            output.push_str(&format!(
                "install:\n  {} startup install --method cron\n",
                shell_quote(&spec.executable)
            ));
            output.push_str(&format!(
                "uninstall:\n  {} startup uninstall --method cron\n",
                shell_quote(&spec.executable)
            ));
        }
        StartupMethod::Windows => {
            output.push_str("service: Eggsearch\n\n");
            output.push_str("install from an elevated PowerShell prompt:\n  ");
            output.push_str(&windows_command(&spec, "install"));
            output.push_str("\nuninstall:\n  ");
            output.push_str(&windows_command(&spec, "uninstall"));
        }
        StartupMethod::Auto => unreachable!(),
    }
    Ok(output)
}

/// Install or update an idempotent startup registration and verify health.
pub async fn install(config: Option<&Path>, requested: StartupMethod) -> io::Result<String> {
    let method = resolve_method(requested)?;
    let pid_file = (method == StartupMethod::Cron)
        .then(cron_pid_path)
        .transpose()?;
    let spec = runtime_spec(config, method, pid_file)?;
    match method {
        StartupMethod::Systemd => install_systemd(&spec).await?,
        StartupMethod::Launchd => install_launchd(&spec).await?,
        StartupMethod::Cron => install_cron(&spec).await?,
        StartupMethod::Windows => install_windows(&spec).await?,
        StartupMethod::Auto => unreachable!(),
    }
    wait_for_health(&spec, true)
        .await
        .map_err(io::Error::other)?;
    Ok(format!(
        "{method} startup installed; health: {}",
        spec.health_url()
    ))
}

/// Remove an owned startup registration without touching unrelated state.
pub async fn uninstall(config: Option<&Path>, requested: StartupMethod) -> io::Result<String> {
    let method = resolve_method(requested)?;
    let pid_file = (method == StartupMethod::Cron)
        .then(cron_pid_path)
        .transpose()?;
    let spec = runtime_spec(config, method, pid_file)?;
    match method {
        StartupMethod::Systemd => uninstall_systemd().await?,
        StartupMethod::Launchd => uninstall_launchd().await?,
        StartupMethod::Cron => uninstall_cron(&spec).await?,
        StartupMethod::Windows => uninstall_windows().await?,
        StartupMethod::Auto => unreachable!(),
    }
    Ok(format!("{method} startup uninstalled"))
}

/// Restart the registered persistent instance and verify its health.
pub async fn restart(config: Option<&Path>) -> io::Result<String> {
    let state = startup_state(config).await?;
    if state.conflict {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, state.detail));
    }
    let Some(method) = state.method else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no managed persistent service is registered; stdio processes are client-owned",
        ));
    };
    let pid_file = (method == StartupMethod::Cron)
        .then(cron_pid_path)
        .transpose()?;
    let spec = runtime_spec(config, method, pid_file)?;
    match method {
        StartupMethod::Systemd => command_ok("systemctl", &["restart", "eggsearch.service"])?,
        StartupMethod::Launchd => {
            let domain = launchd_domain()?;
            command_ok(
                "launchctl",
                &["kickstart", "-k", &format!("{domain}/{SERVICE_LABEL}")],
            )?;
        }
        StartupMethod::Cron => restart_cron(&spec).await?,
        StartupMethod::Windows => restart_windows().await?,
        StartupMethod::Auto => unreachable!(),
    }
    wait_for_health(&spec, true)
        .await
        .map_err(io::Error::other)?;
    Ok(format!(
        "restarted {method}; health verified at {}",
        spec.health_url()
    ))
}

/// Return the exact CLI command used to restart a managed instance.
pub fn restart_command(config: Option<&Path>) -> io::Result<String> {
    let spec = RuntimeSpec::current(config, None)?;
    Ok(format!("{} restart", shell_quote(&spec.executable)))
}

/// Run the identity-safe cron watchdog.
pub async fn croncheck(config: Option<&Path>) -> io::Result<String> {
    let pid_file = cron_pid_path()?;
    let spec = RuntimeSpec::current(config, Some(pid_file.clone()))?;
    match probe_health(&spec).await {
        HealthState::Healthy => return Ok("eggsearch is healthy; no action taken".to_string()),
        HealthState::Refused => {}
        state => {
            return Err(io::Error::other(format!(
                "croncheck will not spawn because health is ambiguous: {state}"
            )))
        }
    }
    let lock_path = cron_lock_path()?;
    let _lock = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    {
        Ok(mut lock) => {
            lock.write_all(process_record_contents(&spec.executable).as_bytes())?;
            lock.sync_all()?;
            lock
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if let Ok(record) = read_pid_record(&lock_path) {
                if process_matches(&record) {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "another croncheck is already starting eggsearch",
                    ));
                }
                fs::remove_file(&lock_path)?;
                let mut lock = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&lock_path)?;
                lock.write_all(process_record_contents(&spec.executable).as_bytes())?;
                lock.sync_all()?;
                lock
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "another croncheck is already starting eggsearch",
                ));
            }
        }
        Err(error) => return Err(error),
    };
    let result = async {
        if probe_health(&spec).await != HealthState::Refused {
            return Ok("eggsearch became available while acquiring the startup lock".to_string());
        }
        spawn_detached(&spec)?;
        wait_for_health(&spec, true)
            .await
            .map_err(io::Error::other)?;
        Ok(format!(
            "started eggsearch; health verified at {}",
            spec.health_url()
        ))
    }
    .await;
    let _ = fs::remove_file(lock_path);
    result
}

async fn wait_for_health(spec: &RuntimeSpec, ready: bool) -> Result<(), String> {
    for _ in 0..HEALTH_POLL_ATTEMPTS {
        let state = probe_health(spec).await;
        if (ready && state == HealthState::Healthy) || (!ready && state == HealthState::Refused) {
            return Ok(());
        }
        tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
    }
    Err(format!(
        "health verification failed for {}",
        spec.health_url()
    ))
}

async fn install_systemd(spec: &RuntimeSpec) -> io::Result<()> {
    if !is_privileged() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("systemd requires Administrator/root; rerun: sudo {} startup install --method systemd", spec.executable.display()),
        ));
    }
    let rendered = render_systemd(spec);
    atomic_write(Path::new(SYSTEMD_UNIT), rendered.as_bytes())?;
    command_ok("systemctl", &["daemon-reload"])?;
    command_ok("systemctl", &["enable", "--now", "eggsearch.service"])
}

async fn uninstall_systemd() -> io::Result<()> {
    if !is_privileged() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "systemd uninstall requires root; rerun with sudo",
        ));
    }
    let _ = command_ok("systemctl", &["disable", "--now", "eggsearch.service"]);
    if Path::new(SYSTEMD_UNIT).exists() {
        fs::remove_file(SYSTEMD_UNIT)?;
    }
    command_ok("systemctl", &["daemon-reload"])
}

async fn install_launchd(spec: &RuntimeSpec) -> io::Result<()> {
    let plist = launchd_plist_path()?;
    atomic_write(&plist, render_launchd(spec).as_bytes())?;
    let domain = launchd_domain()?;
    let _ = command_ok(
        "launchctl",
        &["bootout", &format!("{domain}/{SERVICE_LABEL}")],
    );
    command_ok(
        "launchctl",
        &["bootstrap", &domain, plist.to_str().unwrap_or_default()],
    )?;
    command_ok(
        "launchctl",
        &["kickstart", "-k", &format!("{domain}/{SERVICE_LABEL}")],
    )
}

async fn uninstall_launchd() -> io::Result<()> {
    let plist = launchd_plist_path()?;
    let domain = launchd_domain()?;
    let _ = command_ok(
        "launchctl",
        &["bootout", &format!("{domain}/{SERVICE_LABEL}")],
    );
    if plist.exists() {
        fs::remove_file(plist)?;
    }
    Ok(())
}

async fn install_cron(spec: &RuntimeSpec) -> io::Result<()> {
    let current = read_crontab()?;
    let updated = update_crontab(&current, Some(&cron_line(spec)))?;
    if updated != current {
        write_crontab(&updated)?;
    }
    if probe_health(spec).await == HealthState::Refused {
        spawn_detached(spec)?;
        wait_for_health(spec, true)
            .await
            .map_err(io::Error::other)?;
    }
    Ok(())
}

async fn uninstall_cron(spec: &RuntimeSpec) -> io::Result<()> {
    let current = read_crontab()?;
    let updated = update_crontab(&current, None)?;
    if updated != current {
        write_crontab(&updated)?;
    }
    if let Some(pid_file) = &spec.pid_file {
        stop_owned_process(pid_file)?;
    }
    Ok(())
}

async fn restart_cron(spec: &RuntimeSpec) -> io::Result<()> {
    let pid_file = spec
        .pid_file
        .as_deref()
        .ok_or_else(|| io::Error::other("cron PID file is missing"))?;
    stop_owned_process(pid_file)?;
    wait_for_health(spec, false)
        .await
        .map_err(io::Error::other)?;
    spawn_detached(spec)?;
    Ok(())
}

async fn install_windows(spec: &RuntimeSpec) -> io::Result<()> {
    let command = windows_command(spec, "install");
    let _ = command_ok("sc.exe", &["stop", SERVICE_NAME]);
    let _ = command_ok("sc.exe", &["delete", SERVICE_NAME]);
    command_ok(
        "sc.exe",
        &[
            "create",
            SERVICE_NAME,
            "start=",
            "auto",
            "binPath=",
            &windows_bin_path(spec),
        ],
    )
    .map_err(|error| windows_permission_error(error, &command))?;
    command_ok(
        "sc.exe",
        &[
            "description",
            SERVICE_NAME,
            "eggsearch persistent MCP service",
        ],
    )
    .map_err(|error| windows_permission_error(error, &command))?;
    command_ok(
        "sc.exe",
        &[
            "failure",
            SERVICE_NAME,
            "reset=",
            "86400",
            "actions=",
            "restart/5000/restart/30000/none/0",
        ],
    )
    .map_err(|error| windows_permission_error(error, &command))?;
    command_ok("sc.exe", &["start", SERVICE_NAME])
        .map_err(|error| windows_permission_error(error, &command))
}

async fn uninstall_windows() -> io::Result<()> {
    let _ = command_ok("sc.exe", &["stop", SERVICE_NAME]);
    command_ok("sc.exe", &["delete", SERVICE_NAME])
}

async fn restart_windows() -> io::Result<()> {
    let _ = command_ok("sc.exe", &["stop", SERVICE_NAME]);
    command_ok("sc.exe", &["start", SERVICE_NAME])
}

fn registered_methods() -> io::Result<Vec<StartupMethod>> {
    let mut methods = Vec::new();
    #[cfg(target_os = "linux")]
    if Path::new(SYSTEMD_UNIT).is_file() {
        methods.push(StartupMethod::Systemd);
    }
    #[cfg(target_os = "macos")]
    if launchd_plist_path()
        .map(|path| path.is_file())
        .unwrap_or(false)
    {
        methods.push(StartupMethod::Launchd);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if read_crontab()
        .unwrap_or_default()
        .lines()
        .any(|line| line.contains(CRON_MARKER))
    {
        methods.push(StartupMethod::Cron);
    }
    #[cfg(windows)]
    if command_ok("sc.exe", &["query", SERVICE_NAME]).is_ok() {
        methods.push(StartupMethod::Windows);
    }
    Ok(methods)
}

fn manager_running(method: StartupMethod) -> io::Result<bool> {
    match method {
        StartupMethod::Systemd => {
            Ok(command_ok("systemctl", &["is-active", "--quiet", "eggsearch.service"]).is_ok())
        }
        StartupMethod::Launchd => Ok(command_ok(
            "launchctl",
            &["print", &format!("{}/{}", launchd_domain()?, SERVICE_LABEL)],
        )
        .is_ok()),
        StartupMethod::Cron => Ok(false),
        StartupMethod::Windows => Ok(capture_command("sc.exe", &["query", SERVICE_NAME])
            .is_some_and(|output| output.contains("RUNNING"))),
        StartupMethod::Auto => Ok(false),
    }
}

fn validate_method(method: StartupMethod, info: PlatformInfo) -> io::Result<()> {
    let valid = match method {
        StartupMethod::Auto => true,
        StartupMethod::Systemd => info.os == HostOs::Linux && info.systemd_active,
        StartupMethod::Launchd => info.os == HostOs::Macos && info.launchd_available,
        StartupMethod::Cron => matches!(info.os, HostOs::Linux | HostOs::OtherUnix),
        StartupMethod::Windows => info.os == HostOs::Windows && info.windows_scm_available,
    };
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("startup method {method} is unavailable on this host"),
        ))
    }
}

fn resolve_method(requested: StartupMethod) -> io::Result<StartupMethod> {
    if requested == StartupMethod::Auto {
        let registrations = registered_methods()?;
        if registrations.len() > 1 {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("multiple startup managers registered: {registrations:?}"),
            ));
        }
        if let Some(method) = registrations.first() {
            return Ok(*method);
        }
    }
    select_method(requested, platform_info())
}

#[allow(dead_code)]
fn systemd_is_active() -> bool {
    matches!(
        capture_command("systemctl", &["is-system-running"]).as_deref(),
        Some("running") | Some("degraded")
    )
}

fn render_systemd(spec: &RuntimeSpec) -> String {
    SYSTEMD_TEMPLATE.replace("{{EXEC_START}}", &systemd_command(spec))
}

fn runtime_spec(
    config: Option<&Path>,
    method: StartupMethod,
    pid_file: Option<PathBuf>,
) -> io::Result<RuntimeSpec> {
    let mut spec = RuntimeSpec::current(config, pid_file)?;
    if method == StartupMethod::Systemd && config.is_none() {
        spec.config = PathBuf::from(SYSTEMD_CONFIG);
    }
    Ok(spec)
}

fn render_launchd(spec: &RuntimeSpec) -> String {
    let arguments = std::iter::once(spec.executable.display().to_string())
        .chain(spec.args())
        .map(|arg| format!("    <string>{}</string>", xml_escape(&arg)))
        .collect::<Vec<_>>()
        .join("\n");
    let log = launchd_log_path().unwrap_or_else(|_| PathBuf::from("/tmp/eggsearch.log"));
    LAUNCHD_TEMPLATE
        .replace("{{PROGRAM_ARGUMENTS}}", &arguments)
        .replace("{{STDOUT_PATH}}", &xml_escape(&log.display().to_string()))
        .replace("{{STDERR_PATH}}", &xml_escape(&log.display().to_string()))
}

fn systemd_command(spec: &RuntimeSpec) -> String {
    std::iter::once(systemd_quote(&spec.executable.display().to_string()))
        .chain(spec.args().iter().map(|arg| systemd_quote(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn cron_line(spec: &RuntimeSpec) -> String {
    format!(
        "* * * * * {} {}",
        shell_quote(&spec.executable),
        spec.args()
            .iter()
            .map(|arg| shell_quote(Path::new(arg)))
            .collect::<Vec<_>>()
            .join(" ")
    ) + " "
        + CRON_MARKER
}

fn update_crontab(current: &str, replacement: Option<&str>) -> io::Result<String> {
    if current.contains('\0')
        || replacement.is_some_and(|line| {
            line.chars()
                .any(|character| matches!(character, '\n' | '\r' | '\0'))
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cron content contains forbidden control characters",
        ));
    }
    let had_newline = current.ends_with('\n');
    let mut lines = current
        .lines()
        .filter(|line| !line.contains(CRON_MARKER))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if let Some(line) = replacement {
        lines.push(line.to_string());
    }
    let mut output = lines.join("\n");
    if had_newline || replacement.is_some() {
        output.push('\n');
    }
    Ok(output)
}

fn read_crontab() -> io::Result<String> {
    let output = Command::new("crontab").arg("-l").output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8(output.stdout).map_err(io::Error::other)
        }
        Ok(output) if output.stdout.is_empty() => Ok(String::new()),
        Ok(output) => Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        )),
        Err(error) => Err(error),
    }
}

fn write_crontab(content: &str) -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("eggsearch-crontab");
    fs::write(&path, content)?;
    command_ok("crontab", &[path.to_str().unwrap_or_default()])
}

fn spawn_detached(spec: &RuntimeSpec) -> io::Result<()> {
    let log_path = cron_log_path()?;
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let stderr = log.try_clone()?;
    let mut command = Command::new(&spec.executable);
    command
        .args(spec.args())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    command.spawn().map(|_| ())
}

#[allow(clippy::needless_return)]
fn stop_owned_process(path: &Path) -> io::Result<()> {
    let record = read_pid_record(path)?;
    if !process_matches(&record) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "owned PID record does not match a live eggsearch process",
        ));
    }
    #[cfg(unix)]
    {
        if unsafe { libc::kill(record.pid as libc::pid_t, libc::SIGTERM) } != 0 {
            return Err(io::Error::last_os_error());
        }
        return Ok(());
    }
    #[cfg(windows)]
    {
        let _ = record;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "cron process control is unavailable on Windows",
        ))
    }
}

struct PidRecord {
    pid: u32,
    executable: PathBuf,
    start_token: String,
}

fn process_record_contents(executable: &Path) -> String {
    format!(
        "{}\n{}\n{}\n",
        std::process::id(),
        executable.display(),
        process_start_token(std::process::id()).unwrap_or_default()
    )
}

fn read_pid_record(path: &Path) -> io::Result<PidRecord> {
    let mut text = String::new();
    File::open(path)?.read_to_string(&mut text)?;
    let mut lines = text.lines();
    let pid = lines
        .next()
        .ok_or_else(|| io::Error::other("PID record is incomplete"))?
        .parse()
        .map_err(io::Error::other)?;
    let executable = PathBuf::from(
        lines
            .next()
            .ok_or_else(|| io::Error::other("PID record is incomplete"))?,
    );
    let start_token = lines.next().unwrap_or_default().to_string();
    Ok(PidRecord {
        pid,
        executable,
        start_token,
    })
}

fn process_matches(record: &PidRecord) -> bool {
    let current = match fs::canonicalize(format!("/proc/{}/exe", record.pid)) {
        Ok(path) => path,
        Err(_) => return false,
    };
    let expected = match fs::canonicalize(&record.executable) {
        Ok(path) => path,
        Err(_) => return false,
    };
    !record.start_token.is_empty()
        && current == expected
        && process_start_token(record.pid).as_deref() == Some(record.start_token.as_str())
}

fn process_start_token(pid: u32) -> Option<String> {
    let text = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, rest) = text.rsplit_once(") ")?;
    rest.split_whitespace().nth(19).map(str::to_string)
}

fn shell_quote(path: &Path) -> String {
    let value = path.display().to_string();
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn systemd_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn is_privileged() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(windows)]
    {
        false
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

fn find_program(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|path| path.is_file())
}

#[allow(dead_code)]
fn capture_command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn command_ok(program: &str, args: &[&str]) -> io::Result<()> {
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{program} {:?} exited with {status}",
            args
        )))
    }
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("atomic target has no parent directory"))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(contents)?;
    staged.as_file().sync_all()?;
    staged
        .persist(path)
        .map(|_| ())
        .map_err(|error| io::Error::other(error.error.to_string()))
}

fn launchd_plist_path() -> io::Result<PathBuf> {
    dirs::home_dir()
        .map(|home| {
            home.join("Library/LaunchAgents")
                .join(format!("{SERVICE_LABEL}.plist"))
        })
        .ok_or_else(|| io::Error::other("home directory is unavailable"))
}

fn launchd_log_path() -> io::Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join("Library/Logs/eggsearch.log"))
        .ok_or_else(|| io::Error::other("home directory is unavailable"))
}

fn launchd_domain() -> io::Result<String> {
    #[cfg(unix)]
    {
        Ok(format!("gui/{}", unsafe { libc::getuid() }))
    }
    #[cfg(not(unix))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "launchd is only available on macOS",
        ))
    }
}

fn cron_state_dir() -> io::Result<PathBuf> {
    dirs::data_local_dir()
        .map(|dir| dir.join("eggsearch"))
        .or_else(|| {
            std::env::var_os("XDG_STATE_HOME").map(|dir| PathBuf::from(dir).join("eggsearch"))
        })
        .ok_or_else(|| io::Error::other("local state directory is unavailable"))
}

fn cron_pid_path() -> io::Result<PathBuf> {
    Ok(cron_state_dir()?.join("mcp-serve.pid"))
}
fn cron_lock_path() -> io::Result<PathBuf> {
    Ok(cron_state_dir()?.join("croncheck.lock"))
}
fn cron_log_path() -> io::Result<PathBuf> {
    Ok(cron_state_dir()?.join("mcp-serve.log"))
}

fn windows_bin_path(spec: &RuntimeSpec) -> String {
    format!(
        "\"{}\" --config \"{}\" windows-service",
        spec.executable.display(),
        spec.config.display()
    )
}
fn windows_command(spec: &RuntimeSpec, action: &str) -> String {
    match action {
        "install" => format!(
            "sc.exe create {SERVICE_NAME} start= auto binPath= \"{}\"",
            windows_bin_path(spec).replace('"', "\\\"")
        ),
        "uninstall" => format!("sc.exe stop {SERVICE_NAME}\nsc.exe delete {SERVICE_NAME}"),
        _ => format!("sc.exe {action} {SERVICE_NAME}"),
    }
}

fn windows_permission_error(error: io::Error, command: &str) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("{error}; rerun from an elevated PowerShell prompt:\n  {command}"),
    )
}

#[derive(Deserialize)]
struct HealthPayload {
    service: String,
    status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_selection_obeys_platform_policy() {
        let linux = PlatformInfo {
            os: HostOs::Linux,
            systemd_active: true,
            launchd_available: false,
            windows_scm_available: false,
        };
        assert_eq!(
            select_method(StartupMethod::Auto, linux).unwrap(),
            StartupMethod::Systemd
        );
        let fallback = PlatformInfo {
            systemd_active: false,
            ..linux
        };
        assert_eq!(
            select_method(StartupMethod::Auto, fallback).unwrap(),
            StartupMethod::Cron
        );
        let mac = PlatformInfo {
            os: HostOs::Macos,
            systemd_active: false,
            launchd_available: true,
            windows_scm_available: false,
        };
        assert_eq!(
            select_method(StartupMethod::Auto, mac).unwrap(),
            StartupMethod::Launchd
        );
    }

    #[test]
    fn explicit_methods_are_platform_gated() {
        let linux = PlatformInfo {
            os: HostOs::Linux,
            systemd_active: true,
            launchd_available: false,
            windows_scm_available: false,
        };
        assert!(select_method(StartupMethod::Launchd, linux).is_err());
        assert_eq!(
            select_method(StartupMethod::Systemd, linux).unwrap(),
            StartupMethod::Systemd
        );
    }

    #[test]
    fn cron_marker_update_preserves_unrelated_lines() {
        let before = "MAILTO=\"\"\n0 2 * * * backup\n* * * * * old # eggsearch-managed\n";
        let after = update_crontab(before, Some("* * * * * 'a' # eggsearch-managed")).unwrap();
        assert_eq!(
            after,
            "MAILTO=\"\"\n0 2 * * * backup\n* * * * * 'a' # eggsearch-managed\n"
        );
        assert_eq!(
            update_crontab(&after, None).unwrap(),
            "MAILTO=\"\"\n0 2 * * * backup\n"
        );
    }

    #[test]
    fn cron_rejects_newline_injection() {
        assert!(update_crontab("", Some("bad\n* * * * * evil")).is_err());
    }

    #[test]
    fn renderers_escape_paths() {
        let spec = RuntimeSpec::new(
            PathBuf::from("/tmp/a b/egg'search"),
            PathBuf::from("/tmp/c&d/config.toml"),
            None,
        );
        let launchd = render_launchd(&spec);
        assert!(launchd.contains("a b/egg&apos;search"));
        assert!(render_systemd(&spec).contains("/tmp/a b/egg'search"));
        assert!(cron_line(&spec).contains("'/tmp/a b/egg'\\''search'"));
    }

    #[test]
    fn runtime_command_is_canonical() {
        let spec = RuntimeSpec::new(
            PathBuf::from("/opt/eggsearch"),
            PathBuf::from("/etc/eggsearch/config.toml"),
            Some(PathBuf::from("/run/egg.pid")),
        );
        assert_eq!(
            spec.args(),
            vec![
                "--config",
                "/etc/eggsearch/config.toml",
                "mcp",
                "serve",
                "--bind",
                "127.0.0.1:11320",
                "--path",
                "/mcp",
                "--pid-file",
                "/run/egg.pid"
            ]
        );
    }
}

#[cfg(windows)]
windows_service::define_windows_service!(ffi_service_main, windows_service_main);

#[cfg(windows)]
/// Start the Windows Service Control Manager entry point.
pub fn run_windows_service(config: Option<&Path>) -> io::Result<()> {
    let _ = config;
    windows_service::service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .map_err(|error| io::Error::other(error.to_string()))
}

#[cfg(windows)]
fn windows_service_main(arguments: Vec<std::ffi::OsString>) {
    if let Err(error) = run_windows_service_instance(arguments) {
        eprintln!("eggsearch Windows service failed: {error}");
    }
}

#[cfg(windows)]
fn run_windows_service_instance(arguments: Vec<std::ffi::OsString>) -> io::Result<()> {
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    let config = arguments
        .windows(2)
        .find(|pair| pair[0] == "--config")
        .map(|pair| PathBuf::from(&pair[1]));
    let cancellation = CancellationToken::new();
    let handler_token = cancellation.clone();
    let event_handler = move |event| -> ServiceControlHandlerResult {
        match event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                handler_token.cancel();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .map_err(|error| io::Error::other(error.to_string()))?;
    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .map_err(|error| io::Error::other(error.to_string()))?;
    let default_config = default_config_path();
    let config_path = config.as_deref().unwrap_or(&default_config);
    let cfg = crate::core::config::AppConfig::load(config_path)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime
        .block_on(crate::mcp::http::run_with_cancellation(
            &cfg,
            crate::mcp::http::ServeOptions::default(),
            cancellation,
        ))
        .map_err(|error| io::Error::other(error.to_string()))?;
    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(())
}
