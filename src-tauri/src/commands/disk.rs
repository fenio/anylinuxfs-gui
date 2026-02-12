use serde::{Deserialize, Serialize};
use std::process::Command;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::time::timeout;
use crate::cache;
use crate::cli::execute_command;
use crate::paths::{get_socket_path, COMMAND_TIMEOUT_SECS, MOUNT_TIMEOUT_SECS};

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
        // Normal device: allow alphanumeric, slash, dash, underscore
        let valid_chars = device.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '/' || c == '-' || c == '_'
        });
        if !valid_chars {
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
pub async fn list_disks(use_sudo: bool) -> Result<DiskListResult, String> {
    // Run in blocking task with timeout to avoid freezing UI
    let list_future = tokio::task::spawn_blocking(move || {
        // Run list command (now shows all volumes by default, including broken SD cards)
        let output = execute_command(&["list"], use_sudo, None)?;
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
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    // Collect partitions that need diskutil info (skip RAID/LVM — diskutil won't know about them)
    let mut needs_diskutil: Vec<String> = Vec::new();

    for disk in &result.disks {
        if disk.disk_type != DiskType::Normal {
            continue; // Skip virtual volumes — diskutil won't know about them
        }
        for partition in &disk.partitions {
            let (supported, _) = check_filesystem_support(&partition.filesystem);
            // Skip if anylinuxfs already detected a supported Linux-native filesystem
            if !(supported && is_linux_native_fs(&partition.filesystem)) {
                let device_id = partition.device.trim_start_matches("/dev/").to_string();
                needs_diskutil.push(device_id);
            }
        }
    }

    // Run diskutil info calls in parallel with limited concurrency
    // Limit to 8 concurrent threads to avoid resource exhaustion on systems with many partitions
    const MAX_DISKUTIL_THREADS: usize = 8;
    let diskutil_results: Arc<Mutex<HashMap<String, (String, bool, Option<String>)>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Process in batches to limit concurrency
    for chunk in needs_diskutil.chunks(MAX_DISKUTIL_THREADS) {
        std::thread::scope(|s| {
            for device_id in chunk {
                let results = Arc::clone(&diskutil_results);
                let device = device_id.clone();
                s.spawn(move || {
                    if let Some(info) = get_diskutil_fs_info(&device) {
                        // Handle mutex lock failure gracefully
                        if let Ok(mut guard) = results.lock() {
                            guard.insert(device, info);
                        }
                    }
                });
            }
        });
    }

    // Extract results from Arc<Mutex<...>> safely
    let diskutil_map = match Arc::try_unwrap(diskutil_results) {
        Ok(mutex) => mutex.into_inner().unwrap_or_default(),
        Err(arc) => {
            // If other references exist (shouldn't happen), clone the inner data
            arc.lock().map(|g| g.clone()).unwrap_or_default()
        }
    };

    // Apply results to partitions
    for disk in &mut result.disks {
        for partition in &mut disk.partitions {
            let (supported, note) = check_filesystem_support(&partition.filesystem);

            // For RAID/LVM partitions, use filesystem info from list output directly
            if disk.disk_type != DiskType::Normal {
                partition.supported = supported;
                partition.support_note = note;
                continue;
            }

            // If anylinuxfs detected a known supported filesystem, use that
            if supported && is_linux_native_fs(&partition.filesystem) {
                partition.supported = true;
                partition.support_note = note;
                continue;
            }

            // Apply diskutil results if available
            let device_id = partition.device.trim_start_matches("/dev/");
            if let Some((fs_personality, diskutil_supported, diskutil_note)) = diskutil_map.get(device_id) {
                if !fs_personality.is_empty() && !is_linux_native_fs(&partition.filesystem) {
                    partition.filesystem = fs_personality.clone();
                }
                partition.supported = *diskutil_supported;
                partition.support_note = diskutil_note.clone();
            } else {
                partition.supported = supported;
                partition.support_note = note;
            }
        }
    }
}

fn is_linux_native_fs(fs: &str) -> bool {
    let fs_lower = fs.to_lowercase();
    fs_lower.contains("ext4") || fs_lower.contains("ext3") || fs_lower.contains("ext2")
        || fs_lower.contains("btrfs") || fs_lower.contains("xfs") || fs_lower.contains("f2fs")
        || fs_lower.contains("reiserfs") || fs_lower.contains("zfs")
        || fs_lower.contains("ntfs") || fs_lower.contains("exfat")
        || fs_lower == "linux filesystem"
}

fn get_diskutil_fs_info(device_id: &str) -> Option<(String, bool, Option<String>)> {
    let output = Command::new("diskutil")
        .args(["info", device_id])
        .output()
        .ok()?;

    let info = String::from_utf8_lossy(&output.stdout);

    let mut fs_personality = String::new();

    for line in info.lines() {
        if line.contains("File System Personality:") {
            fs_personality = line.split(':').nth(1)?.trim().to_string();
            break;
        }
    }

    // Determine support based on filesystem personality
    let (supported, note) = check_filesystem_support(&fs_personality);

    Some((fs_personality, supported, note))
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
            let (model, _) = extract_parenthesized_info(line);
            Some((device, model, false, DiskType::Raid))
        } else if line.starts_with("lvm:") {
            let device = line.split_whitespace().next().unwrap_or("")
                .trim_end_matches(':').to_string();
            let (model, _) = extract_parenthesized_info(line);
            Some((device, model, false, DiskType::Lvm))
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
                            // For RAID/LVM, partition 0 IS the mountable volume
                            if disk.disk_type != DiskType::Normal {
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
pub async fn mount_disk(app: AppHandle, device: String, passphrase: Option<String>) -> Result<String, String> {
    // Validate device path before use
    validate_device_path(&device)?;

    // Run in blocking task with timeout to avoid freezing UI
    let mount_future = tokio::task::spawn_blocking(move || {
        let pass_ref = passphrase.as_deref();
        // RAID/LVM: the device identifier IS the argument (e.g., anylinuxfs raid:disk10s1:disk11s1)
        // Normal: use explicit mount subcommand
        let result = if device.starts_with("raid:") || device.starts_with("lvm:") {
            execute_command(&[&device], true, pass_ref)
        } else {
            execute_command(&["mount", &device], true, pass_ref)
        };

        // Check immediately first, then retry with short intervals
        // 40 retries × 250ms = 10 seconds total timeout
        for i in 0..40 {
            // Invalidate cache to get fresh mount data
            cache::invalidate_mount_cache();
            if check_nfs_mount_exists() {
                return Ok(result.unwrap_or_else(|_| "Mounted successfully".to_string()));
            }
            // Don't sleep on first iteration - check immediately
            if i > 0 {
                thread::sleep(Duration::from_millis(250));
            }
        }

        // Mount verification failed - return error with details
        match result {
            Ok(output) => {
                // CLI succeeded but mount not visible - likely filesystem error
                if output.contains("wrong fs type") || output.contains("mount:") {
                    Err(format!("Mount failed: {}", output))
                } else {
                    Err("Mount failed: filesystem not mounted after timeout".to_string())
                }
            }
            Err(e) => Err(e),
        }
    });

    // Apply overall timeout
    let result = timeout(Duration::from_secs(MOUNT_TIMEOUT_SECS), mount_future)
        .await
        .map_err(|_| format!("Mount operation timed out after {} seconds", MOUNT_TIMEOUT_SECS))?
        .map_err(|e| format!("Task error: {}", e))?;

    // Emit status changed event regardless of success/failure
    let _ = app.emit("status-changed", ());

    result
}

fn check_nfs_mount_exists() -> bool {
    // Use cached mount output to avoid redundant process spawning
    if let Some(output) = cache::get_mount_output() {
        let mount_output = String::from_utf8_lossy(&output.stdout);
        // Look for anylinuxfs NFS mount pattern
        mount_output.contains("localhost:/mnt/") && mount_output.contains("/Volumes/")
    } else {
        false
    }
}

#[tauri::command]
pub async fn unmount_disk(app: AppHandle) -> Result<String, String> {
    // Run in blocking task with timeout
    let unmount_future = tokio::task::spawn_blocking(|| {
        execute_command(&["unmount"], false, None)
    });

    let result = timeout(Duration::from_secs(COMMAND_TIMEOUT_SECS), unmount_future)
        .await
        .map_err(|_| format!("Unmount timed out after {} seconds", COMMAND_TIMEOUT_SECS))?
        .map_err(|e| format!("Task error: {}", e))?;

    // Wait for VM to fully shut down by polling until krun process is gone
    // 40 retries × 250ms = 10 seconds max wait
    for _ in 0..40 {
        if !is_vm_running() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    // Invalidate all caches after unmount
    cache::invalidate_all();

    // Emit status changed event
    let _ = app.emit("status-changed", ());

    result
}

/// Check if the VM (krun) process is running
fn is_vm_running() -> bool {
    // Use cached pgrep result, but invalidate first since we're polling for shutdown
    cache::invalidate_process_cache();
    cache::is_krun_running()
}

#[tauri::command]
pub async fn eject_disk(device: String) -> Result<String, String> {
    // Validate device path before use
    validate_device_path(&device)?;

    // Eject (power down) a disk using diskutil
    // First unmount anylinuxfs if it has anything mounted, then eject
    let eject_future = tokio::task::spawn_blocking(move || {
        // Check if anylinuxfs has anything mounted and unmount first
        if check_nfs_mount_exists() {
            // Unmount anylinuxfs first - this shuts down the VM properly
            let _ = execute_command(&["unmount"], false, None);

            // Wait for anylinuxfs to fully stop (up to 5 seconds)
            for _ in 0..10 {
                thread::sleep(Duration::from_millis(500));
                // Invalidate cache to get fresh mount data
                cache::invalidate_mount_cache();
                if !check_nfs_mount_exists() {
                    break;
                }
            }
            // Invalidate all caches after unmount
            cache::invalidate_all();
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
    // Force kill orphaned anylinuxfs/krun processes
    tokio::task::spawn_blocking(|| {
        let mut killed = Vec::new();

        // Kill krun processes
        if let Ok(output) = Command::new("pkill").args(["-9", "krun"]).output() {
            if output.status.success() {
                killed.push("krun");
            }
        }

        // Kill any anylinuxfs processes
        if let Ok(output) = Command::new("pkill").args(["-9", "-f", "anylinuxfs"]).output() {
            if output.status.success() {
                killed.push("anylinuxfs");
            }
        }

        // Remove socket file if it exists
        let socket_path = get_socket_path();
        if socket_path.exists() {
            if std::fs::remove_file(&socket_path).is_ok() {
                killed.push("socket");
            }
        }

        if killed.is_empty() {
            Ok("No processes found to clean up".to_string())
        } else {
            Ok(format!("Cleaned up: {}", killed.join(", ")))
        }
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
