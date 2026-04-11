use serde::{Deserialize, Serialize};
use std::process::Command;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::time::timeout;
use crate::cache;
use crate::cli::execute_command;
use crate::paths::{COMMAND_TIMEOUT_SECS, MOUNT_TIMEOUT_SECS};

/// Validate device path to prevent command injection
/// Device must start with /dev/, raid:, or lvm: and contain only safe characters
fn validate_device_path(device: &str) -> Result<(), String> {
    if device.is_empty() {
        return Err("Device path is required".to_string());
    }
    // Prevent path traversal
    if device.contains("..") {
        return Err("Device path cannot contain '..'".to_string());
    }
    if device.starts_with("/dev/") {
        // Normal device: only allow alphanumeric, dash, underscore after /dev/ prefix
        let suffix = &device["/dev/".len()..];
        let valid_chars = suffix.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '-' || c == '_'
        });
        if suffix.is_empty() || !valid_chars {
            return Err("Device path contains invalid characters".to_string());
        }
    } else if device.starts_with("raid:") || device.starts_with("lvm:") {
        // RAID/LVM: allow alphanumeric, colon, dash, underscore
        let valid_chars = device.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == ':' || c == '-' || c == '_'
        });
        if !valid_chars {
            return Err("Device path contains invalid characters".to_string());
        }
    } else {
        return Err("Device path must start with /dev/, raid:, or lvm:".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DiskType {
    Normal,
    Raid,
    Lvm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    pub device: String,
    pub size: String,
    pub filesystem: String,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub encrypted: bool,
    pub mounted_by_system: bool,
    pub system_mount_point: Option<String>,
    pub supported: bool,
    pub support_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disk {
    pub device: String,
    pub size: String,
    pub model: Option<String>,
    pub is_external: bool,
    pub disk_type: DiskType,
    pub partitions: Vec<Partition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskListResult {
    pub disks: Vec<Disk>,
    pub has_supported_partitions: bool,
    pub used_admin_mode: bool,
}

#[tauri::command]
pub async fn list_disks(use_sudo: bool, silent: bool) -> Result<DiskListResult, String> {
    // Run in blocking task with timeout to avoid freezing UI
    let list_future = tokio::task::spawn_blocking(move || {
        // Run list command (now shows all volumes by default, including broken SD cards)
        let output = execute_command(&["list"], use_sudo, None, silent)?;
        let mut result = parse_disk_list_output(&output)?;

        // Check which partitions are already mounted by the system
        update_mount_status(&mut result);

        // Check filesystem support using diskutil
        update_filesystem_support(&mut result);

        // Check if there are any supported, mountable partitions
        result.has_supported_partitions = result.disks.iter().any(|d| {
            d.partitions.iter().any(|p| p.supported && !p.mounted_by_system)
        });
        result.used_admin_mode = use_sudo;

        Ok(result)
    });

    timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), list_future)
        .await
        .map_err(|_| format!("List disks timed out after {} seconds", COMMAND_TIMEOUT_SECS))?
        .map_err(|e| format!("Task error: {}", e))?
}

fn update_mount_status(result: &mut DiskListResult) {
    // Get current mounts
    let mounts = get_system_mounts();

    for disk in &mut result.disks {
        for partition in &mut disk.partitions {
            // Check if this partition is mounted
            // The device might be /dev/disk6s1 but mount shows it without /dev/
            let device_short = partition.device.trim_start_matches("/dev/");

            for (mount_device, mount_point) in &mounts {
                if mount_device.ends_with(device_short) || mount_device == &partition.device {
                    partition.mounted_by_system = true;
                    partition.system_mount_point = Some(mount_point.clone());
                    break;
                }
            }
        }
    }
}

fn get_system_mounts() -> Vec<(String, String)> {
    let mut mounts = Vec::new();

    // Use cached mount output to avoid redundant process spawning
    if let Some(output) = cache::get_mount_output() {
        let mount_output = String::from_utf8_lossy(&output.stdout);

        for line in mount_output.lines() {
            // Format: /dev/disk6s1 on /Volumes/NO NAME (msdos, ...)
            let parts: Vec<&str> = line.split(" on ").collect();
            if parts.len() >= 2 {
                let device = parts[0].to_string();
                // Extract mount point (everything before the parenthesis)
                let rest = parts[1..].join(" on ");
                if let Some(paren_pos) = rest.find(" (") {
                    let mount_point = rest[..paren_pos].to_string();
                    mounts.push((device, mount_point));
                }
            }
        }
    }

    mounts
}

fn update_filesystem_support(result: &mut DiskListResult) {
    // Run a single diskutil info -all call and parse the combined output
    let diskutil_info = get_all_diskutil_info();

    for disk in &mut result.disks {
        for partition in &mut disk.partitions {
            // Look up diskutil entry for UUID (applies to all partition types)
            let device_id = partition.device.trim_start_matches("/dev/");
            let entry = diskutil_info.get(device_id);

            // Set UUID from diskutil if available
            if let Some(e) = entry {
                partition.uuid = e.uuid.clone();
            }

            // For RAID/LVM partitions, use filesystem info from list output directly
            if disk.disk_type != DiskType::Normal {
                let (supported, note) = check_filesystem_support(&partition.filesystem);
                partition.supported = supported;
                partition.support_note = note;
                continue;
            }

            // If anylinuxfs detected a known Linux-native filesystem type, use that directly
            if is_linux_native_fs(&partition.filesystem) {
                let (supported, note) = check_filesystem_support(&partition.filesystem);
                partition.supported = supported;
                partition.support_note = note;
                continue;
            }

            // Look up diskutil results if available
            if let Some(e) = entry {
                if let Some(ref fs_personality) = e.fs_personality {
                    let (supported, note) = check_filesystem_support(fs_personality);
                    if !fs_personality.is_empty() && !is_linux_native_fs(&partition.filesystem) {
                        partition.filesystem = fs_personality.clone();
                    }
                    partition.supported = supported;
                    partition.support_note = note;
                } else {
                    let (supported, note) = check_filesystem_support(&partition.filesystem);
                    partition.supported = supported;
                    partition.support_note = note;
                }
            } else {
                let (supported, note) = check_filesystem_support(&partition.filesystem);
                partition.supported = supported;
                partition.support_note = note;
            }
        }
    }
}

struct DiskutilEntry {
    fs_personality: Option<String>,
    uuid: Option<String>,
}

fn get_all_diskutil_info() -> std::collections::HashMap<String, DiskutilEntry> {
    use std::collections::HashMap;

    let mut map = HashMap::new();
    let output = match Command::new("diskutil").args(["info", "-all"]).output() {
        Ok(o) => o,
        Err(_) => return map,
    };
    let text = String::from_utf8_lossy(&output.stdout);
    for block in text.split("**********") {
        let mut device_id = None;
        let mut fs_personality = None;
        let mut uuid = None;
        for line in block.lines() {
            if line.contains("Device Identifier:") {
                device_id = line.split(':').nth(1).map(|s| s.trim().to_string());
            } else if line.contains("File System Personality:") {
                fs_personality = line.split(':').nth(1).map(|s| s.trim().to_string());
            } else if line.contains("Disk / Partition UUID:") {
                uuid = line.split(':').nth(1).map(|s| s.trim().to_string());
            }
        }
        if let Some(id) = device_id {
            map.insert(id, DiskutilEntry { fs_personality, uuid });
        }
    }
    map
}

fn is_linux_native_fs(fs: &str) -> bool {
    let fs_lower = fs.to_lowercase();
    fs_lower.contains("ext4") || fs_lower.contains("ext3") || fs_lower.contains("ext2")
        || fs_lower.contains("btrfs") || fs_lower.contains("xfs") || fs_lower.contains("f2fs")
        || fs_lower.contains("reiserfs") || fs_lower.contains("zfs")
        || fs_lower.contains("ntfs") || fs_lower.contains("exfat")
        || fs_lower.contains("luks")
        || fs_lower.contains("lvm") || fs_lower.contains("raid")
        || fs_lower == "linux filesystem"
}

fn check_filesystem_support(fs: &str) -> (bool, Option<String>) {
    let fs_lower = fs.to_lowercase();

    // Fully supported Linux-native filesystems
    if fs_lower.contains("ext4") || fs_lower.contains("ext3") || fs_lower.contains("ext2")
        || fs_lower.contains("btrfs") || fs_lower.contains("xfs") || fs_lower.contains("f2fs")
        || fs_lower.contains("reiserfs")
    {
        return (true, None);
    }

    // LUKS encrypted partitions — supported, passphrase will be requested at mount time
    if fs_lower.contains("luks") {
        return (true, Some("Encrypted (passphrase required)".to_string()));
    }

    // RAID/LVM member partitions — not directly mountable, use admin mode for actual volumes
    if fs_lower.contains("raid") {
        return (false, Some("RAID member (use admin mode for volumes)".to_string()));
    }
    if fs_lower.contains("lvm") {
        return (false, Some("LVM member (use admin mode for volumes)".to_string()));
    }

    // Generic "Linux Filesystem" from GPT partition type (native anylinuxfs detection)
    if fs_lower == "linux filesystem" {
        return (true, Some("Linux partition (use admin mode for exact fs type)".to_string()));
    }

    // FAT filesystems - well supported
    if fs_lower.contains("fat32") || fs_lower.contains("fat16") || fs_lower.contains("exfat") {
        return (true, None);
    }

    // NTFS - supported via ntfs-3g
    if fs_lower.contains("ntfs") {
        return (true, Some("NTFS via ntfs-3g".to_string()));
    }

    // Generic MS-DOS without FAT32/FAT16 specification - might be problematic
    if fs_lower == "ms-dos" {
        return (false, Some("Unknown FAT variant - may not mount".to_string()));
    }

    // Apple filesystems - not supported
    if fs_lower.contains("apfs") {
        return (false, Some("APFS not supported by Linux".to_string()));
    }
    if fs_lower.contains("hfs") || fs_lower.contains("mac os") {
        return (false, Some("HFS/HFS+ has limited Linux support".to_string()));
    }

    // Unknown filesystem
    if fs.is_empty() || fs_lower == "unknown" {
        return (false, Some("Unknown filesystem".to_string()));
    }

    // Default: assume supported but note it's unverified
    (true, Some(format!("Unverified: {}", fs)))
}

/// Extract model and is_external from parenthesized info in a disk header line
fn extract_parenthesized_info(line: &str) -> (Option<String>, bool) {
    if let Some(start) = line.find('(') {
        if let Some(end) = line.find(')') {
            let info = line[start+1..end].to_string();
            let external = info.to_lowercase().contains("external");
            return (Some(info), external);
        }
    }
    (None, false)
}

fn parse_disk_list_output(output: &str) -> Result<DiskListResult, String> {
    let mut disks: Vec<Disk> = Vec::new();
    let mut current_disk: Option<Disk> = None;

    for line in output.lines() {
        // Detect disk header type
        let header = if line.starts_with("/dev/") {
            let device = line.split_whitespace().next().unwrap_or("").to_string();
            let (model, is_external) = extract_parenthesized_info(line);
            Some((device, model, is_external, DiskType::Normal))
        } else if line.starts_with("raid:") {
            let device = line.split_whitespace().next().unwrap_or("")
                .trim_end_matches(':').to_string();
            Some((device, Some("Autodetected RAID volume".to_string()), false, DiskType::Raid))
        } else if line.starts_with("lvm:") {
            let device = line.split_whitespace().next().unwrap_or("")
                .trim_end_matches(':').to_string();
            Some((device, Some("Autodetected LVM volume group".to_string()), false, DiskType::Lvm))
        } else {
            None
        };

        if let Some((device, model, is_external, disk_type)) = header {
            // Save previous disk if any
            if let Some(disk) = current_disk.take() {
                if !disk.partitions.is_empty() {
                    disks.push(disk);
                }
            }

            current_disk = Some(Disk {
                device,
                size: String::new(), // Will be set from partition 0
                model,
                is_external,
                disk_type,
                partitions: Vec::new(),
            });
        } else if line.trim().starts_with("#:") {
            // Skip header line
            continue;
        } else if let Some(ref mut disk) = current_disk {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // For RAID/LVM, skip non-partition continuation lines
            // (Physical Store lines, bare disk member lines, etc.)
            if disk.disk_type != DiskType::Normal {
                if let Some(colon_pos) = trimmed.find(':') {
                    let num_part = &trimmed[..colon_pos];
                    if !num_part.chars().all(|c| c.is_ascii_digit()) {
                        continue; // Not a partition line — skip
                    }
                } else {
                    continue; // No colon — skip
                }
            }

            // Check if line starts with a number followed by colon
            if let Some(colon_pos) = trimmed.find(':') {
                let num_part = &trimmed[..colon_pos];
                if num_part.chars().all(|c| c.is_ascii_digit()) {
                    let partition_num: u32 = num_part.parse().unwrap_or(0);
                    let rest = trimmed[colon_pos+1..].trim();

                    if let Some(partition) = parse_partition_line(rest, &disk.device, partition_num, &disk.disk_type) {
                        if partition_num == 0 {
                            // Partition 0: always use its size for the disk
                            disk.size = partition.size.clone();
                            // For RAID, partition 0 IS the mountable volume
                            // For LVM, partition 0 is the VG scheme (not mountable)
                            if disk.disk_type == DiskType::Raid {
                                disk.partitions.push(partition);
                            }
                        } else {
                            disk.partitions.push(partition);
                        }
                    }
                }
            }
        }
    }

    // Don't forget the last disk
    if let Some(disk) = current_disk {
        if !disk.partitions.is_empty() {
            disks.push(disk);
        }
    }

    Ok(DiskListResult {
        disks,
        has_supported_partitions: false, // Will be updated after filesystem check
        used_admin_mode: false,          // Will be updated by caller
    })
}

fn parse_partition_line(line: &str, _disk_device: &str, _partition_num: u32, disk_type: &DiskType) -> Option<Partition> {
    // Format: "Microsoft Basic Data NO NAME                 47.2 GB    disk6s1"
    // Or:     "ext4 linuxrootfs             7.5 GB     disk6s5"
    // The identifier is always at the end, size is before it

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }

    // Last part is the identifier (e.g., disk6s1)
    let identifier = parts.last()?;

    // Second to last and third to last form the size (e.g., "47.2 GB" or "*62.5 GB" or "+62.5 GB")
    let size_unit = parts.get(parts.len() - 2)?;
    let size_num = parts.get(parts.len() - 3)?;
    let size = format!("{} {}", size_num.trim_start_matches('*').trim_start_matches('+'), size_unit);

    // Everything before the size is TYPE and NAME
    // TYPE is known keywords, NAME is the rest
    let type_and_name: Vec<&str> = parts[..parts.len()-3].to_vec();

    // Determine filesystem type and label
    let (filesystem, label) = parse_type_and_name(&type_and_name);

    // Check for encryption markers
    let encrypted = filesystem.to_lowercase().contains("luks")
        || filesystem.to_lowercase().contains("bitlocker")
        || line.to_lowercase().contains("encrypted");

    // Build the device path based on disk type
    let device = match disk_type {
        DiskType::Normal => format!("/dev/{}", identifier),
        DiskType::Raid => format!("raid:{}", identifier),
        DiskType::Lvm => format!("lvm:{}", identifier),
    };

    Some(Partition {
        device,
        size,
        filesystem,
        label,
        uuid: None,  // Will be populated from diskutil info
        encrypted,
        mounted_by_system: false,  // Will be updated after parsing
        system_mount_point: None,
        supported: true,  // Will be updated after parsing
        support_note: None,
    })
}

fn parse_type_and_name(parts: &[&str]) -> (String, Option<String>) {
    // Known filesystem types that may have multiple words
    let multi_word_types = [
        "Microsoft Basic Data",
        "Microsoft Reserved",
        "EFI System",
        "Apple APFS",
        "Apple HFS",
        "Linux Filesystem",
        "Linux LVM",
        "Linux RAID",
        "GUID_partition_scheme",
    ];

    let joined = parts.join(" ");

    // Check for multi-word types
    for type_name in &multi_word_types {
        if joined.starts_with(type_name) {
            let label_part = joined[type_name.len()..].trim();
            let label = if label_part.is_empty() { None } else { Some(label_part.to_string()) };
            return (type_name.to_string(), label);
        }
    }

    // Single word filesystem type
    if let Some(first) = parts.first() {
        let label_parts: Vec<&str> = parts[1..].to_vec();
        let label = if label_parts.is_empty() {
            None
        } else {
            Some(label_parts.join(" "))
        };
        return (first.to_string(), label);
    }

    ("unknown".to_string(), None)
}

#[tauri::command]
pub async fn mount_disk(app: AppHandle, device: String, passphrase: Option<String>, read_only: Option<bool>, extra_options: Option<String>) -> Result<String, String> {
    // Validate device path before use
    validate_device_path(&device)?;

    // Sanitize extra_options with a whitelist to prevent command injection
    if let Some(ref opts) = extra_options {
        let valid = opts.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, ',' | '.' | '_' | '-' | '=' | '/' | ':')
        });
        if !valid {
            return Err("Mount options contain invalid characters".to_string());
        }
    }

    // Build combined mount options string
    let ro = read_only.unwrap_or(false);
    let mut opts = Vec::new();
    if ro {
        opts.push("ro".to_string());
    }
    if let Some(ref extra) = extra_options {
        let trimmed = extra.trim();
        if !trimmed.is_empty() {
            opts.push(trimmed.to_string());
        }
    }
    let combined_options = if opts.is_empty() { None } else { Some(opts.join(",")) };

    // Spawn the mount command in a background thread so we can poll status
    // concurrently — the mount appears in Finder before the command exits
    let mount_device = device.clone();
    let mount_result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let mount_result_bg = mount_result.clone();

    let _mount_thread = tokio::task::spawn_blocking(move || {
        let effective_passphrase = passphrase.unwrap_or_else(|| "##PROBE##".to_string());
        let pass_ref = Some(effective_passphrase.as_str());

        let result = {
            let mut args: Vec<&str> = vec!["mount"];
            if let Some(ref combined) = combined_options {
                args.extend_from_slice(&["-o", combined]);
            }
            args.push(&mount_device);
            execute_command(&args, true, pass_ref, false)
        };

        *mount_result_bg.lock().unwrap() = Some(result);
    });

    // Helper to detect encryption-related output
    let is_encryption_error = |text: &str| -> bool {
        let lower = text.to_lowercase();
        lower.contains("luks") || lower.contains("decrypt")
            || lower.contains("passphrase") || lower.contains("password")
            || lower.contains("encrypted") || lower.contains("wrong key")
    };

    // Poll `anylinuxfs status` concurrently while mount command runs
    // 120 retries × 500ms = 60 seconds total timeout
    for i in 0..120 {
        if i > 0 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Check if mount command finished with an error
        if let Some(ref result) = *mount_result.lock().unwrap() {
            let output_text = match result {
                Ok(out) => out.clone(),
                Err(e) => e.clone(),
            };
            if is_encryption_error(&output_text) {
                // Clean up leftover VM from the failed probe attempt
                let _ = execute_command(&["stop"], true, None, false);
                let _ = app.emit("status-changed", ());
                return Err("ENCRYPTION_REQUIRED: This partition is encrypted. A passphrase is needed to mount it.".to_string());
            }
            if result.is_err() {
                let _ = app.emit("status-changed", ());
                return Err(result.as_ref().unwrap_err().clone());
            }
        }

        // Check if this specific device appeared in `anylinuxfs status`
        if check_device_mounted(&device) {
            let _ = app.emit("status-changed", ());
            return Ok("Mounted successfully".to_string());
        }
    }

    let _ = app.emit("status-changed", ());
    Err(format!("Mount operation timed out after {} seconds", MOUNT_TIMEOUT_SECS))
}

