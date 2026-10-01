#![deny(unsafe_code)]

use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

const TRIGGER_TIMEOUT: u8 = 0;
const TRIGGER_STDOUT_LIMIT: u8 = 1;
const TRIGGER_STDERR_LIMIT: u8 = 2;

pub(crate) fn run_bounded_command(
    command: &mut Command,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> BoundedCommandResult {
    run_bounded_command_impl(command, timeout, stdout_limit, stderr_limit)
}

struct ProcessTerminationController {
    child_pgid: i32,
    trigger: AtomicU8,
    kill_sent: AtomicBool,
}

impl ProcessTerminationController {
    fn new(child_pgid: i32) -> Self {
        Self {
            child_pgid,
            trigger: AtomicU8::new(u8::MAX),
            kill_sent: AtomicBool::new(false),
        }
    }

    fn try_terminate(&self, trigger: u8) -> bool {
        if self
            .kill_sent
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            self.trigger.store(trigger, Ordering::Relaxed);
            if self.child_pgid > 1 {
                #[cfg(windows)]
                {
                    let _ = terminate_windows_process_tree(self.child_pgid);
                }
                #[cfg(unix)]
                let _ = crate::process::terminate_process_group(self.child_pgid);
            }
            true
        } else {
            false
        }
    }

    fn termination_reason(&self) -> CommandTermination {
        match self.trigger.load(Ordering::Relaxed) {
            TRIGGER_TIMEOUT => CommandTermination::TimedOut,
            TRIGGER_STDOUT_LIMIT => CommandTermination::StdoutLimitExceeded,
            TRIGGER_STDERR_LIMIT => CommandTermination::StderrLimitExceeded,
            _ if self.kill_sent.load(Ordering::Relaxed) => CommandTermination::Signaled,
            _ => CommandTermination::Exited,
        }
    }
}

#[cfg(windows)]
fn terminate_windows_process_tree(pid: i32) -> std::io::Result<()> {
    let mut child = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .spawn()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(std::io::Error::other("taskkill failed"))
            };
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "taskkill timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// How the bounded command was terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTermination {
    /// Process exited normally.
    Exited,
    /// Process was killed due to timeout.
    TimedOut,
    /// Stdout cap breach triggered termination.
    StdoutLimitExceeded,
    /// Stderr cap breach triggered termination.
    StderrLimitExceeded,
    /// Process could not be spawned.
    SpawnFailed,
    /// Process was killed by a signal.
    Signaled,
    #[allow(missing_docs)]
    ReaderPanicked,
}

/// Result of a bounded command execution.
pub struct BoundedCommandResult {
    /// The exit status of the command, if it was spawned.
    pub status: Option<std::process::ExitStatus>,
    /// Captured stdout bytes.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes.
    pub stderr: Vec<u8>,
    /// Whether the command was killed due to timeout.
    pub timed_out: bool,
    /// Whether stdout was truncated at the cap.
    pub stdout_truncated: bool,
    /// Whether stderr was truncated at the cap.
    pub stderr_truncated: bool,
    /// How the command was terminated.
    pub termination: CommandTermination,
}

fn run_bounded_command_impl(
    cmd: &mut Command,
    timeout: Duration,
    stdout_cap: usize,
    stderr_cap: usize,
) -> BoundedCommandResult {
    use std::io::Read;

    if cmd.get_program() == "git" {
        cmd.env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "safe.directory")
            .env("GIT_CONFIG_VALUE_0", "*");
    }

    crate::process::configure_new_session(cmd);

    let mut child = match cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => {
            return BoundedCommandResult {
                status: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                timed_out: false,
                stdout_truncated: false,
                stderr_truncated: false,
                termination: CommandTermination::SpawnFailed,
            };
        }
    };

    let child_id = child.id() as i32;
    let controller = Arc::new(ProcessTerminationController::new(child_id));
    let exited = Arc::new(AtomicBool::new(false));

    let controller_timeout = controller.clone();
    let exited_timeout = exited.clone();
    let kill_handle = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if exited_timeout.load(Ordering::Relaxed) {
                return;
            }
            if std::time::Instant::now() >= deadline {
                controller_timeout.try_terminate(TRIGGER_TIMEOUT);
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let controller_stdout = controller.clone();
    let stdout_thread = std::thread::spawn(move || {
        let mut local_stdout = Vec::new();
        let mut local_truncated = false;
        if let Some(mut out) = stdout_handle {
            let mut buf = [0u8; 8192];
            loop {
                match out.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let remaining = stdout_cap.saturating_sub(local_stdout.len());
                        if n <= remaining {
                            local_stdout.extend_from_slice(&buf[..n]);
                        } else {
                            local_stdout.extend_from_slice(&buf[..remaining]);
                            local_truncated = true;
                            controller_stdout.try_terminate(TRIGGER_STDOUT_LIMIT);
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        (local_stdout, local_truncated)
    });

    let controller_stderr = controller.clone();
    let stderr_thread = std::thread::spawn(move || {
        let mut local_stderr = Vec::new();
        let mut local_truncated = false;
        if let Some(mut err) = stderr_handle {
            let mut buf = [0u8; 8192];
            loop {
                match err.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let remaining = stderr_cap.saturating_sub(local_stderr.len());
                        if n <= remaining {
                            local_stderr.extend_from_slice(&buf[..n]);
                        } else {
                            local_stderr.extend_from_slice(&buf[..remaining]);
                            local_truncated = true;
                            controller_stderr.try_terminate(TRIGGER_STDERR_LIMIT);
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        (local_stderr, local_truncated)
    });

    let stdout_result = stdout_thread.join();
    let stderr_result = stderr_thread.join();
    let stdout_panicked = stdout_result.is_err();
    let stderr_panicked = stderr_result.is_err();
    let (stdout, stdout_truncated) = stdout_result.unwrap_or((Vec::new(), false));
    let (stderr, stderr_truncated) = stderr_result.unwrap_or((Vec::new(), false));

    let status = child.wait().ok();
    exited.store(true, Ordering::Relaxed);
    let _ = kill_handle.join();

    let timed_out = controller.trigger.load(Ordering::Relaxed) == TRIGGER_TIMEOUT;
    let termination = if stdout_panicked || stderr_panicked {
        CommandTermination::ReaderPanicked
    } else {
        controller.termination_reason()
    };

    BoundedCommandResult {
        status,
        stdout,
        stderr,
        timed_out,
        stdout_truncated,
        stderr_truncated,
        termination,
    }
}

#[allow(unsafe_code)]
pub(crate) fn configure_new_session(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid is async-signal-safe; the closure performs no allocation and reports errno immediately.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(not(unix))]
    let _ = command;
}

#[allow(unsafe_code)]
pub(crate) fn terminate_process_group(pgid: i32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        if unsafe { libc::kill(-pgid, libc::SIGKILL) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pgid;
        Ok(())
    }
}

#[allow(unsafe_code)]
pub(crate) fn terminate_process(pid: i32, signal: i32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        if unsafe { libc::kill(pid, signal) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, signal);
        Ok(())
    }
}

#[allow(unsafe_code)]
pub(crate) fn is_privileged_user() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[allow(unsafe_code)]
pub(crate) fn real_user_id() -> Option<u32> {
    #[cfg(unix)]
    {
        Some(unsafe { libc::getuid() })
    }
    #[cfg(not(unix))]
    {
        None
    }
}
