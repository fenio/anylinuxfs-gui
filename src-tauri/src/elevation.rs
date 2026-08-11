use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Managed approval and an encryption prompt can both require user interaction.
pub const INTERACTIVE_ELEVATION_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElevationMode {
    Native,
    InteractiveTerminal,
}

#[derive(Debug, Clone, Serialize)]
pub struct ElevationPolicy {
    pub mode: ElevationMode,
}

#[derive(Debug, Clone)]
pub enum TerminalInteraction {
    CaptureOutput { operation: String },
    SecretPrompt { operation: String },
}

impl TerminalInteraction {
    fn operation(&self) -> &str {
        match self {
            Self::CaptureOutput { operation } | Self::SecretPrompt { operation } => operation,
        }
    }

    fn prompts_for_secret(&self) -> bool {
        matches!(self, Self::SecretPrompt { .. })
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum TerminalExecutionError {
    #[error("ALFS_SILENT_AUTH_EXPIRED")]
    InteractionRequired,
    #[error("Interactive Terminal operation was cancelled")]
    Cancelled,
    #[error("Interactive Terminal operation timed out")]
    TimedOut,
    #[error("Interactive Terminal command failed with status {status}. Review the Terminal tab for details.")]
    CommandFailed { status: i32, output: String },
    #[error("{0}")]
    Launch(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredPreferences {
    mode: ElevationMode,
}

#[derive(Debug, Clone)]
struct TerminalSession {
    operation: String,
    cancel_path: PathBuf,
    pid_path: PathBuf,
    launch_ack_path: PathBuf,
    status_path: PathBuf,
    cli_path: PathBuf,
    persistent_mount: bool,
    cancellation_started: bool,
}

pub struct ElevationState {
    config_path: PathBuf,
    mode: RwLock<ElevationMode>,
    sessions: Mutex<HashMap<u64, TerminalSession>>,
    active_operations: Mutex<HashSet<String>>,
    cancellation_requests: Mutex<HashSet<String>>,
    next_session_id: AtomicU64,
}

pub struct ElevationOperationGuard {
    state: Arc<ElevationState>,
    operation: String,
    mode: ElevationMode,
}

impl ElevationOperationGuard {
    pub fn mode(&self) -> ElevationMode {
        self.mode
    }
}

impl Drop for ElevationOperationGuard {
    fn drop(&mut self) {
        self.state.finish_operation(&self.operation);
    }
}

impl ElevationState {
    pub fn load(config_path: PathBuf) -> Self {
        let mode = read_stored_mode(&config_path).unwrap_or(ElevationMode::Native);

        Self {
            config_path,
            mode: RwLock::new(mode),
            sessions: Mutex::new(HashMap::new()),
            active_operations: Mutex::new(HashSet::new()),
            cancellation_requests: Mutex::new(HashSet::new()),
            next_session_id: AtomicU64::new(1),
        }
    }

    pub fn policy(&self) -> ElevationPolicy {
        ElevationPolicy { mode: self.mode() }
    }

    pub fn mode(&self) -> ElevationMode {
        *self
            .mode
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set_mode(&self, mode: ElevationMode) -> Result<ElevationPolicy, String> {
        // Keep the selected policy stable for the lifetime of every privileged
        // operation. Holding this lock through persistence closes the window in
        // which a new operation could snapshot the old mode while it is changing.
        let active_operations = self
            .active_operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !active_operations.is_empty() {
            return Err(
                "Elevation mode cannot be changed while a privileged operation is active"
                    .to_string(),
            );
        }

        write_stored_mode(&self.config_path, mode)?;
        *self
            .mode
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = mode;
        drop(active_operations);
        Ok(self.policy())
    }

    fn register_session(
        &self,
        operation: String,
        cancel_path: PathBuf,
        pid_path: PathBuf,
        launch_ack_path: PathBuf,
        status_path: PathBuf,
        cli_path: PathBuf,
    ) -> (u64, bool) {
        let id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        // Hold cancellation requests through insertion so a request either
        // marks this session now or finds it in `cancel_matching` afterward.
        let mut cancellation_requests = self
            .cancellation_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let cancellation_requested = cancellation_requests.remove(&operation);
        let session = TerminalSession {
            operation: operation.clone(),
            cancel_path,
            pid_path,
            launch_ack_path,
            status_path,
            cli_path,
            persistent_mount: false,
            cancellation_started: cancellation_requested,
        };
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, session.clone());
        drop(cancellation_requests);

        if cancellation_requested {
            // The command has not been launched yet, so only mark the handoff
            // cancelled. Stopping the device here could affect an older mount.
            cancel_terminal_session(&session, false);
        }
        (id, cancellation_requested)
    }

    fn unregister_session(&self, id: u64) {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&id);
    }

    pub fn mark_mount_persistent(&self, device: &str) -> bool {
        let operation = format!("mount:{}", device);
        let cancellation_requests = self
            .cancellation_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if cancellation_requests.contains(&operation) {
            return false;
        }
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((_, session)) = sessions
            .iter_mut()
            .filter(|(_, session)| session.operation == operation)
            .max_by_key(|(id, _)| **id)
        {
            if session.cancellation_started {
                return false;
            }
            session.persistent_mount = true;
            return true;
        }
        drop(sessions);
        drop(cancellation_requests);

        // Native elevation has no Terminal session to preserve. Interactive
        // callers must separately prove that a completed command succeeded.
        self.mode() != ElevationMode::InteractiveTerminal
    }

    pub fn begin_operation(
        self: &Arc<Self>,
        operation: impl Into<String>,
    ) -> Result<ElevationOperationGuard, String> {
        let operation = operation.into();
        let mut active_operations = self
            .active_operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active_operations.contains(&operation) {
            return Err(format!("Operation is already in progress: {}", operation));
        }
        let mode = self.mode();
        let mut cancellation_requests = self
            .cancellation_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cancellation_requests.remove(&operation);
        active_operations.insert(operation.clone());
        Ok(ElevationOperationGuard {
            state: self.clone(),
            operation,
            mode,
        })
    }

    fn finish_operation(&self, operation: &str) {
        let mut active_operations = self
            .active_operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut cancellation_requests = self
            .cancellation_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active_operations.remove(operation);
        cancellation_requests.remove(operation);
    }

    /// Request user-initiated cancellation. The request is retained while the
    /// operation is active so it survives the short pre-registration window.
    pub fn request_mount_cancellation(&self, device: &str) -> usize {
        let operation = format!("mount:{}", device);
        let active_operations = self
            .active_operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !active_operations.contains(&operation) {
            return 0;
        }

        self.cancellation_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(operation.clone());
        drop(active_operations);
        let cancelled = self.cancel_matching(Some(&operation), false);
        cancelled.max(1)
    }

    /// Cancel only sessions that already exist. Cleanup timeouts use this so
    /// they cannot cause an unrelated future retry to be cancelled.
    pub fn cancel_active_mount(&self, device: &str) -> usize {
        self.cancel_matching(Some(&format!("mount:{}", device)), false)
    }

    pub fn cancel_pending_operation(&self, operation: &str) -> usize {
        self.cancel_matching(Some(operation), false)
    }

    pub fn cancel_all_pending(&self) -> usize {
        self.cancel_matching(None, false)
    }

    /// Returns true when a confirmed mount must be preserved. The persistence
    /// check and session snapshot share one lock so timeout cleanup cannot race
    /// `mark_mount_persistent` after observing a stale value.
    fn cancel_session_for_timeout(&self, id: u64) -> bool {
        let mut sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(session) = sessions.get_mut(&id) else {
            return false;
        };
        if session.persistent_mount {
            return true;
        }
        if session.cancellation_started {
            return false;
        }
        session.cancellation_started = true;
        let session = session.clone();
        drop(sessions);

        cancel_terminal_session(&session, true);
        false
    }

    fn cancel_matching(&self, operation: Option<&str>, include_persistent: bool) -> usize {
        let sessions: Vec<TerminalSession> = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values_mut()
            .filter_map(|session| {
                if (include_persistent || !session.persistent_mount)
                    && !session.cancellation_started
                    && operation
                        .map(|value| value == session.operation.as_str())
                        .unwrap_or(true)
                {
                    session.cancellation_started = true;
                    Some(session.clone())
                } else {
                    None
                }
            })
            .collect();

        for session in &sessions {
            cancel_terminal_session(session, true);
        }
        sessions.len()
    }

    #[cfg(test)]
    fn active_session_count(&self) -> usize {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

fn read_stored_mode(path: &Path) -> Option<ElevationMode> {
    let contents = fs::read_to_string(path).ok()?;
    toml::from_str::<StoredPreferences>(&contents)
        .ok()
        .map(|preferences| preferences.mode)
}

fn write_stored_mode(path: &Path, mode: ElevationMode) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Elevation preference path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("Failed to create preference directory: {}", e))?;

    let contents = toml::to_string(&StoredPreferences { mode })
        .map_err(|e| format!("Failed to encode elevation preference: {}", e))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| format!("Failed to create temporary preference file: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Failed to secure preference file: {}", e))?;
    }

    temp.write_all(contents.as_bytes())
        .map_err(|e| format!("Failed to write elevation preference: {}", e))?;
    temp.as_file_mut()
        .sync_all()
        .map_err(|e| format!("Failed to flush elevation preference: {}", e))?;
    temp.persist(path)
        .map_err(|e| format!("Failed to save elevation preference: {}", e.error))?;
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(target_os = "macos")]
fn terminal_script_content(
    command_line: &str,
    output_path: &Path,
    status_path: &Path,
    cancel_path: &Path,
    pid_path: &Path,
    launch_ack_path: &Path,
    prompts_for_secret: bool,
) -> String {
    let prelude = format!(
        r#"#!/bin/zsh
/usr/bin/printf '%s\n' "$$" > {pid_temp_path}
/bin/mv -f {pid_temp_path} {pid_path}
start_gate_owned=0
acquire_start_gate() {{
  while ! /bin/mkdir {start_gate_path} 2>/dev/null; do /bin/sleep 0.01; done
  start_gate_owned=1
}}
release_start_gate() {{
  if [[ "$start_gate_owned" == "1" ]]; then
    /bin/rmdir {start_gate_path} 2>/dev/null
    start_gate_owned=0
  fi
}}
restore_terminal_echo() {{ /bin/stty echo 2>/dev/null }}
finish_terminal() {{
  command_status="$1"
  completion="finished"
  [[ -e {cancel_path} ]] && completion="cancelled"
  /usr/bin/printf '%s\n%s\n' "$command_status" "$completion" > {status_temp_path}
  /bin/mv -f {status_temp_path} {status_path}
  release_start_gate
  restore_terminal_echo
}}
trap 'finish_terminal 130; exit 130' HUP INT TERM
acquire_start_gate
if [[ -e {cancel_path} ]]; then
  finish_terminal 130
  exit 130
fi
/usr/bin/printf '%s\n' "command-starting" > {launch_ack_temp_path}
/bin/mv -f {launch_ack_temp_path} {launch_ack_path}
release_start_gate
"#,
        pid_path = shell_quote(&pid_path.to_string_lossy()),
        pid_temp_path = shell_quote(&format!("{}.tmp", pid_path.to_string_lossy())),
        launch_ack_path = shell_quote(&launch_ack_path.to_string_lossy()),
        launch_ack_temp_path = shell_quote(&format!("{}.tmp", launch_ack_path.to_string_lossy())),
        start_gate_path = shell_quote(&cancel_path.with_file_name("start-gate").to_string_lossy()),
        cancel_path = shell_quote(&cancel_path.to_string_lossy()),
        status_path = shell_quote(&status_path.to_string_lossy()),
        status_temp_path = shell_quote(&format!("{}.tmp", status_path.to_string_lossy())),
    );

    let command = if prompts_for_secret {
        format!(
            "/bin/stty -echo\n/usr/bin/script -q /dev/null {}\ncommand_status=$?\n",
            command_line
        )
    } else {
        format!(
            "{} 2>&1 | /usr/bin/tee {}\ncommand_status=${{pipestatus[1]}}\n",
            command_line,
            shell_quote(&output_path.to_string_lossy())
        )
    };

    format!(
        "{}{}finish_terminal \"$command_status\"\ntrap - HUP INT TERM\n/usr/bin/printf '\\nanylinuxfs finished with status %s. This Terminal tab can be closed.\\n' \"$command_status\"\nexit \"$command_status\"\n",
        prelude, command
    )
}

#[cfg(target_os = "macos")]
pub fn execute_in_terminal(
    state: &ElevationState,
    cli_path: &Path,
    args: &[&str],
    silent: bool,
    interaction: TerminalInteraction,
) -> Result<String, TerminalExecutionError> {
    use std::os::unix::fs::PermissionsExt;

    if silent {
        return Err(TerminalExecutionError::InteractionRequired);
    }

    let work_dir = tempfile::Builder::new()
        .prefix("anylinuxfs-terminal-")
        .tempdir()
        .map_err(|e| {
            TerminalExecutionError::Launch(format!(
                "Failed to create Terminal handoff directory: {}",
                e
            ))
        })?;
    fs::set_permissions(work_dir.path(), fs::Permissions::from_mode(0o700)).map_err(|e| {
        TerminalExecutionError::Launch(format!(
            "Failed to secure Terminal handoff directory: {}",
            e
        ))
    })?;

    let script_path = work_dir.path().join("run-anylinuxfs.command");
    let output_path = work_dir.path().join("output.txt");
    let status_path = work_dir.path().join("status.txt");
    let cancel_path = work_dir.path().join("cancel");
    let pid_path = work_dir.path().join("shell.pid");
    let launch_ack_path = work_dir.path().join("launched");

    let mut command_parts = Vec::with_capacity(args.len() + 2);
    command_parts.push("/usr/bin/sudo".to_string());
    command_parts.push(shell_quote(&cli_path.to_string_lossy()));
    command_parts.extend(args.iter().map(|arg| shell_quote(arg)));
    let command_line = command_parts.join(" ");

    let script_content = terminal_script_content(
        &command_line,
        &output_path,
        &status_path,
        &cancel_path,
        &pid_path,
        &launch_ack_path,
        interaction.prompts_for_secret(),
    );
    let mut script = fs::File::create(&script_path).map_err(|e| {
        TerminalExecutionError::Launch(format!("Failed to create Terminal command file: {}", e))
    })?;
    script.write_all(script_content.as_bytes()).map_err(|e| {
        TerminalExecutionError::Launch(format!("Failed to write Terminal command file: {}", e))
    })?;
    script
        .set_permissions(fs::Permissions::from_mode(0o700))
        .map_err(|e| {
            TerminalExecutionError::Launch(format!("Failed to secure Terminal command file: {}", e))
        })?;

    let (session_id, cancellation_requested) = state.register_session(
        interaction.operation().to_string(),
        cancel_path.clone(),
        pid_path,
        launch_ack_path,
        status_path.clone(),
        cli_path.to_path_buf(),
    );
    if cancellation_requested {
        state.unregister_session(session_id);
        return Err(TerminalExecutionError::Cancelled);
    }

    let launched = match Command::new("/usr/bin/open")
        .args(["-a", "Terminal"])
        .arg(&script_path)
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            state.unregister_session(session_id);
            return Err(TerminalExecutionError::Launch(format!(
                "Failed to open Terminal: {}",
                error
            )));
        }
    };
    if !launched.status.success() {
        state.unregister_session(session_id);
        return Err(TerminalExecutionError::Launch(format!(
            "Failed to open Terminal: {}",
            String::from_utf8_lossy(&launched.stderr).trim()
        )));
    }

    log::debug!("sudo: waiting for interactive elevation in Terminal");
    let result = wait_for_terminal_result(
        state,
        session_id,
        &status_path,
        &output_path,
        &cancel_path,
        Duration::from_secs(INTERACTIVE_ELEVATION_TIMEOUT_SECS),
    );
    if matches!(
        result,
        Err(TerminalExecutionError::Cancelled | TerminalExecutionError::TimedOut)
    ) {
        ensure_terminal_session_stopped(state, session_id);
    }
    state.unregister_session(session_id);
    result
}

