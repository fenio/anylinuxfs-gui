use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::Duration;
use crate::cli;

const SOCKET_PATH: &str = "/tmp/anylinuxfs.sock";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliStatus {
    pub available: bool,
    pub path: String,
}

#[tauri::command]
pub fn check_cli() -> CliStatus {
    CliStatus {
        available: cli::is_available(),
        path: cli::get_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "not found".to_string()),
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

fn get_mount_status_sync() -> Result<MountInfo, String> {
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
    let stream = UnixStream::connect(SOCKET_PATH).ok()?;
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

    // Check for krun process (the VM runtime) - use exact match to avoid false positives
    if let Ok(output) = Command::new("pgrep").args(["-x", "krun"]).output() {
        if output.status.success() && !output.stdout.is_empty() {
            return true;
        }
    }

    // Also check for libkrun processes
    if let Ok(output) = Command::new("pgrep").args(["-f", "libkrun"]).output() {
        if output.status.success() && !output.stdout.is_empty() {
            return true;
        }
    }

    false
}

fn check_mount_point_fallback() -> Option<MountInfo> {
    // Check for localhost NFS mounts (anylinuxfs signature)
    // Run: mount | grep "localhost:/mnt"
    let output = Command::new("mount")
        .output()
        .ok()?;

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
