use std::{
    ffi::OsStr,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::Duration,
};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use tempfile::NamedTempFile;

use super::{claude, codegg, codex, cursor, opencode, vscode, zed};

pub const HTTP_ENDPOINT: &str = "http://127.0.0.1:11320/mcp";
const REQUIRED_TOOLS: [&str; 2] = ["web_search", "web_fetch"];
const RECOMMENDED_TOOLS: [&str; 8] = [
    "batch_fetch",
    "repo_search",
    "repo_fetch",
    "repo_map",
    "security_search",
    "research_search",
    "build_evidence_bundle",
    "provider_status",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Stdio,
    Http,
}

impl std::fmt::Display for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Stdio => "stdio",
            Self::Http => "http",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Client {
    Codegg,
    Zed,
    Codex,
    Claude,
    Cursor,
    Vscode,
    Opencode,
}

impl Client {
    pub const ALL: [Self; 7] = [
        Self::Codegg,
        Self::Zed,
        Self::Codex,
        Self::Claude,
        Self::Cursor,
        Self::Vscode,
        Self::Opencode,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Codegg => "codegg",
            Self::Zed => "zed",
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
            Self::Vscode => "vscode",
            Self::Opencode => "opencode",
        }
    }
}

impl std::fmt::Display for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct IntegrationSummary {
    pub client: Client,
    pub available: bool,
    pub stdio: bool,
    pub http: bool,
    pub apply_mode: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct IntegrationReport {
    pub client: Client,
    pub transport: Transport,
    pub available: bool,
    pub applied: bool,
    pub verified: bool,
    pub apply_mode: &'static str,
    pub config_path: Option<String>,
    pub command: Option<Vec<String>>,
    pub configuration: Option<Value>,
    pub message: String,
}

struct Rendered {
    available: bool,
    apply_mode: &'static str,
    config_path: Option<PathBuf>,
    command: Option<Vec<String>>,
    configuration: Option<Value>,
    executable: String,
    ephemeral_executable: bool,
}

pub async fn run(
    client: Client,
    transport: Transport,
    apply: bool,
    json_output: bool,
    executable: Option<PathBuf>,
) -> Result<()> {
    let rendered = render_internal(client, transport, executable.as_deref())?;
    if apply && rendered.apply_mode == "print-only" {
        bail!(
            "{} apply is print-only because its settings format cannot be edited safely; review the rendered configuration and add it in the client",
            client
        );
    }
    if apply
        && transport == Transport::Stdio
        && rendered.ephemeral_executable
        && executable.is_none()
    {
        bail!(
            "the current eggsearch executable is a development/test path; install eggsearch or pass --executable /path/to/eggsearch before using --apply"
        );
    }

    let mut report = IntegrationReport {
        client,
        transport,
        available: rendered.available,
        applied: false,
        verified: false,
        apply_mode: rendered.apply_mode,
        config_path: rendered
            .config_path
            .as_ref()
            .map(|path| path.display().to_string()),
        command: rendered.command.clone(),
        configuration: rendered.configuration.clone(),
        message: if apply {
            "not applied".to_string()
        } else {
            "rendered only; pass --apply to register and verify".to_string()
        },
    };

    if apply {
        if !rendered.available {
            bail!("{} is not available on PATH", client);
        }
        apply_rendered(client, transport, &rendered).await?;
        verify(transport, &rendered.executable).await?;
        report.applied = true;
        report.verified = true;
        report.message = "registered and verified minimum MCP tool set".to_string();
    }

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("client: {}", report.client);
        println!("transport: {}", report.transport);
        println!("available: {}", report.available);
        println!("apply mode: {}", report.apply_mode);
        if let Some(path) = &report.config_path {
            println!("config path: {path}");
        }
        if let Some(command) = &report.command {
            println!("command: {}", shell_join(command));
        }
        if let Some(configuration) = &report.configuration {
            println!("configuration:");
            println!("{}", serde_json::to_string_pretty(configuration)?);
        }
        println!("status: {}", report.message);
    }
    Ok(())
}

