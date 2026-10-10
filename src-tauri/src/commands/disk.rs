use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;
use tokio::time::timeout;
use crate::cache;
use crate::cli::{
    execute_command, execute_command_with_elevation, is_unlock_failure, CommandExecutionError,
};
use crate::elevation::{
    ElevationMode, ElevationState, TerminalInteraction,
    INTERACTIVE_ELEVATION_TIMEOUT_SECS,
};
use crate::paths::{COMMAND_TIMEOUT_SECS, MOUNT_TIMEOUT_SECS};

/// Validate device path to prevent command injection
/// Device must start with /dev/, raid:, or lvm: and contain only safe characters
pub(super) fn validate_device_path(device: &str) -> Result<(), String> {
    if device.is_empty() {
        return Err("Device path is required".to_string());
    }
    // Prevent path traversal
    if device.contains("..") {
        return Err("Device path cannot contain '..'".to_string());
    }
    if let Some(suffix) = device.strip_prefix("/dev/") {
        // Normal device: only allow alphanumeric, dash, underscore after /dev/ prefix
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
pub async fn list_disks(
    elevation_state: tauri::State<'_, Arc<ElevationState>>,
    use_sudo: bool,
    silent: bool,
) -> Result<DiskListResult, String> {
    let elevation_state = elevation_state.inner().clone();
    let operation_guard = elevation_state.begin_operation("list")?;
    let elevation_mode = operation_guard.mode();
    let timeout_secs = if use_sudo && elevation_mode == ElevationMode::InteractiveTerminal {
        INTERACTIVE_ELEVATION_TIMEOUT_SECS
    } else {
        COMMAND_TIMEOUT_SECS
    };
    let list_elevation_state = elevation_state.clone();

    // Run in blocking task with timeout to avoid freezing UI
    let list_future = tokio::task::spawn_blocking(move || {
        // Run list command (now shows all volumes by default, including broken SD cards)
        let output = execute_command_with_elevation(
            &["list"],
            use_sudo,
            None,
            silent,
            elevation_mode,
            &list_elevation_state,
            TerminalInteraction::CaptureOutput {
                operation: "list".to_string(),
            },
        )
        .map_err(|error| error.message())?;
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

    match timeout(Duration::from_secs(timeout_secs), list_future).await {
        Ok(result) => result
            .map_err(|error| format!("Task error: {}", error))?,
        Err(_) => {
            elevation_state.cancel_pending_operation("list");
            Err(format!("List disks timed out after {} seconds", timeout_secs))
        }
    }
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
                if mount_device.trim_start_matches("/dev/") == device_short {
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
        || fs_lower.contains("luks") || fs_lower.contains("bitlocker")
        || fs_lower.contains("lvm") || fs_lower.contains("raid")
        || fs_lower == "linux filesystem"
}

fn check_filesystem_support(fs: &str) -> (bool, Option<String>) {
    let fs_lower = fs.to_lowercase();

    // Known mountable types; actual features depend on the installed CLI/VM.
    if fs_lower.contains("ext4") || fs_lower.contains("ext3") || fs_lower.contains("ext2")
        || fs_lower.contains("btrfs") || fs_lower.contains("xfs") || fs_lower.contains("f2fs")
        || fs_lower.contains("reiserfs")
    {
        return (true, None);
    }

    if fs_lower == "zfs" || fs_lower == "zfs_member" {
        return (true, Some("ZFS pool (import and dataset mounting handled by anylinuxfs)".to_string()));
    }

    // Encrypted partitions are supported; an unlock key is requested at mount time.
    if fs_lower.contains("luks") || fs_lower.contains("bitlocker") {
        return (true, Some("Encrypted (unlock key required)".to_string()));
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
                            } else if disk.disk_type == DiskType::Normal
                                && !partition.filesystem.to_lowercase().contains("partition_scheme")
                            {
                                // Whole-disk filesystem with no partition table: the
                                // index-0 entry IS the filesystem (e.g. whole-disk LUKS,
                                // a "superfloppy" ext4/FAT), not a scheme container like
                                // GUID_partition_scheme. Surface it as a mountable
                                // partition instead of dropping the disk for having an
                                // empty partition list. (issue #83)
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
        || filesystem.to_lowercase().contains("bitlocker");

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
        if let Some(rest) = joined.strip_prefix(*type_name) {
            let label_part = rest.trim();
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MountOutcome {
    Mounted,
    EncryptionRequired,
    Cancelled,
    TimedOut,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct MountCommandResult {
    pub outcome: MountOutcome,
    pub message: Option<String>,
}

impl MountCommandResult {
    fn new(outcome: MountOutcome, message: impl Into<Option<String>>) -> Self {
        Self {
            outcome,
            message: message.into(),
        }
    }
}

#[tauri::command]
pub async fn select_key_file(app: AppHandle) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || {
        app.dialog().file().set_title("Select encryption key file").blocking_pick_file()
            .map(|file| {
                let path = file.into_path().map_err(|_| "Select a local key file".to_string())?;
                path.to_str().map(str::to_owned)
                    .ok_or_else(|| "Key file path is not valid UTF-8".to_string())
            })
            .transpose()
    }).await.map_err(|e| format!("Unable to open file picker: {}", e))?
}

fn validate_unlock_credentials(passphrase: Option<&str>, key_file: Option<&str>) -> Result<(), String> {
    if let Some(file) = key_file {
        if passphrase.is_some() {
            return Err("Choose either a passphrase or a key file, not both".to_string());
        }
        let path = std::path::Path::new(file);
        if !path.is_absolute() || file.contains(['\0', '\n', '\r']) {
            return Err("Select a key file using an absolute local path".to_string());
        }
        if !path.is_file() {
            return Err("Key file does not exist or is not a regular file".to_string());
        }
        std::fs::File::open(path).map_err(|_| "Key file is not readable".to_string())?;
    }
    Ok(())
}

fn validate_mount_options(opts: &str) -> Result<(), String> {
    if opts.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, ',' | '.' | '_' | '-' | '=' | '/' | ':' | '@')
    }) {
        Ok(())
    } else {
        Err("Mount options contain invalid characters".to_string())
    }
}

fn mount_passphrase(passphrase: Option<String>, using_key_file: bool, mode: ElevationMode) -> Option<String> {
    if using_key_file || mode == ElevationMode::InteractiveTerminal {
        None
    } else {
        Some(passphrase.unwrap_or_else(|| "##PROBE##".to_string()))
    }
}

fn unlock_failure_outcome(
    result: &Result<String, CommandExecutionError>,
    supplied_credentials: bool,
    mode: ElevationMode,
) -> Option<MountOutcome> {
    match result {
        Err(CommandExecutionError::Failed(message)) if is_unlock_failure(message) => {
            Some(if supplied_credentials || mode == ElevationMode::InteractiveTerminal {
                MountOutcome::Failed
            } else {
                MountOutcome::EncryptionRequired
            })
        }
        _ => None,
    }
}

fn combined_mount_options(read_only: bool, extra: Option<&str>) -> Result<Option<String>, String> {
    let mut parts: Vec<_> = extra.unwrap_or("").split(',').map(str::trim).filter(|part| !part.is_empty()).collect();
    if (read_only || parts.contains(&"ro")) && parts.contains(&"rw") {
        return Err("Choose either read-only (ro) or read-write (rw), not both".into());
    }
    if read_only && !parts.contains(&"ro") { parts.insert(0, "ro"); }
    Ok((!parts.is_empty()).then(|| parts.join(",")))
}

#[tauri::command]
pub async fn mount_disk(
    app: AppHandle,
    elevation_state: tauri::State<'_, Arc<ElevationState>>,
    device: String,
    passphrase: Option<String>,
    key_file: Option<String>,
    read_only: Option<bool>,
    extra_options: Option<String>,
    ignore_permissions: Option<bool>,
) -> Result<MountCommandResult, String> {
    // Validate device path before use
    validate_device_path(&device)?;
    validate_unlock_credentials(passphrase.as_deref(), key_file.as_deref())?;
    let using_key_file = key_file.is_some();
    let supplied_credentials = passphrase.is_some() || using_key_file;
    if using_key_file {
        // Feature detection works for upstream and backported CLI releases.
        let help = tokio::task::spawn_blocking(|| execute_command(&["mount", "--help"], false, None, true))
            .await.map_err(|e| format!("Unable to check CLI support: {}", e))??;
        if !help.contains("--key-file") {
            return Err("The installed anylinuxfs CLI does not support key files. Update anylinuxfs and try again.".to_string());
        }
    }
    let elevation_state = elevation_state.inner().clone();
    let operation = format!("mount:{}", device);
    let operation_guard = elevation_state.begin_operation(operation.clone())?;
    let elevation_mode = operation_guard.mode();

    // Sanitize extra_options with a whitelist to prevent command injection
    if let Some(ref opts) = extra_options {
        validate_mount_options(opts)?;
    }

    // Build combined mount options string
    let ro = read_only.unwrap_or(false);
    let combined_options = combined_mount_options(ro, extra_options.as_deref())?;

    // Spawn the mount command in a background thread so we can poll status
    // concurrently — the mount appears in Finder before the command exits
    let mount_device = device.clone();
    let mount_operation = operation.clone();
    let mount_elevation_state = elevation_state.clone();
    let mount_result: std::sync::Arc<
        std::sync::Mutex<Option<Result<String, CommandExecutionError>>>,
    > =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let mount_result_bg = mount_result.clone();

    let _mount_thread = tokio::task::spawn_blocking(move || {
        // Interactive Terminal elevation prompts there. Never place a disk
        // passphrase in a generated command file or process environment.
        let effective_passphrase = mount_passphrase(passphrase, using_key_file, elevation_mode);
        let pass_ref = effective_passphrase.as_deref();

        let result = {
            let mut args: Vec<&str> = vec!["mount"];
            if let Some(ref file) = key_file {
                args.extend_from_slice(&["--key-file", file]);
            }
            if ignore_permissions.unwrap_or(false) {
                args.push("--ignore-permissions");
            }
            if let Some(ref combined) = combined_options {
                args.extend_from_slice(&["-o", combined]);
            }
            args.push(&mount_device);
            execute_command_with_elevation(
                &args,
                true,
                pass_ref,
                false,
                elevation_mode,
                &mount_elevation_state,
                TerminalInteraction::SecretPrompt {
                    operation: mount_operation,
                },
            )
        };

        *mount_result_bg.lock().unwrap() = Some(result);
    });

    // Poll `anylinuxfs status` concurrently while mount command runs. Interactive
    // elevation gets a longer window for approval and a disk passphrase.
    let mount_timeout_secs = if elevation_mode == ElevationMode::InteractiveTerminal {
        INTERACTIVE_ELEVATION_TIMEOUT_SECS
    } else {
        MOUNT_TIMEOUT_SECS
    };
    let retries = mount_timeout_secs * 2;
    for i in 0..retries {
        if i > 0 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Check if mount command finished with an error
        let mount_command_succeeded = if let Some(ref result) = *mount_result.lock().unwrap() {
            let output_text = match result {
                Ok(out) => out.clone(),
                Err(error) => error.message(),
            };
            // Never turn successful informational output into an unlock error,
            // and never mistake cancellation/authentication failures for disk keys.
            if let Some(outcome) = unlock_failure_outcome(result, supplied_credentials, elevation_mode) {
                // Clean up leftover VM from the failed probe attempt
                let _ = execute_command(&["stop", &device], false, None, false);
                let _ = app.emit("status-changed", ());
                let message = if outcome == MountOutcome::EncryptionRequired {
                    "This partition is encrypted. A passphrase or key file is needed to mount it.".to_string()
                } else { output_text };
                return Ok(MountCommandResult::new(outcome, message));
            }
            if let Err(error) = result {
                let _ = app.emit("status-changed", ());
                return Ok(match error {
                    CommandExecutionError::Cancelled => MountCommandResult::new(
                        MountOutcome::Cancelled,
                        "Mount cancelled".to_string(),
                    ),
                    CommandExecutionError::TimedOut => MountCommandResult::new(
                        MountOutcome::TimedOut,
                        "Interactive Terminal mount timed out and cleanup was requested."
                            .to_string(),
                    ),
                    other => MountCommandResult::new(MountOutcome::Failed, other.message()),
                });
            }
            true
        } else {
            false
        };

        // Check if this specific device appeared in `anylinuxfs status`
        if check_device_mounted(&device)
            && (mount_command_succeeded || elevation_state.mark_mount_persistent(&device))
        {
            let _ = app.emit("status-changed", ());
            return Ok(MountCommandResult::new(
                MountOutcome::Mounted,
                Some("Mounted successfully".to_string()),
            ));
        }
    }

    elevation_state.cancel_active_mount(&device);
    let _ = app.emit("status-changed", ());
    Ok(MountCommandResult::new(
        MountOutcome::TimedOut,
        format!(
            "Mount operation timed out after {} seconds and cleanup was requested.",
            mount_timeout_secs
        ),
    ))
}

fn check_device_mounted(device: &str) -> bool {
    crate::cli::get_status()
        .map(|s| super::status::status_has_device(&s, device))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_mounts_auth_errors_and_cancellation_never_request_disk_keys() {
        for result in [
            Ok("Encrypted volume: key load error resolved; mounted successfully".into()),
            Err(CommandExecutionError::Failed("Permission denied - administrator approval required".into())),
            Err(CommandExecutionError::Failed("XFS log needs recovery".into())),
            Err(CommandExecutionError::Cancelled),
            Err(CommandExecutionError::TimedOut),
        ] {
            assert_eq!(unlock_failure_outcome(&result, false, ElevationMode::Native), None);
        }
    }

    #[test]
    fn btrfs_mount_options_accept_at_paths_but_reject_shell_metacharacters() {
        for opts in ["subvol=@", "subvol=@home,compress=zstd:5", "ro,subvol=@/.snapshots/42/snapshot", "subvolid=5"] {
            assert!(validate_mount_options(opts).is_ok());
        }
        for opts in ["subvol=@;id", "subvol=$(id)", "subvol=`id`", "subvol=@\nreboot", "subvol=\"@\""] {
            assert!(validate_mount_options(opts).is_err());
        }
    }

    #[test]
    fn zfs_and_luks_unlock_failures_only_prompt_for_missing_credentials() {
        for message in ["No key available with this passphrase", "Key load error: Incorrect key provided", "Encryption unlock failed - incorrect credentials"] {
            let result = Err(CommandExecutionError::Failed(message.into()));
            assert_eq!(unlock_failure_outcome(&result, false, ElevationMode::Native), Some(MountOutcome::EncryptionRequired));
            assert_eq!(unlock_failure_outcome(&result, true, ElevationMode::Native), Some(MountOutcome::Failed));
            assert_eq!(unlock_failure_outcome(&result, false, ElevationMode::InteractiveTerminal), Some(MountOutcome::Failed));
        }
    }

    #[test]
    fn mount_options_preserve_readonly_and_reject_conflicts() {
        assert_eq!(combined_mount_options(true, Some("noatime")), Ok(Some("ro,noatime".into())));
        assert_eq!(combined_mount_options(true, Some("ro")), Ok(Some("ro".into())));
        assert_eq!(combined_mount_options(false, None), Ok(None));
        assert!(combined_mount_options(true, Some("rw")).is_err());
        assert!(combined_mount_options(false, Some("ro,rw")).is_err());
    }

    #[test]
    fn known_filesystems_and_zfs_members_are_mountable_not_unknown() {
        for fs in ["ext2", "ext3", "ext4", "xfs", "f2fs", "btrfs", "zfs", "zfs_member", "ntfs", "exfat"] {
            assert!(check_filesystem_support(fs).0, "{}", fs);
        }
        assert!(check_filesystem_support("zfs_member").1.unwrap().contains("ZFS pool"));
        assert!(!check_filesystem_support("APFS").0);
        assert!(!check_filesystem_support("unknown").0);
        let part = parse_partition_line("xfs encrypted-backup 1.0 GB disk4s1", "/dev/disk4", 1, &DiskType::Normal).unwrap();
        assert!(!part.encrypted, "a label is not an encryption marker");
    }

    #[test]
    fn key_files_and_terminal_mounts_do_not_set_probe_passphrases() {
        assert_eq!(mount_passphrase(None, true, ElevationMode::Native), None);
        assert_eq!(mount_passphrase(None, true, ElevationMode::InteractiveTerminal), None);
        assert_eq!(mount_passphrase(Some("secret".into()), false, ElevationMode::InteractiveTerminal), None);
        assert_eq!(mount_passphrase(None, false, ElevationMode::Native), Some("##PROBE##".into()));
        assert_eq!(mount_passphrase(Some("secret".into()), false, ElevationMode::Native), Some("secret".into()));
    }

    #[test]
    fn key_file_accepts_spaces_quotes_and_binary_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("disk's key file.key");
        std::fs::write(&path, [0, 255, 10, 128]).unwrap();
        assert!(validate_unlock_credentials(None, path.to_str()).is_ok());
    }

    #[test]
    fn unlock_methods_are_mutually_exclusive() {
        assert!(validate_unlock_credentials(Some("secret"), Some("/key")).is_err());
        assert!(validate_unlock_credentials(Some("secret"), None).is_ok());
        assert!(validate_unlock_credentials(None, None).is_ok());
    }

    #[test]
    fn key_file_rejects_missing_files_directories_and_invalid_paths() {
        let dir = tempfile::tempdir().unwrap();
        assert!(validate_unlock_credentials(None, dir.path().to_str()).is_err());
        assert!(validate_unlock_credentials(None, dir.path().join("missing").to_str()).is_err());
        assert!(validate_unlock_credentials(None, Some("relative.key")).is_err());
        assert!(validate_unlock_credentials(None, Some("/key\nfile")).is_err());
        assert!(validate_unlock_credentials(None, Some("/key\0file")).is_err());
    }

    /// Issue #83: a whole-disk filesystem with no partition table (here a
    /// whole-disk LUKS volume) shows up only as the index-0 entry, whose
    /// identifier is the whole disk itself. It must still be surfaced as a
    /// mountable partition rather than dropped for having an empty list.
    #[test]
    fn whole_disk_filesystem_is_listed() {
        let output = "\
/dev/disk4 (external, physical):
   #: TYPE NAME SIZE IDENTIFIER
   0: crypto_LUKS *2.0 TB disk4
";
        let result = parse_disk_list_output(output).expect("parse should succeed");
        assert_eq!(result.disks.len(), 1, "whole-disk LUKS must not be filtered out");

        let disk = &result.disks[0];
        assert_eq!(disk.device, "/dev/disk4");
        assert!(disk.is_external);
        assert_eq!(disk.size, "2.0 TB");

        assert_eq!(disk.partitions.len(), 1, "the index-0 filesystem is the mountable entry");
        let part = &disk.partitions[0];
        assert_eq!(part.device, "/dev/disk4");
        assert_eq!(part.filesystem, "crypto_LUKS");
        assert!(part.encrypted);
    }

    /// Regression guard: on a partitioned disk the index-0 entry is the scheme
    /// container (GUID_partition_scheme), which must NOT become a partition —
    /// only the real partitions below it should be listed.
    #[test]
    fn partitioned_disk_skips_scheme_container() {
        let output = "\
/dev/disk6 (external, physical):
   #: TYPE NAME SIZE IDENTIFIER
   0: GUID_partition_scheme *64.0 GB disk6
   1: EFI System EFI 209.7 MB disk6s1
   2: ext4 linuxrootfs 63.5 GB disk6s5
";
        let result = parse_disk_list_output(output).expect("parse should succeed");
        assert_eq!(result.disks.len(), 1);

        let disk = &result.disks[0];
        assert_eq!(disk.size, "64.0 GB", "disk size still comes from index 0");
        assert_eq!(disk.partitions.len(), 2, "scheme container must not be listed");

        let devices: Vec<&str> = disk.partitions.iter().map(|p| p.device.as_str()).collect();
        assert_eq!(devices, vec!["/dev/disk6s1", "/dev/disk6s5"]);
    }

    #[test]
    fn mount_outcomes_have_a_stable_frontend_shape() {
        let result = MountCommandResult::new(
            MountOutcome::TimedOut,
            "Cleanup was requested".to_string(),
        );
        let json = serde_json::to_value(result).expect("mount result should serialize");
        assert_eq!(json["outcome"], "timed_out");
        assert_eq!(json["message"], "Cleanup was requested");
    }

    /// Issue #133: the CLI identifies BitLocker directly, so diskutil's
    /// misleading MS-DOS personality must not replace or disable it.
    #[test]
    fn bitlocker_partitions_are_encrypted_and_mountable() {
        let output = "\
/dev/disk6 (external, physical):
   #:                       TYPE NAME                    SIZE       IDENTIFIER
   0:      GUID_partition_scheme                        *2.0 TB     disk6
   3:                  BitLocker Workstation Window...   706.4 GB   disk6s3
   4:       Microsoft Basic Data Sharing                 104.9 GB   disk6s4
   5:                  BitLocker Workstation Data 2...    1.2 TB     disk6s5
";
        let result = parse_disk_list_output(output).expect("parse should succeed");
        let bitlocker: Vec<&Partition> = result.disks[0]
            .partitions
            .iter()
            .filter(|partition| partition.filesystem == "BitLocker")
            .collect();

        assert_eq!(bitlocker.len(), 2);
        assert!(bitlocker.iter().all(|partition| partition.encrypted));
        assert!(is_linux_native_fs("BitLocker"));
        assert_eq!(
            check_filesystem_support("BitLocker"),
            (true, Some("Encrypted (unlock key required)".to_string()))
        );
    }
}
