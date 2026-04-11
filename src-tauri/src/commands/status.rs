use serde::{Deserialize, Serialize};
use crate::cli;

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
    pub device: String,
    pub mount_point: String,
    pub filesystem: Option<String>,
    pub ram_mb: Option<u32>,
    pub vcpus: Option<u32>,
}

#[tauri::command]
pub async fn get_mount_status() -> Result<Vec<MountInfo>, String> {
    tokio::task::spawn_blocking(|| get_mount_status_sync())
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

pub fn get_mount_status_sync() -> Result<Vec<MountInfo>, String> {
    if let Some(status) = cli::get_status() {
        if !status.is_empty() {
            let mounts: Vec<MountInfo> = status.lines()
                .filter_map(parse_status_line)
                .collect();
            return Ok(mounts);
        }
    }
    Ok(Vec::new())
}

/// Parse a line from `anylinuxfs status` output.
/// Format: "/dev/disk4s1 on /Volumes/ntfs-test (ntfs, uid=501, ...) VM[cpus: 1, ram: 512 MiB]"
fn parse_status_line(line: &str) -> Option<MountInfo> {
    // Split on " on " to get device and the rest
    let on_pos = line.find(" on ")?;
    let device = line[..on_pos].trim().to_string();
    let rest = &line[on_pos + 4..];

    // Mount point is everything before the first '('
    let paren_pos = rest.find('(')?;
    let mount_point = rest[..paren_pos].trim().to_string();

    // Filesystem is the first token inside parentheses
    let close_paren = rest.find(')')?;
    let paren_content = &rest[paren_pos + 1..close_paren];
    let filesystem = paren_content.split(',').next()
        .map(|s| s.trim().to_string());

    // Parse VM info: "VM[cpus: N, ram: N MiB]"
    let mut ram_mb = None;
    let mut vcpus = None;
    if let Some(vm_start) = line.find("VM[") {
        let vm_section = &line[vm_start..];
        if let Some(cpu_start) = vm_section.find("cpus: ") {
            let after = &vm_section[cpu_start + 6..];
            vcpus = after.split(|c: char| !c.is_ascii_digit()).next()
                .and_then(|s| s.parse().ok());
        }
        if let Some(ram_start) = vm_section.find("ram: ") {
            let after = &vm_section[ram_start + 5..];
            ram_mb = after.split(|c: char| !c.is_ascii_digit()).next()
                .and_then(|s| s.parse().ok());
        }
    }

    Some(MountInfo {
        device,
        mount_point,
        filesystem,
        ram_mb,
        vcpus,
    })
}