pub fn summaries() -> Vec<IntegrationSummary> {
    Client::ALL
        .into_iter()
        .map(|client| IntegrationSummary {
            client,
            available: client_available(client),
            stdio: true,
            http: true,
            apply_mode: apply_mode(client),
        })
        .collect()
}

pub fn render(
    client: Client,
    transport: Transport,
    override_executable: Option<&Path>,
) -> Result<RenderedForTest> {
    let rendered = render_internal(client, transport, override_executable)?;
    Ok(RenderedForTest {
        command: rendered.command,
        configuration: rendered.configuration,
        config_path: rendered.config_path,
        apply_mode: rendered.apply_mode,
        available: rendered.available,
    })
}

#[derive(Clone, Debug)]
pub struct RenderedForTest {
    pub command: Option<Vec<String>>,
    pub configuration: Option<Value>,
    pub config_path: Option<PathBuf>,
    pub apply_mode: &'static str,
    pub available: bool,
}

fn render_internal(
    client: Client,
    transport: Transport,
    override_executable: Option<&Path>,
) -> Result<Rendered> {
    let (executable, ephemeral_executable) = resolve_executable(override_executable)?;
    let command = if matches!(client, Client::Codex | Client::Claude | Client::Vscode) {
        Some(match client {
            Client::Codex => codex::command(transport, &executable, HTTP_ENDPOINT),
            Client::Claude => claude::command(transport, &executable, HTTP_ENDPOINT),
            Client::Vscode => vscode::command(transport, &executable, HTTP_ENDPOINT),
            _ => unreachable!(),
        })
    } else {
        None
    };
    let configuration = match client {
        Client::Codegg => Some(codegg::entry(transport, &executable, HTTP_ENDPOINT)),
        Client::Zed => Some(zed::entry(transport, &executable, HTTP_ENDPOINT)),
        Client::Cursor => Some(cursor::entry(transport, &executable, HTTP_ENDPOINT)),
        Client::Opencode => Some(opencode::entry(transport, &executable, HTTP_ENDPOINT)),
        Client::Codex | Client::Claude | Client::Vscode => None,
    };
    Ok(Rendered {
        available: client_available(client),
        apply_mode: apply_mode(client),
        config_path: config_path(client),
        command,
        configuration,
        executable,
        ephemeral_executable,
    })
}

async fn apply_rendered(client: Client, transport: Transport, rendered: &Rendered) -> Result<()> {
    match client {
        Client::Codex => {
            apply_native(
                &codex::command(transport, &rendered.executable, HTTP_ENDPOINT),
                &codex::remove_command(),
            )
            .await
        }
        Client::Claude => {
            apply_native(
                &claude::command(transport, &rendered.executable, HTTP_ENDPOINT),
                &claude::remove_command(),
            )
            .await
        }
        Client::Vscode => run_command(
            &vscode::command(transport, &rendered.executable, HTTP_ENDPOINT),
            false,
        ),
        Client::Codegg => apply_codegg(rendered),
        Client::Cursor => apply_cursor(rendered, transport),
        Client::Zed => bail!("zed apply is print-only"),
        Client::Opencode => apply_opencode(rendered, transport),
    }
}

async fn apply_native(add: &[String], remove: &[String]) -> Result<()> {
    let command = &add[0];
    let exists = Command::new(command)
        .args(["mcp", "get", "eggsearch"])
        .output()
        .with_context(|| format!("failed to run {command} mcp get"))?
        .status
        .success();
    if exists {
        run_command(remove, false)?;
    }
    run_command(add, false)
}