#[cfg(not(target_os = "macos"))]
pub fn execute_in_terminal(
    _state: &ElevationState,
    _cli_path: &Path,
    _args: &[&str],
    _silent: bool,
    _interaction: TerminalInteraction,
) -> Result<String, TerminalExecutionError> {
    Err(TerminalExecutionError::Launch(
        "Interactive Terminal elevation is only available on macOS".to_string(),
    ))
}

fn wait_for_terminal_result(
    state: &ElevationState,
    session_id: u64,
    status_path: &Path,
    output_path: &Path,
    cancel_path: &Path,
    timeout: Duration,
) -> Result<String, TerminalExecutionError> {
    let start = Instant::now();
    loop {
        if let Ok(status_text) = fs::read_to_string(status_path) {
            let mut lines = status_text.lines();
            if let Some(status) = lines
                .next()
                .and_then(|value| value.trim().parse::<i32>().ok())
            {
                let completion = lines.next().unwrap_or("finished");
                let output = fs::read_to_string(output_path).unwrap_or_default();
                if completion == "cancelled" || cancel_path.exists() {
                    return Err(TerminalExecutionError::Cancelled);
                }
                if status == 0 {
                    return Ok(output);
                }
                return Err(TerminalExecutionError::CommandFailed { status, output });
            }
        }

        if cancel_path.exists() {
            return Err(TerminalExecutionError::Cancelled);
        }
        if start.elapsed() > timeout {
            if state.cancel_session_for_timeout(session_id) {
                // The mount is already available to the user. Its detached VM
                // can still be a descendant if the CLI parent is hung, so do
                // not signal this hierarchy or request device cleanup.
                return Ok(fs::read_to_string(output_path).unwrap_or_default());
            }
            return Err(TerminalExecutionError::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn cancel_terminal_session(session: &TerminalSession, cleanup_mount: bool) {
    // Serialize the cancel marker with the script's final pre-command check.
    // Whichever side acquires the gate first determines whether the command is
    // committed to start and therefore whether device cleanup is appropriate.
    let start_gate_path = session.cancel_path.with_file_name("start-gate");
    let gate_acquired = acquire_start_gate(&start_gate_path, Duration::from_secs(1));
    if let Err(error) = fs::write(&session.cancel_path, b"cancelled\n") {
        log::warn!("Failed to mark Terminal session cancelled: {}", error);
    }
    let command_started = session.launch_ack_path.exists();
    if gate_acquired {
        let _ = fs::remove_dir(&start_gate_path);
    }

    if let Some(pid) = read_session_pid(&session.pid_path).filter(|pid| *pid > 1) {
        terminate_process_tree(session, pid);
    }

    if cleanup_mount && command_started {
        if let Some(device) = session.operation.strip_prefix("mount:") {
            request_device_cleanup(session, device);
        }
    }
}

fn acquire_start_gate(path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match fs::create_dir(path) {
            Ok(()) => return true,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if Instant::now() >= deadline {
                    log::warn!("Timed out waiting for Terminal command start gate");
                    return false;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                log::warn!("Failed to acquire Terminal command start gate: {}", error);
                return false;
            }
        }
    }
}

fn ensure_terminal_session_stopped(state: &ElevationState, id: u64) {
    let session = state
        .sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&id)
        .cloned();
    let Some(session) = session else {
        return;
    };
    // `open` returns after LaunchServices accepts the request, not necessarily
    // after Terminal starts the script. Keep the cancel marker alive while we
    // wait for the script's acknowledgement/PID so a delayed launch cannot run
    // the privileged command after the user has cancelled it.
    let Some(pid) = wait_for_terminal_session_pid(&session, Duration::from_secs(5)) else {
        if session.launch_ack_path.exists() {
            log::warn!(
                "Terminal elevation acknowledged launch without publishing a valid shell PID"
            );
        } else {
            log::debug!("Terminal elevation did not launch before cancellation cleanup");
        }
        return;
    };
    if pid <= 1 {
        return;
    }

    terminate_process_tree(&session, pid);
}

fn read_session_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse::<u32>().ok()
}

fn wait_for_terminal_session_pid(session: &TerminalSession, timeout: Duration) -> Option<u32> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(pid) = read_session_pid(&session.pid_path) {
            return Some(pid);
        }
        if session.status_path.exists() || Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn child_process_ids(parent_pid: u32) -> Vec<u32> {
    let output = Command::new("/usr/bin/pgrep")
        .args(["-P", &parent_pid.to_string()])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .filter(|pid| *pid > 1)
        .collect()
}

fn process_tree(root_pid: u32) -> Vec<u32> {
    fn collect(pid: u32, seen: &mut HashSet<u32>, result: &mut Vec<u32>) {
        for child in child_process_ids(pid) {
            if seen.insert(child) {
                collect(child, seen, result);
                result.push(child);
            }
        }
    }

    let mut seen = HashSet::from([root_pid]);
    let mut result = Vec::new();
    collect(root_pid, &mut seen, &mut result);
    result.push(root_pid);
    result
}

fn signal_processes(pids: &[u32], signal: &str) {
    for pid in pids {
        let _ = Command::new("/bin/kill")
            .args([signal, &pid.to_string()])
            .status();
    }
}

fn process_identity(pid: u32) -> Option<String> {
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "lstart=", "-o", "command="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let identity = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!identity.is_empty()).then_some(identity)
}

