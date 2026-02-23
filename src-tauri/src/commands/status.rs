use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;
use crate::cache;
use crate::cli;
use crate::paths::get_socket_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliStatus {
    pub available: bool,
    pub path: String,
    pub initialized: bool,
    pub reinit_pending: bool,
    pub cli_version: Option<String>,
    pub gui_version: String,
}

fn check_vm_initialized() -> bool {
    // Check if the Alpine rootfs exists, indicating the VM has been initialized
    if let Some(home) = dirs::home_dir() {
        home.join(".anylinuxfs/alpine/rootfs").exists()
    } else {
        false
    }
}

fn check_reinit_pending() -> bool {
    let cli_path = match cli::get_path() {
        Some(p) => p,
        None => return false,
    };

    // Resolve symlink to get the real install path
    // e.g. /opt/homebrew/Cellar/anylinuxfs/0.11.2/bin/anylinuxfs
    let real_path = match std::fs::canonicalize(cli_path) {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Go up 2 levels (past bin/anylinuxfs) to get install prefix
    let prefix = match real_path.parent().and_then(|p| p.parent()) {
        Some(p) => p,
        None => return false,
    };

    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return false,
    };

    let anylinuxfs_dir = home.join(".anylinuxfs");

    // Check alpine: compare {prefix}/share/alpine/rootfs.ver vs ~/.anylinuxfs/alpine/rootfs.ver
    let alpine_desired = prefix.join("share/alpine/rootfs.ver");
    let alpine_installed = anylinuxfs_dir.join("alpine/rootfs.ver");
    if version_mismatch(&alpine_desired, &alpine_installed) {
        return true;
    }

    // Check freebsd: directory is freebsd-15.0 etc., so scan for freebsd* dirs
    // Only flag reinit if the user has actually installed a FreeBSD image
    let freebsd_desired = prefix.join("share/freebsd/rootfs.ver");
    if freebsd_desired.exists() {
        if let Ok(entries) = std::fs::read_dir(&anylinuxfs_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("freebsd") && entry.path().is_dir() {
                    let installed_ver = entry.path().join("rootfs.ver");
                    if version_mismatch(&freebsd_desired, &installed_ver) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

fn version_mismatch(desired: &std::path::Path, installed: &std::path::Path) -> bool {
    let desired_ver = match std::fs::read_to_string(desired) {
        Ok(v) => v.trim().to_string(),
        Err(_) => return false, // No desired version file → no reinit needed
    };

    let installed_ver = match std::fs::read_to_string(installed) {
        Ok(v) => v.trim().to_string(),
        Err(_) => return true, // Desired exists but installed doesn't → reinit pending
    };

    desired_ver != installed_ver
}

#[tauri::command]
pub fn check_cli() -> CliStatus {
    let available = cli::is_available();
    let initialized = check_vm_initialized();
    CliStatus {
        available,
        path: cli::get_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "not found".to_string()),
        initialized,
        reinit_pending: if available { check_reinit_pending() } else { false },
        cli_version: cli::get_version(),
        gui_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    pub mounted: bool,
    pub device: Option<String>,
    pub mount_point: Option<String>,
    pub filesystem: Option<String>,
    pub vm_running: bool,
    pub ram_mb: Option<u32>,
    pub vcpus: Option<u32>,
    pub orphaned_instance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeInfo {
    device: Option<String>,
    mount_point: Option<String>,
    filesystem: Option<String>,
    vm_pid: Option<u32>,
    ram_mb: Option<u32>,
    vcpus: Option<u32>,
}

#[tauri::command]
pub async fn get_mount_status() -> Result<MountInfo, String> {
    // Run in blocking task to avoid freezing UI
    tokio::task::spawn_blocking(|| get_mount_status_sync())
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

pub fn get_mount_status_sync() -> Result<MountInfo, String> {
    // First, try the socket approach
    if let Some(info) = try_socket_status() {
        return Ok(info);
    }

    // Fallback: check if mount point exists and has content
    if let Some(info) = check_mount_point_fallback() {
        return Ok(info);
    }

    // Check for orphaned instance (VM running but no mount)
    let orphaned = check_orphaned_instance();

    // Not mounted
    Ok(MountInfo {
        mounted: false,
        device: None,
        mount_point: None,
        filesystem: None,
        vm_running: orphaned,
        ram_mb: None,
        vcpus: None,
        orphaned_instance: orphaned,
    })
}

fn try_socket_status() -> Option<MountInfo> {
    let socket_path = get_socket_path();
    let stream = UnixStream::connect(socket_path).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();

    let mut stream = stream;

    // Send status request
    let request = r#"{"command": "status"}"#;
    stream.write_all(request.as_bytes()).ok()?;

    // Read response
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).ok()?;
    if n == 0 {
        return None;
    }

    let response = String::from_utf8_lossy(&buf[..n]).to_string();

    // Parse response
    if let Ok(info) = serde_json::from_str::<RuntimeInfo>(&response) {
        if info.mount_point.is_some() {
            return Some(MountInfo {
                mounted: true,
                device: info.device,
                mount_point: info.mount_point,
                filesystem: info.filesystem,
                vm_running: info.vm_pid.is_some(),
                ram_mb: info.ram_mb,
                vcpus: info.vcpus,
                orphaned_instance: false,
            });
        }
    }

    None
}

fn check_orphaned_instance() -> bool {
    // Only check for actual running processes, not stale socket files
    // The socket file can be left over from previous sessions
    // Uses cached pgrep results to avoid redundant process spawning
    cache::is_vm_running_cached()
}

fn check_mount_point_fallback() -> Option<MountInfo> {
    // Check for localhost NFS mounts (anylinuxfs signature)
    // Uses cached mount output to avoid redundant process spawning
    let output = cache::get_mount_output()?;
    let mount_output = String::from_utf8_lossy(&output.stdout);

    // Look for anylinuxfs NFS mount pattern: localhost:/mnt/XXX on /Volumes/XXX
    for line in mount_output.lines() {
        if line.contains("localhost:/mnt/") && line.contains("/Volumes/") {
            // Parse: localhost:/mnt/appfs on /Volumes/appfs (nfs, ...)
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let mount_point = parts[2].to_string();
                return Some(MountInfo {
                    mounted: true,
                    device: Some(parts[0].to_string()),
                    mount_point: Some(mount_point),
                    filesystem: Some("nfs".to_string()),
                    vm_running: true,
                    ram_mb: None,
                    vcpus: None,
                    orphaned_instance: false,
                });
            }
        }
    }

    None
}
