use std::process::{Command, Stdio};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::sync::OnceLock;

/// Sanitize error output to avoid exposing sensitive system details
/// Logs the full error for debugging but returns a user-friendly message
fn sanitize_error(stdout: &str, stderr: &str) -> String {
    // Log full details for debugging
    if !stdout.is_empty() || !stderr.is_empty() {
        log::debug!("Command failed - stdout: {}, stderr: {}", stdout, stderr);
    }

    // Check for common error patterns and return user-friendly messages
    let combined = format!("{}{}", stdout, stderr);

    if combined.contains("not mounted") || combined.contains("No such file") {
        return "Filesystem is not mounted".to_string();
    }
    if combined.contains("Permission denied") {
        return "Permission denied - try running with administrator privileges".to_string();
    }
    if combined.contains("Device busy") || combined.contains("resource busy") {
        return "Device is busy - close any applications using it and try again".to_string();
    }
    if combined.contains("Invalid argument") {
        return "Invalid operation or unsupported filesystem".to_string();
    }
    if combined.contains("No space left") {
        return "No space left on device".to_string();
    }
    if combined.contains("Read-only") {
        return "Filesystem is read-only".to_string();
    }

    // LUKS/encryption errors — pass through with keyword so mount_disk can detect them
    if combined.contains("LUKS") || combined.contains("luks")
        || combined.contains("decrypt") || combined.contains("passphrase")
        || combined.contains("encrypted") || combined.contains("wrong key")
    {
        return format!("Encrypted volume - {}", combined.lines()
            .find(|l| l.to_lowercase().contains("luks") || l.to_lowercase().contains("decrypt")
                || l.to_lowercase().contains("passphrase") || l.to_lowercase().contains("encrypted"))
            .unwrap_or("decryption failed"));
    }

    // For anylinuxfs-specific errors, extract the message after "Error:"
    if let Some(pos) = combined.find("Error:") {
        let error_msg = combined[pos + 6..].trim();
        // Take first line only, limit length
        let first_line = error_msg.lines().next().unwrap_or(error_msg);
        if first_line.len() <= 200 {
            return first_line.to_string();
        }
    }

    // Generic fallback - don't expose raw output
    "Operation failed - check logs for details".to_string()
}

/// Common locations to search for anylinuxfs
const SEARCH_PATHS: &[&str] = &[
    "/opt/homebrew/bin/anylinuxfs",
    "/usr/local/bin/anylinuxfs",
    "/usr/bin/anylinuxfs",
];

/// Cached path to anylinuxfs binary
static ANYLINUXFS_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Find anylinuxfs in PATH or common locations
fn find_anylinuxfs() -> Option<PathBuf> {
    // First check ANYLINUXFS_PATH environment variable
    if let Ok(env_path) = std::env::var("ANYLINUXFS_PATH") {
        let path = PathBuf::from(&env_path);
        if path.exists() {
            return Some(path);
        }
    }

    // Search in PATH using `which`
    if let Ok(output) = Command::new("which").arg("anylinuxfs").output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                let path = PathBuf::from(&path_str);
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    // Fall back to common locations
    for search_path in SEARCH_PATHS {
        let path = Path::new(search_path);
        if path.exists() {
            return Some(path.to_path_buf());
        }
    }

    None
}

/// Get the cached path to anylinuxfs, finding it if needed
fn get_anylinuxfs_path() -> Option<&'static PathBuf> {
    ANYLINUXFS_PATH.get_or_init(find_anylinuxfs).as_ref()
}

/// Check if the anylinuxfs CLI is available
pub fn is_available() -> bool {
    get_anylinuxfs_path().is_some()
}

/// Get the version of the anylinuxfs CLI
pub fn get_version() -> Option<String> {
    let cli_path = get_anylinuxfs_path()?;

    let output = Command::new(cli_path)
        .arg("--version")
        .output()
        .ok()?;

    if output.status.success() {
        let version_str = String::from_utf8_lossy(&output.stdout);
        // Parse "anylinuxfs 0.10.2" -> "0.10.2"
        version_str
            .trim()
            .strip_prefix("anylinuxfs ")
            .map(|v| v.to_string())
            .or_else(|| Some(version_str.trim().to_string()))
    } else {
        None
    }
}

/// Get the path to the anylinuxfs CLI
pub fn get_path() -> Option<&'static Path> {
    get_anylinuxfs_path().map(|p| p.as_path())
}

/// Execute an anylinuxfs command with optional sudo elevation
///
/// When `silent` is true and sudo credentials have expired, returns an
/// `AUTH_EXPIRED` error instead of showing an interactive password dialog.
/// This is used for automatic background refreshes (e.g. disk-watcher events)
/// so the user isn't bombarded with auth dialogs while away from the computer.
pub fn execute_command(args: &[&str], needs_sudo: bool, passphrase: Option<&str>, silent: bool) -> Result<String, String> {
    if needs_sudo {
        execute_with_sudo(args, passphrase, silent)
    } else {
        execute_direct(args, passphrase)
    }
}