fn recorded_terminal_shell_matches(session: &TerminalSession, pid: u32) -> bool {
    let Some(work_dir) = session.pid_path.parent() else {
        return false;
    };
    let expected_script = work_dir.join("run-anylinuxfs.command");
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output();
    let Ok(output) = output else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout).contains(expected_script.to_string_lossy().as_ref())
}

fn terminate_process_tree(session: &TerminalSession, root_pid: u32) {
    if root_pid <= 1 {
        return;
    }
    if !recorded_terminal_shell_matches(session, root_pid) {
        log::warn!(
            "Refusing to terminate stale or unexpected Terminal shell PID {}",
            root_pid
        );
        return;
    }

    // Snapshot descendants before terminating their parents so reparenting
    // cannot make deeper `script`/`sudo`/CLI processes invisible to cleanup.
    let pids = process_tree(root_pid);
    let identities: Vec<(u32, String)> = pids
        .iter()
        .filter_map(|pid| process_identity(*pid).map(|identity| (*pid, identity)))
        .collect();
    signal_processes(&pids, "-TERM");

    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if identities
            .iter()
            .all(|(pid, identity)| process_identity(*pid).as_ref() != Some(identity))
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let remaining: Vec<u32> = identities
        .iter()
        .filter_map(|(pid, identity)| {
            (process_identity(*pid).as_ref() == Some(identity)).then_some(*pid)
        })
        .collect();
    if remaining.is_empty() {
        return;
    }

    log::warn!(
        "{} Terminal elevation process(es) ignored cancellation; forcing termination",
        remaining.len()
    );
    signal_processes(&remaining, "-KILL");
}