fn check_device_mounted(device: &str) -> bool {
    crate::cli::get_status()
        .map(|s| s.lines().any(|line| line.starts_with(device)))
        .unwrap_or(false)
}

#[tauri::command]
pub async fn unmount_disk(app: AppHandle, device: Option<String>) -> Result<String, String> {
    // Validate device path if provided
    if let Some(ref dev) = device {
        validate_device_path(dev)?;
    }

    // Run in blocking task with timeout
    let unmount_future = tokio::task::spawn_blocking(move || {
        match device {
            Some(ref dev) => execute_command(&["unmount", dev], false, None, false),
            None => execute_command(&["unmount"], false, None, false),
        }
    });

    let result = timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), unmount_future)
        .await
        .map_err(|_| format!("Unmount timed out after {} seconds", COMMAND_TIMEOUT_SECS))?
        .map_err(|e| format!("Task error: {}", e))?;

    // Invalidate caches after unmount
    cache::invalidate_all();

    // Emit status changed event
    let _ = app.emit("status-changed", ());

    result
}


#[tauri::command]
pub async fn eject_disk(device: String) -> Result<String, String> {
    // Validate device path before use
    validate_device_path(&device)?;

    // Eject (power down) a disk using diskutil
    // First unmount anylinuxfs if it has anything mounted, then eject
    let eject_future = tokio::task::spawn_blocking(move || {
        // Check if this device is mounted by anylinuxfs and unmount it first
        if check_device_mounted(&device) {
            let _ = execute_command(&["unmount", &device], false, None, false);

            // Wait for this device to be unmounted (up to 5 seconds)
            for _ in 0..10 {
                thread::sleep(Duration::from_millis(500));
                if !check_device_mounted(&device) {
                    break;
                }
            }
        }

        // Now safe to eject the disk
        let output = Command::new("diskutil")
            .args(["eject", &device])
            .output()
            .map_err(|e| format!("Failed to run diskutil: {}", e))?;

        if output.status.success() {
            Ok(format!("Ejected {}", device))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Failed to eject: {}", stderr))
        }
    });

    timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), eject_future)
        .await
        .map_err(|_| format!("Eject timed out after {} seconds", COMMAND_TIMEOUT_SECS))?
        .map_err(|e| format!("Task error: {}", e))?
}

#[tauri::command]
pub async fn force_cleanup() -> Result<String, String> {
    // Use `anylinuxfs stop` to cleanly stop all instances
    tokio::task::spawn_blocking(|| {
        execute_command(&["stop"], false, None, false)
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