fn apply_codegg(rendered: &Rendered) -> Result<()> {
    let path = rendered
        .config_path
        .as_deref()
        .context("CodeGG config path is unavailable")?;
    update_json_file(path, |root| {
        let object = root
            .as_object_mut()
            .context("CodeGG configuration root must be a JSON object")?;
        let search = object.entry("search").or_insert_with(|| json!({}));
        let search = search
            .as_object_mut()
            .context("CodeGG search configuration must be a JSON object")?;
        search.insert("backend".to_string(), json!("eggsearch"));
        if rendered
            .configuration
            .as_ref()
            .and_then(|v| v.get("mcp"))
            .and_then(|v| v.get("eggsearch"))
            .is_some()
        {
            let mcp = object.entry("mcp").or_insert_with(|| json!({}));
            let mcp = mcp
                .as_object_mut()
                .context("CodeGG MCP configuration must be a JSON object")?;
            let entry = rendered
                .configuration
                .as_ref()
                .and_then(|v| v.get("mcp"))
                .and_then(|v| v.get("eggsearch"))
                .cloned()
                .context("CodeGG remote entry missing")?;
            mcp.insert("eggsearch".to_string(), entry);
        }
        Ok(())
    })
}

fn apply_cursor(rendered: &Rendered, transport: Transport) -> Result<()> {
    let path = rendered
        .config_path
        .as_deref()
        .context("Cursor config path is unavailable")?;
    let desired = rendered
        .configuration
        .clone()
        .context("Cursor entry missing")?;
    update_json_file(path, |root| {
        let root = root
            .as_object_mut()
            .context("Cursor configuration root must be a JSON object")?;
        let servers = root.entry("mcpServers").or_insert_with(|| json!({}));
        let servers = servers
            .as_object_mut()
            .context("Cursor mcpServers must be a JSON object")?;
        servers.insert("eggsearch".to_string(), desired.clone());
        if transport == Transport::Stdio {
            ensure_string_command(&desired)?;
        }
        Ok(())
    })
}

fn apply_opencode(rendered: &Rendered, transport: Transport) -> Result<()> {
    let path = rendered
        .config_path
        .as_deref()
        .context("OpenCode config path is unavailable")?;
    if path.extension() == Some(OsStr::new("jsonc")) {
        bail!("OpenCode JSONC configuration is print-only; use opencode mcp add or edit it in the client");
    }
    let desired = rendered
        .configuration
        .clone()
        .context("OpenCode entry missing")?;
    update_json_file(path, |root| {
        let root = root
            .as_object_mut()
            .context("OpenCode configuration root must be a JSON object")?;
        let mcp = root.entry("mcp").or_insert_with(|| json!({}));
        let mcp = mcp
            .as_object_mut()
            .context("OpenCode mcp must be a JSON object")?;
        let servers = mcp.entry("servers").or_insert_with(|| json!({}));
        let servers = servers
            .as_object_mut()
            .context("OpenCode mcp.servers must be a JSON object")?;
        servers.insert("eggsearch".to_string(), desired.clone());
        if transport == Transport::Stdio {
            ensure_string_command_array(&desired)?;
        }
        Ok(())
    })
}

fn update_json_file<F>(path: &Path, update: F) -> Result<()>
where
    F: FnOnce(&mut Value) -> Result<()>,
{
    let existed = path.exists();
    let original = if existed {
        fs::read(path).with_context(|| format!("failed to read {}", path.display()))?
    } else {
        Vec::new()
    };
    let mut root = if existed {
        serde_json::from_slice(&original)
            .with_context(|| format!("{} is not valid JSON; no changes made", path.display()))?
    } else {
        json!({})
    };
    if !root.is_object() {
        bail!(
            "{} must contain a JSON object at the root; no changes made",
            path.display()
        );
    }
    let before = root.clone();
    update(&mut root)?;
    if root == before {
        return Ok(());
    }
    let next = serde_json::to_vec_pretty(&root)?;
    if existed {
        let backup = backup_path(path);
        fs::copy(path, &backup)
            .with_context(|| format!("failed to create backup {}", backup.display()))?;
        eprintln!("backup: {}", backup.display());
    } else if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut staged = NamedTempFile::new_in(parent)?;
    staged.write_all(&next)?;
    staged.write_all(b"\n")?;
    staged.as_file().sync_all()?;
    if existed {
        let permissions = fs::metadata(path)?.permissions();
        staged.as_file().set_permissions(permissions)?;
    }
    staged.persist(path).map_err(|e| anyhow::anyhow!(e.error))?;
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    PathBuf::from(format!("{}.bak.{stamp}", path.display()))
}