fn request_device_cleanup(session: &TerminalSession, device: &str) {
    let child = Command::new(&session.cli_path)
        .args(["stop", device])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        log::warn!("Failed to start cleanup for cancelled mount {}", device);
        return;
    };

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    log::warn!(
                        "Cleanup for cancelled mount {} failed with status {}",
                        device,
                        status
                    );
                }
                return;
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Ok(None) => {
                log::warn!("Cleanup for cancelled mount {} timed out", device);
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
            Err(error) => {
                log::warn!(
                    "Failed while waiting for cancelled mount {} cleanup: {}",
                    device,
                    error
                );
                return;
            }
        }
    }
}

#[tauri::command]
pub fn get_elevation_policy(state: tauri::State<'_, Arc<ElevationState>>) -> ElevationPolicy {
    state.policy()
}

#[tauri::command]
pub fn set_elevation_mode(
    state: tauri::State<'_, Arc<ElevationState>>,
    mode: ElevationMode,
) -> Result<ElevationPolicy, String> {
    state.set_mode(mode)
}

#[tauri::command]
pub fn cancel_elevation_operation(
    state: tauri::State<'_, Arc<ElevationState>>,
    device: String,
) -> Result<usize, String> {
    if !valid_device_identifier(&device) {
        return Err("Invalid device path".to_string());
    }
    Ok(state.request_mount_cancellation(&device))
}

