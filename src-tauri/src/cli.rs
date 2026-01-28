use std::process::{Command, Stdio};
use std::io::Read;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::sync::OnceLock;

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
pub fn execute_command(args: &[&str], needs_sudo: bool, passphrase: Option<&str>) -> Result<String, String> {
    if needs_sudo {
        execute_with_sudo(args, passphrase)
    } else {
        execute_direct(args, passphrase)
    }
}

fn execute_direct(args: &[&str], passphrase: Option<&str>) -> Result<String, String> {
    let cli_path = get_anylinuxfs_path()
        .ok_or_else(|| "anylinuxfs CLI not found in PATH or standard locations".to_string())?;

    let mut cmd = Command::new(cli_path);
    cmd.args(args);
    cmd.stdin(Stdio::null());

    if let Some(pass) = passphrase {
        cmd.env("ALFS_PASSPHRASE", pass);
    }

    let output = cmd.output().map_err(|e| format!("Failed to execute command: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(format!("{}{}", stdout, stderr))
    }
}

fn execute_with_sudo(args: &[&str], passphrase: Option<&str>) -> Result<String, String> {
    let cli_path = get_anylinuxfs_path()
        .ok_or_else(|| "anylinuxfs CLI not found in PATH or standard locations".to_string())?;

    // Create a temporary askpass script that uses osascript
    // This way the password never passes through our code
    let askpass_script = create_askpass_script()?;

    // Build the command arguments for sudo with SUDO_ASKPASS
    let cli_path_str = cli_path.to_string_lossy();
    let mut sudo_args: Vec<&str> = vec!["-A", "--", &cli_path_str];
    sudo_args.extend(args.iter().copied());

    let mut cmd = Command::new("sudo");
    cmd.args(&sudo_args);
    cmd.env("SUDO_ASKPASS", &askpass_script);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    if let Some(pass) = passphrase {
        cmd.env("ALFS_PASSPHRASE", pass);
    }

    // Spawn the process so we can handle it with timeout
    let mut child = cmd.spawn()
        .map_err(|e| format!("Failed to execute sudo: {}", e))?;

    // Wait for process with timeout (30 seconds for mount operations)
    let timeout = Duration::from_secs(30);
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process finished
                let _ = fs::remove_file(&askpass_script);

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
                } else {
                    // Check for wrong password or cancelled
                    if stderr.contains("Sorry, try again") || stderr.contains("incorrect password") {
                        return Err("Incorrect password".to_string());
                    } else if stderr.contains("no askpass program") || stderr.contains("no password was provided") {
                        return Err("Authentication cancelled".to_string());
                    } else {
                        return Err(format!("{}{}", stdout, stderr));
                    }
                }
            }
            Ok(None) => {
                // Process still running
                if start.elapsed() > timeout {
                    // Timeout - kill the process
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = fs::remove_file(&askpass_script);
                    return Err("Command timed out".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                let _ = fs::remove_file(&askpass_script);
                return Err(format!("Error waiting for process: {}", e));
            }
        }
    }
}

fn create_askpass_script() -> Result<String, String> {
    let script_path = "/tmp/anylinuxfs-askpass.sh";

    // Create askpass script that uses osascript to prompt for password
    // The password goes directly from osascript to sudo, never through our app
    let script_content = r#"#!/bin/bash
osascript -e 'Tell application "System Events" to display dialog "anylinuxfs requires administrator privileges." & return & return & "Enter your password:" with hidden answer default answer "" buttons {"Cancel", "OK"} default button "OK" with title "Authentication Required" with icon caution' -e 'text returned of result' 2>/dev/null
"#;

    fs::write(script_path, script_content)
        .map_err(|e| format!("Failed to create askpass script: {}", e))?;

    // Make it executable
    fs::set_permissions(script_path, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("Failed to set script permissions: {}", e))?;

    Ok(script_path.to_string())
}