fn ensure_string_command(value: &Value) -> Result<()> {
    if value.get("command").and_then(Value::as_str).is_none() {
        bail!("stdio configuration command must be a string");
    }
    Ok(())
}

fn ensure_string_command_array(value: &Value) -> Result<()> {
    if !value.get("command").is_some_and(Value::is_array) {
        bail!("OpenCode local command must be an argument array");
    }
    Ok(())
}

fn client_available(client: Client) -> bool {
    match client {
        Client::Codegg => command_available("codegg"),
        Client::Zed => command_available("zed"),
        Client::Codex => command_available("codex"),
        Client::Claude => command_available("claude"),
        Client::Cursor => command_available("cursor") || command_available("cursor-agent"),
        Client::Vscode => command_available("code") || command_available("code-insiders"),
        Client::Opencode => command_available("opencode"),
    }
}

fn command_available(command: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| {
            let candidate = dir.join(command);
            candidate.is_file() && is_executable(&candidate)
        })
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn apply_mode(client: Client) -> &'static str {
    match client {
        Client::Codegg | Client::Cursor => "json-atomic-backup",
        Client::Codex | Client::Claude | Client::Vscode => "native-cli",
        Client::Opencode => "json-atomic-backup-if-json",
        Client::Zed => "print-only",
    }
}

fn config_path(client: Client) -> Option<PathBuf> {
    let config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    match client {
        Client::Codegg => Some(config.join("codegg").join("config.json")),
        Client::Zed => Some(config.join("Zed").join("settings.json")),
        Client::Cursor => dirs::home_dir().map(|home| home.join(".cursor").join("mcp.json")),
        Client::Opencode => {
            let jsonc = config.join("opencode").join("opencode.jsonc");
            let json_path = config.join("opencode").join("opencode.json");
            Some(if jsonc.exists() { jsonc } else { json_path })
        }
        Client::Codex | Client::Claude | Client::Vscode => None,
    }
}

fn resolve_executable(override_executable: Option<&Path>) -> Result<(String, bool)> {
    if let Some(path) = override_executable {
        if path.as_os_str().is_empty() {
            bail!("--executable must not be empty");
        }
        return Ok((path.display().to_string(), false));
    }
    let path = std::env::current_exe().context("failed to resolve the current executable")?;
    let ephemeral = path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("target" | "debug" | "deps")
        )
    });
    if ephemeral {
        Ok(("eggsearch".to_string(), true))
    } else {
        Ok((path.display().to_string(), false))
    }
}

fn run_command(argv: &[String], inherit_stderr: bool) -> Result<()> {
    let (program, args) = argv.split_first().context("empty command")?;
    let mut command = Command::new(program);
    command.args(args);
    if inherit_stderr {
        let status = command.status().context("failed to execute command")?;
        if !status.success() {
            bail!("command exited with {status}");
        }
        return Ok(());
    }
    {
        let output = command.output().context("failed to execute command")?;
        if !output.status.success() {
            bail!("{}", command_failure(&output));
        }
        Ok(())
    }
}

fn command_failure(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("command exited with {}", output.status)
    } else {
        stderr
    }
}

fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| {
            if arg
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._/:".contains(&b))
            {
                arg.clone()
            } else {
                format!("'{}'", arg.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

async fn verify(transport: Transport, executable: &str) -> Result<()> {
    match transport {
        Transport::Stdio => verify_stdio(executable).await,
        Transport::Http => verify_http().await,
    }
}

async fn verify_stdio(executable: &str) -> Result<()> {
    use rmcp::{transport::TokioChildProcess, ServiceExt};

    let transport = TokioChildProcess::new({
        let mut command = tokio::process::Command::new(executable);
        command.args(["mcp", "stdio"]);
        command
    })?;
    let client = ().serve(transport).await.context("MCP stdio initialize failed")?;
    let tools = client
        .list_all_tools()
        .await
        .context("MCP stdio tools/list failed")?;
    check_tools(
        &tools
            .iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>(),
    )?;
    client.cancel().await?;
    Ok(())
}

async fn verify_http() -> Result<()> {
    use rmcp::{transport::StreamableHttpClientTransport, ServiceExt};

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let health = http
        .get("http://127.0.0.1:11320/healthz")
        .send()
        .await
        .context("HTTP health check failed")?;
    if !health.status().is_success() {
        bail!("HTTP health check returned {}", health.status());
    }
    let payload: Value = health.json().await.context("invalid /healthz JSON")?;
    if payload.get("service").and_then(Value::as_str) != Some("eggsearch")
        || payload.get("status").and_then(Value::as_str) != Some("ready")
    {
        bail!("/healthz did not identify a ready eggsearch service");
    }
    let transport = StreamableHttpClientTransport::from_uri(HTTP_ENDPOINT);
    let client = ().serve(transport).await.context("MCP HTTP initialize failed")?;
    let tools = client
        .list_all_tools()
        .await
        .context("MCP HTTP tools/list failed")?;
    check_tools(
        &tools
            .iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>(),
    )?;
    client.cancel().await?;
    Ok(())
}

fn check_tools(tools: &[String]) -> Result<()> {
    let missing: Vec<_> = REQUIRED_TOOLS
        .iter()
        .filter(|name| !tools.iter().any(|tool| tool == **name))
        .copied()
        .collect();
    if !missing.is_empty() {
        bail!(
            "MCP server is missing required tools: {}",
            missing.join(", ")
        );
    }
    let missing_recommended: Vec<_> = RECOMMENDED_TOOLS
        .iter()
        .filter(|name| !tools.iter().any(|tool| tool == **name))
        .copied()
        .collect();
    if !missing_recommended.is_empty() {
        eprintln!(
            "warning: recommended tools unavailable: {}",
            missing_recommended.join(", ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn native_commands_use_argv_boundaries() {
        let rendered = render(
            Client::Codex,
            Transport::Stdio,
            Some(Path::new("/opt/eggsearch")),
        )
        .unwrap();
        assert_eq!(
            rendered.command.unwrap(),
            vec![
                "codex",
                "mcp",
                "add",
                "eggsearch",
                "--",
                "/opt/eggsearch",
                "mcp",
                "stdio"
            ]
        );
    }

    #[test]
    fn codegg_stdio_is_minimal_and_http_is_explicit_remote() {
        let stdio = render(
            Client::Codegg,
            Transport::Stdio,
            Some(Path::new("eggsearch")),
        )
        .unwrap();
        assert_eq!(
            stdio.configuration.unwrap()["search"]["backend"],
            "eggsearch"
        );
        let http = render(
            Client::Codegg,
            Transport::Http,
            Some(Path::new("eggsearch")),
        )
        .unwrap();
        assert_eq!(
            http.configuration.unwrap()["mcp"]["eggsearch"]["type"],
            "remote"
        );
    }

    #[test]
    fn malformed_json_is_not_mutated() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(&path, b"{broken").unwrap();
        let before = fs::read(&path).unwrap();
        let result = update_json_file(&path, |_| Ok(()));
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn json_update_preserves_unrelated_servers_and_creates_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(&path, r#"{"mcpServers":{"other":{"command":"keep"}}}"#).unwrap();
        update_json_file(&path, |root| {
            root["mcpServers"]["eggsearch"] = json!({"command":"eggsearch"});
            Ok(())
        })
        .unwrap();
        let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["mcpServers"]["other"]["command"], "keep");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn json_update_is_idempotent_after_formatting() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(
            &path,
            r#"{
  "mcpServers": {
    "eggsearch": {"command": "eggsearch"}
  }
}
"#,
        )
        .unwrap();
        let before = fs::read(&path).unwrap();
        update_json_file(&path, |root| {
            root["mcpServers"]["eggsearch"] = json!({"command":"eggsearch"});
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn every_client_renders_both_transports() {
        for client in Client::ALL {
            for transport in [Transport::Stdio, Transport::Http] {
                let rendered =
                    render(client, transport, Some(Path::new("/opt/eggsearch"))).unwrap();
                assert!(rendered.command.is_some() || rendered.configuration.is_some());
            }
        }
    }
}