fn execute_direct(args: &[&str], passphrase: Option<&str>) -> Result<String, String> {
    let cli_path = get_anylinuxfs_path()
        .ok_or_else(|| "anylinuxfs CLI not found in PATH or standard locations".to_string())?;

    let mut cmd = Command::new(cli_path);
    cmd.args(args);
    // Use piped stdin instead of null - libkrun's epoll fails with /dev/null
    cmd.stdin(Stdio::piped());

    if let Some(pass) = passphrase {
        cmd.env("ALFS_PASSPHRASE", pass);
    }

    let output = cmd.output().map_err(|e| format!("Failed to execute command: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(sanitize_error(&stdout, &stderr))
    }
}

/// Try sudo via native PAM auth (handles cached credentials, Touch ID, Apple Watch)
/// Returns None if auth fails/unavailable, falling back to askpass dialog
fn try_sudo_native(cli_path: &Path, args: &[&str], passphrase: Option<&str>) -> Option<Result<String, String>> {
    let cli_path_str = cli_path.to_string_lossy();
    let mut sudo_args: Vec<&str> = if passphrase.is_some() {
        vec!["--preserve-env=ALFS_PASSPHRASE", "--", &*cli_path_str]
    } else {
        vec!["--", &*cli_path_str]
    };
    sudo_args.extend(args.iter().copied());

    let mut cmd = Command::new("sudo");
    cmd.args(&sudo_args);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    if let Some(pass) = passphrase {
        cmd.env("ALFS_PASSPHRASE", pass);
    }

    let mut child = cmd.spawn().ok()?;

    // Native auth (cached creds, biometric) is fast — give it 10 seconds
    let timeout = Duration::from_secs(10);
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(ref mut out) = child.stdout {
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(ref mut err) = child.stderr {
                    let _ = err.read_to_string(&mut stderr);
                }

                if status.success() {
                    return Some(Ok(stdout));
                }
                // If sudo failed because no credential (user denied Touch ID
                // or no cached credential), return None to fall back to askpass
                if stderr.contains("a password is required")
                    || stderr.contains("no askpass")
                    || stderr.contains("a terminal is required")
                {
                    return None;
                }
                // Real error — return it
                return Some(Err(sanitize_error(&stdout, &stderr)));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None; // Timeout — fall back to askpass
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return None,
        }
    }
}

fn execute_with_sudo(args: &[&str], passphrase: Option<&str>, silent: bool) -> Result<String, String> {
    let cli_path = get_anylinuxfs_path()
        .ok_or_else(|| "anylinuxfs CLI not found in PATH or standard locations".to_string())?;

    // Try native PAM auth first (handles cached credentials, Touch ID, Apple Watch)
    // If it fails or is unavailable, fall back to askpass password dialog
    match try_sudo_native(cli_path, args, passphrase) {
        Some(Ok(stdout)) => return Ok(stdout),
        Some(Err(e)) => return Err(e),
        None => {
            if silent {
                log::debug!("sudo: native auth expired, silent mode — skipping password dialog");
                return Err("ALFS_SILENT_AUTH_EXPIRED".to_string());
            }
            log::debug!("sudo: native auth unavailable, falling back to password dialog");
        }
    }

    // Fall back to native macOS authorization dialog (supports Touch ID)
    execute_with_osascript_admin(cli_path, args, passphrase)
}

/// Escape a string for use inside a single-quoted shell argument.
/// Replaces `'` with `'\''` (end quote, escaped quote, start quote).
fn shell_escape(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Escape a string for use inside an AppleScript double-quoted string.
/// Escapes backslashes and double quotes.
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Execute a command with administrator privileges via the native macOS
/// authorization dialog (`do shell script ... with administrator privileges`).
/// This shows the standard macOS auth prompt which supports Touch ID/Face ID.
fn execute_with_osascript_admin(cli_path: &Path, args: &[&str], passphrase: Option<&str>) -> Result<String, String> {
    let cli_path_str = cli_path.to_string_lossy();

    // Build the inner shell command: ALFS_PASSPHRASE='...' /path/to/anylinuxfs arg1 arg2
    let mut shell_cmd = String::new();
    if let Some(pass) = passphrase {
        shell_cmd.push_str(&format!("ALFS_PASSPHRASE='{}' ", shell_escape(pass)));
    }
    shell_cmd.push_str(&format!("'{}'", shell_escape(&cli_path_str)));
    for arg in args {
        shell_cmd.push_str(&format!(" '{}'", shell_escape(arg)));
    }

    // Wrap in AppleScript: do shell script "<cmd>" with administrator privileges
    let applescript = format!(
        "do shell script \"{}\" with administrator privileges",
        applescript_escape(&shell_cmd)
    );

    let mut cmd = Command::new("osascript");
    cmd.arg("-e").arg(&applescript);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()
        .map_err(|e| format!("Failed to execute osascript: {}", e))?;

    // 60-second timeout — user needs time to authenticate via Touch ID / password
    let timeout = Duration::from_secs(60);
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(ref mut out) = child.stdout {
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(ref mut err) = child.stderr {
                    let _ = err.read_to_string(&mut stderr);
                }

                if status.success() {
                    return Ok(stdout);
                }

                // User pressed Cancel in the auth dialog
                if stderr.contains("User canceled")
                    || stderr.contains("user canceled")
                    || stderr.contains("-128")
                {
                    return Err("Authentication cancelled".to_string());
                }

                return Err(sanitize_error(&stdout, &stderr));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Command timed out".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return Err(format!("Error waiting for process: {}", e));
            }
        }
    }
}