fn valid_device_identifier(device: &str) -> bool {
    if let Some(suffix) = device.strip_prefix("/dev/") {
        return !suffix.is_empty()
            && suffix.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            });
    }
    if device.starts_with("raid:") || device.starts_with("lvm:") {
        return device.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, ':' | '-' | '_')
        });
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_prevents_expansion_and_handles_single_quotes() {
        assert_eq!(shell_quote("/dev/disk7"), "'/dev/disk7'");
        assert_eq!(shell_quote("$(touch /tmp/nope)"), "'$(touch /tmp/nope)'");
        assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn secret_prompt_uses_unrecorded_pty_and_echo_protection() {
        let script = terminal_script_content(
            "/usr/bin/sudo '/opt/homebrew/bin/anylinuxfs' 'mount' '/dev/disk7'",
            Path::new("/tmp/output.txt"),
            Path::new("/tmp/status.txt"),
            Path::new("/tmp/cancel"),
            Path::new("/tmp/shell.pid"),
            Path::new("/tmp/launched"),
            true,
        );
        assert!(script.contains("/bin/stty -echo"));
        assert!(script.contains("/usr/bin/script -q /dev/null /usr/bin/sudo"));
        assert!(script.contains("shell.pid"));
        assert!(script.contains("cancel"));
        assert!(script.contains("/bin/mv -f '/tmp/launched.tmp' '/tmp/launched'"));
        assert!(script.contains("/bin/mv -f '/tmp/status.txt.tmp' '/tmp/status.txt'"));
        assert!(script.contains("finish_terminal 130; exit 130"));
        let cancel_check = script.find("if [[ -e '/tmp/cancel' ]]").unwrap();
        let command_start = script.find("command-starting").unwrap();
        let secret_prompt = script.find("/bin/stty -echo").unwrap();
        assert!(cancel_check < command_start && command_start < secret_prompt);
        assert!(!script.contains("/usr/bin/tee"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn non_secret_command_captures_parseable_output() {
        let script = terminal_script_content(
            "/usr/bin/sudo '/opt/homebrew/bin/anylinuxfs' 'list'",
            Path::new("/tmp/output.txt"),
            Path::new("/tmp/status.txt"),
            Path::new("/tmp/cancel"),
            Path::new("/tmp/shell.pid"),
            Path::new("/tmp/launched"),
            false,
        );
        assert!(script.contains("/usr/bin/tee '/tmp/output.txt'"));
        assert!(!script.contains("/bin/stty -echo"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn preexisting_cancel_marker_prevents_command_execution() {
        let directory = tempfile::tempdir().unwrap();
        let command_marker = directory.path().join("command-ran");
        let output_path = directory.path().join("output.txt");
        let status_path = directory.path().join("status.txt");
        let cancel_path = directory.path().join("cancel");
        let pid_path = directory.path().join("shell.pid");
        let launch_ack_path = directory.path().join("launched");
        fs::write(&cancel_path, b"cancelled\n").unwrap();

        let script = terminal_script_content(
            &format!(
                "/usr/bin/touch {}",
                shell_quote(&command_marker.to_string_lossy())
            ),
            &output_path,
            &status_path,
            &cancel_path,
            &pid_path,
            &launch_ack_path,
            false,
        );
        let script_path = directory.path().join("cancelled.command");
        fs::write(&script_path, script).unwrap();

        let result = Command::new("/bin/zsh").arg(&script_path).output().unwrap();
        assert_eq!(result.status.code(), Some(130));
        assert!(!command_marker.exists());
        assert!(!launch_ack_path.exists());
        assert_eq!(fs::read_to_string(status_path).unwrap(), "130\ncancelled\n");
    }

    #[test]
    fn cancellation_cleanup_waits_for_a_delayed_shell_pid() {
        let directory = tempfile::tempdir().unwrap();
        let pid_path = directory.path().join("shell.pid");
        let session = TerminalSession {
            operation: "list".to_string(),
            cancel_path: directory.path().join("cancel"),
            pid_path: pid_path.clone(),
            launch_ack_path: directory.path().join("launched"),
            status_path: directory.path().join("status"),
            cli_path: PathBuf::from("/missing/anylinuxfs"),
            persistent_mount: false,
            cancellation_started: false,
        };
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(75));
            fs::write(pid_path, b"4242\n").unwrap();
        });

        assert_eq!(
            wait_for_terminal_session_pid(&session, Duration::from_secs(1)),
            Some(4242)
        );
        writer.join().unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn cancellation_terminates_nested_processes() {
        let directory = tempfile::tempdir().unwrap();
        let script_path = directory.path().join("run-anylinuxfs.command");
        fs::write(
            &script_path,
            b"#!/bin/sh\ntrap '' TERM\n/bin/sleep 30 & wait\n",
        )
        .unwrap();
        let session = TerminalSession {
            operation: "list".to_string(),
            cancel_path: directory.path().join("cancel"),
            pid_path: directory.path().join("shell.pid"),
            launch_ack_path: directory.path().join("launched"),
            status_path: directory.path().join("status"),
            cli_path: PathBuf::from("/missing/anylinuxfs"),
            persistent_mount: false,
            cancellation_started: false,
        };
        let mut shell = Command::new("/bin/sh")
            .arg(&script_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let shell_pid = shell.id();

        let descendants = (0..20)
            .find_map(|_| {
                let descendants = child_process_ids(shell_pid);
                if descendants.is_empty() {
                    std::thread::sleep(Duration::from_millis(25));
                    None
                } else {
                    Some(descendants)
                }
            })
            .expect("nested process should start");

        assert!(recorded_terminal_shell_matches(&session, shell_pid));
        terminate_process_tree(&session, shell_pid);
        let _ = shell.wait();
        for pid in std::iter::once(shell_pid).chain(descendants) {
            let state = Command::new("/bin/ps")
                .args(["-p", &pid.to_string(), "-o", "stat="])
                .output()
                .unwrap();
            let state = String::from_utf8_lossy(&state.stdout);
            assert!(state.trim().is_empty() || state.trim().starts_with('Z'));
        }
    }

    #[test]
    #[cfg(unix)]
    fn cancelled_mount_waits_for_device_cleanup() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let cleanup_marker = directory.path().join("cleanup-finished");
        let cli_path = directory.path().join("fake-anylinuxfs");
        fs::write(
            &cli_path,
            format!(
                "#!/bin/sh\n/bin/sleep 0.1\n/usr/bin/touch {}\n",
                shell_quote(&cleanup_marker.to_string_lossy())
            ),
        )
        .unwrap();
        fs::set_permissions(&cli_path, fs::Permissions::from_mode(0o700)).unwrap();
        let session = TerminalSession {
            operation: "mount:/dev/disk7".to_string(),
            cancel_path: directory.path().join("cancel"),
            pid_path: directory.path().join("missing.pid"),
            launch_ack_path: directory.path().join("launched"),
            status_path: directory.path().join("status"),
            cli_path,
            persistent_mount: false,
            cancellation_started: false,
        };
        fs::write(&session.launch_ack_path, b"launched\n").unwrap();

        cancel_terminal_session(&session, true);
        assert!(cleanup_marker.exists());
    }

    #[test]
    #[cfg(unix)]
    fn cancellation_waits_for_command_start_decision() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let cleanup_marker = directory.path().join("cleanup-finished");
        let cli_path = directory.path().join("fake-anylinuxfs");
        fs::write(
            &cli_path,
            format!(
                "#!/bin/sh\n/usr/bin/touch {}\n",
                shell_quote(&cleanup_marker.to_string_lossy())
            ),
        )
        .unwrap();
        fs::set_permissions(&cli_path, fs::Permissions::from_mode(0o700)).unwrap();
        let session = TerminalSession {
            operation: "mount:/dev/disk7".to_string(),
            cancel_path: directory.path().join("cancel"),
            pid_path: directory.path().join("missing.pid"),
            launch_ack_path: directory.path().join("launched"),
            status_path: directory.path().join("status"),
            cli_path,
            persistent_mount: false,
            cancellation_started: false,
        };
        let start_gate_path = directory.path().join("start-gate");
        fs::create_dir(&start_gate_path).unwrap();

        let cancelling_session = session.clone();
        let cancellation =
            std::thread::spawn(move || cancel_terminal_session(&cancelling_session, true));
        std::thread::sleep(Duration::from_millis(50));
        assert!(!session.cancel_path.exists());

        fs::write(&session.launch_ack_path, b"command-starting\n").unwrap();
        fs::remove_dir(start_gate_path).unwrap();
        cancellation.join().unwrap();

        assert!(session.cancel_path.exists());
        assert!(cleanup_marker.exists());
    }

    #[test]
    fn preference_round_trip_uses_rust_owned_storage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.toml");
        let state = ElevationState::load(path.clone());

        assert_eq!(state.mode(), ElevationMode::Native);
        state.set_mode(ElevationMode::InteractiveTerminal).unwrap();
        assert_eq!(
            ElevationState::load(path).mode(),
            ElevationMode::InteractiveTerminal
        );
    }

    #[test]
    #[cfg(unix)]
    fn preference_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.toml");
        let state = ElevationState::load(path.clone());
        state.set_mode(state.mode()).unwrap();
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn elevation_mode_is_stable_while_an_operation_is_active() {
        let directory = tempfile::tempdir().unwrap();
        let state = Arc::new(ElevationState::load(
            directory.path().join("preferences.toml"),
        ));
        state.set_mode(ElevationMode::InteractiveTerminal).unwrap();

        let operation = state.begin_operation("list").unwrap();
        assert_eq!(operation.mode(), ElevationMode::InteractiveTerminal);
        assert!(state.set_mode(ElevationMode::Native).is_err());
        assert!(state.begin_operation("list").is_err());
        drop(operation);

        state.set_mode(ElevationMode::Native).unwrap();
        assert_eq!(state.mode(), ElevationMode::Native);
    }

    #[test]
    fn timeout_marks_session_cancelled_and_does_not_leave_it_active() {
        let directory = tempfile::tempdir().unwrap();
        let state = ElevationState::load(directory.path().join("preferences.toml"));
        let cancel_path = directory.path().join("cancel");
        let pid_path = directory.path().join("missing.pid");
        let (session_id, _) = state.register_session(
            "list".to_string(),
            cancel_path.clone(),
            pid_path,
            directory.path().join("launched"),
            directory.path().join("status"),
            PathBuf::from("/missing/anylinuxfs"),
        );
        let result = wait_for_terminal_result(
            &state,
            session_id,
            &directory.path().join("status"),
            &directory.path().join("output"),
            &cancel_path,
            Duration::from_millis(1),
        );
        assert!(matches!(result, Err(TerminalExecutionError::TimedOut)));
        assert!(cancel_path.exists());
        state.unregister_session(session_id);
        assert_eq!(state.active_session_count(), 0);
    }

    #[test]
    fn persistent_mount_timeout_does_not_cancel_processes() {
        let directory = tempfile::tempdir().unwrap();
        let state = ElevationState::load(directory.path().join("preferences.toml"));
        let cancel_path = directory.path().join("cancel");
        let status_path = directory.path().join("status");
        fs::write(&status_path, b"pending\n").unwrap();
        let (session_id, _) = state.register_session(
            "mount:/dev/disk7".to_string(),
            cancel_path.clone(),
            directory.path().join("missing.pid"),
            directory.path().join("launched"),
            status_path.clone(),
            PathBuf::from("/missing/anylinuxfs"),
        );
        assert!(state.mark_mount_persistent("/dev/disk7"));

        let result = wait_for_terminal_result(
            &state,
            session_id,
            &status_path,
            &directory.path().join("output"),
            &cancel_path,
            Duration::from_millis(1),
        );

        assert_eq!(result.unwrap(), "");
        assert!(!cancel_path.exists());
        state.unregister_session(session_id);
        assert_eq!(state.active_session_count(), 0);
    }

    #[test]
    fn timeout_cancellation_cannot_be_overwritten_by_persistence() {
        let directory = tempfile::tempdir().unwrap();
        let state = ElevationState::load(directory.path().join("preferences.toml"));
        state.set_mode(ElevationMode::InteractiveTerminal).unwrap();
        let (session_id, _) = state.register_session(
            "mount:/dev/disk7".to_string(),
            directory.path().join("cancel"),
            directory.path().join("missing.pid"),
            directory.path().join("launched"),
            directory.path().join("status"),
            PathBuf::from("/missing/anylinuxfs"),
        );

        assert!(!state.cancel_session_for_timeout(session_id));
        assert!(!state.mark_mount_persistent("/dev/disk7"));
        state.unregister_session(session_id);
        assert!(!state.mark_mount_persistent("/dev/disk7"));
    }

    #[test]
    fn cancellation_before_terminal_registration_is_not_lost() {
        let directory = tempfile::tempdir().unwrap();
        let state = Arc::new(ElevationState::load(
            directory.path().join("preferences.toml"),
        ));
        let operation_guard = state
            .begin_operation("mount:/dev/disk7")
            .expect("operation should begin");
        assert!(state.begin_operation("mount:/dev/disk7").is_err());
        assert_eq!(state.request_mount_cancellation("/dev/disk7"), 1);
        assert!(!state.mark_mount_persistent("/dev/disk7"));

        let cancel_path = directory.path().join("cancel");
        let (_, cancellation_observed) = state.register_session(
            "mount:/dev/disk7".to_string(),
            cancel_path.clone(),
            directory.path().join("missing.pid"),
            directory.path().join("launched"),
            directory.path().join("status"),
            PathBuf::from("/missing/anylinuxfs"),
        );
        assert!(cancellation_observed);
        assert!(cancel_path.exists());

        drop(operation_guard);
        assert_eq!(state.request_mount_cancellation("/dev/disk7"), 0);
    }

    #[test]
    fn cancellation_device_validation_matches_supported_identifiers() {
        for valid in ["/dev/disk7", "/dev/disk7s1", "raid:md0", "lvm:vg_data"] {
            assert!(valid_device_identifier(valid), "expected valid: {}", valid);
        }
        for invalid in ["disk7", "/dev/../disk7", "raid:md0;reboot", ""] {
            assert!(
                !valid_device_identifier(invalid),
                "expected invalid: {}",
                invalid
            );
        }
    }

    #[test]
    fn persistent_mounts_are_not_cancelled_during_app_shutdown() {
        let directory = tempfile::tempdir().unwrap();
        let state = ElevationState::load(directory.path().join("preferences.toml"));
        let cancel_path = directory.path().join("cancel");
        state.register_session(
            "mount:/dev/disk7".to_string(),
            cancel_path.clone(),
            directory.path().join("missing.pid"),
            directory.path().join("launched"),
            directory.path().join("status"),
            PathBuf::from("/missing/anylinuxfs"),
        );
        assert!(state.mark_mount_persistent("/dev/disk7"));
        assert_eq!(state.cancel_all_pending(), 0);
        assert!(!cancel_path.exists());
    }
}
