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
    pub locked: bool,
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
    cli_path: PathBuf,
    persistent_mount: bool,
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
}

impl Drop for ElevationOperationGuard {
    fn drop(&mut self) {
        self.state.finish_operation(&self.operation);
    }
}

impl ElevationState {
    pub fn load(config_path: PathBuf) -> Self {
        let mode = if managed_terminal_elevation_locked() {
            ElevationMode::InteractiveTerminal
        } else {
            read_stored_mode(&config_path).unwrap_or(ElevationMode::Native)
        };

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
        ElevationPolicy {
            mode: self.mode(),
            locked: managed_terminal_elevation_locked(),
        }
    }

    pub fn mode(&self) -> ElevationMode {
        *self
            .mode
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set_mode(&self, mode: ElevationMode) -> Result<ElevationPolicy, String> {
        if managed_terminal_elevation_locked() && mode != ElevationMode::InteractiveTerminal {
            return Err("This build requires interactive Terminal elevation".to_string());
        }

        write_stored_mode(&self.config_path, mode)?;
        *self
            .mode
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = mode;
        Ok(self.policy())
    }

    fn register_session(
        &self,
        operation: String,
        cancel_path: PathBuf,
        pid_path: PathBuf,
        cli_path: PathBuf,
    ) -> (u64, bool) {
        let id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        let session = TerminalSession {
            operation: operation.clone(),
            cancel_path,
            pid_path,
            cli_path,
            persistent_mount: false,
        };
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, session.clone());

        // A UI cancellation can arrive after mount_disk starts but just before
        // Terminal registration. Consume that request here so no command is
        // launched after the user has cancelled it.
        let cancellation_requested = self
            .cancellation_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&operation);
        if cancellation_requested {
            cancel_terminal_session(&session);
        }
        (id, cancellation_requested)
    }

    fn unregister_session(&self, id: u64) {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&id);
    }

    pub fn mark_mount_persistent(&self, device: &str) {
        let operation = format!("mount:{}", device);
        for session in self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values_mut()
        {
            if session.operation == operation {
                session.persistent_mount = true;
            }
        }
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
        let mut cancellation_requests = self
            .cancellation_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cancellation_requests.remove(&operation);
        active_operations.insert(operation.clone());
        Ok(ElevationOperationGuard {
            state: self.clone(),
            operation,
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

    fn cancel_session(&self, id: u64) -> bool {
        let session = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&id)
            .cloned();

        if let Some(session) = session {
            cancel_terminal_session(&session);
            true
        } else {
            false
        }
    }

    fn cancel_matching(&self, operation: Option<&str>, include_persistent: bool) -> usize {
        let sessions: Vec<TerminalSession> = self
            .sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .filter(|session| {
                (include_persistent || !session.persistent_mount)
                    && operation
                        .map(|value| value == session.operation.as_str())
                        .unwrap_or(true)
            })
            .cloned()
            .collect();

        for session in &sessions {
            cancel_terminal_session(session);
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

fn managed_terminal_elevation_locked() -> bool {
    cfg!(feature = "managed-terminal-elevation")
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
    prompts_for_secret: bool,
) -> String {
    let prelude = format!(
        r#"#!/bin/zsh
/usr/bin/printf '%s\n' "$$" > {pid_path}
restore_terminal_echo() {{ /bin/stty echo 2>/dev/null }}
finish_terminal() {{
  command_status="$1"
  completion="finished"
  [[ -e {cancel_path} ]] && completion="cancelled"
  /usr/bin/printf '%s\n%s\n' "$command_status" "$completion" > {status_temp_path}
  /bin/mv -f {status_temp_path} {status_path}
  restore_terminal_echo
}}
trap 'finish_terminal 130; exit 130' HUP INT TERM
"#,
        pid_path = shell_quote(&pid_path.to_string_lossy()),
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
            state.cancel_session(session_id);
            return Err(TerminalExecutionError::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn cancel_terminal_session(session: &TerminalSession) {
    if let Err(error) = fs::write(&session.cancel_path, b"cancelled\n") {
        log::warn!("Failed to mark Terminal session cancelled: {}", error);
    }

    if let Ok(pid_text) = fs::read_to_string(&session.pid_path) {
        if let Ok(pid) = pid_text.trim().parse::<u32>() {
            if pid > 1 {
                let _ = Command::new("/usr/bin/pkill")
                    .args(["-TERM", "-P", &pid.to_string()])
                    .status();
                let _ = Command::new("/bin/kill")
                    .args(["-TERM", &pid.to_string()])
                    .status();
            }
        }
    }

    if let Some(device) = session.operation.strip_prefix("mount:") {
        let _ = Command::new(&session.cli_path)
            .args(["stop", device])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
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
    let Ok(pid_text) = fs::read_to_string(&session.pid_path) else {
        return;
    };
    let Ok(pid) = pid_text.trim().parse::<u32>() else {
        return;
    };
    if pid <= 1 {
        return;
    }

    for _ in 0..20 {
        if !process_exists(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    log::warn!(
        "Terminal elevation shell {} did not stop after cancellation; forcing termination",
        pid
    );
    let _ = Command::new("/usr/bin/pkill")
        .args(["-KILL", "-P", &pid.to_string()])
        .status();
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .status();
}

fn process_exists(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
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
            true,
        );
        assert!(script.contains("/bin/stty -echo"));
        assert!(script.contains("/usr/bin/script -q /dev/null /usr/bin/sudo"));
        assert!(script.contains("shell.pid"));
        assert!(script.contains("cancel"));
        assert!(script.contains("/bin/mv -f '/tmp/status.txt.tmp' '/tmp/status.txt'"));
        assert!(script.contains("finish_terminal 130; exit 130"));
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
            false,
        );
        assert!(script.contains("/usr/bin/tee '/tmp/output.txt'"));
        assert!(!script.contains("/bin/stty -echo"));
    }

    #[test]
    fn preference_round_trip_uses_rust_owned_storage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.toml");
        let state = ElevationState::load(path.clone());

        if managed_terminal_elevation_locked() {
            assert_eq!(state.mode(), ElevationMode::InteractiveTerminal);
            assert!(state.set_mode(ElevationMode::Native).is_err());
        } else {
            assert_eq!(state.mode(), ElevationMode::Native);
            state.set_mode(ElevationMode::InteractiveTerminal).unwrap();
            assert_eq!(
                ElevationState::load(path).mode(),
                ElevationMode::InteractiveTerminal
            );
        }
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
    fn timeout_marks_session_cancelled_and_does_not_leave_it_active() {
        let directory = tempfile::tempdir().unwrap();
        let state = ElevationState::load(directory.path().join("preferences.toml"));
        let cancel_path = directory.path().join("cancel");
        let pid_path = directory.path().join("missing.pid");
        let (session_id, _) = state.register_session(
            "list".to_string(),
            cancel_path.clone(),
            pid_path,
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

        let cancel_path = directory.path().join("cancel");
        let (_, cancellation_observed) = state.register_session(
            "mount:/dev/disk7".to_string(),
            cancel_path.clone(),
            directory.path().join("missing.pid"),
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
            PathBuf::from("/missing/anylinuxfs"),
        );
        state.mark_mount_persistent("/dev/disk7");
        assert_eq!(state.cancel_all_pending(), 0);
        assert!(!cancel_path.exists());
    }
}
