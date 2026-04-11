use std::path::PathBuf;

/// Get the socket path for communicating with anylinuxfs CLI
/// Checks cache directory first, falls back to /tmp for compatibility
pub fn get_socket_path() -> PathBuf {
    // Check if CLI created socket in cache dir first
    if let Some(cache_dir) = dirs::cache_dir() {
        let cache_socket = cache_dir.join("anylinuxfs").join("anylinuxfs.sock");
        if cache_socket.exists() {
            return cache_socket;
        }
    }
    // Fallback to /tmp for compatibility with current CLI
    PathBuf::from("/tmp/anylinuxfs.sock")
}

/// Get the log directory path
pub fn get_log_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        let macos_log_dir = home.join("Library/Logs");
        if macos_log_dir.exists() {
            return macos_log_dir;
        }
    }
    PathBuf::from("/tmp")
}

/// Get all anylinuxfs log file paths, sorted by modification time (newest first)
pub fn get_log_paths() -> Vec<PathBuf> {
    let log_dir = get_log_dir();
    let mut logs: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Match anylinuxfs-*.log but not kernel or nethelper logs
            if name_str.starts_with("anylinuxfs-") && name_str.ends_with(".log")
                && !name_str.contains("kernel") && !name_str.contains("nethelper")
            {
                let mtime = entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                logs.push((entry.path(), mtime));
            }
            // Also match legacy anylinuxfs.log
            if name_str == "anylinuxfs.log" {
                let mtime = entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                logs.push((entry.path(), mtime));
            }
        }
    }

    // Also check /tmp for legacy fallback
    let tmp_log = PathBuf::from("/tmp/anylinuxfs.log");
    if tmp_log.exists() && log_dir != PathBuf::from("/tmp") {
        let mtime = std::fs::metadata(&tmp_log).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        logs.push((tmp_log, mtime));
    }

    logs.sort_by(|a, b| a.1.cmp(&b.1)); // oldest first (chronological)
    logs.into_iter().map(|(p, _)| p).collect()
}

/// Get the log file path (legacy single-file API, returns newest)
pub fn get_log_path() -> PathBuf {
    get_log_paths().into_iter().last()
        .unwrap_or_else(|| get_log_dir().join("anylinuxfs.log"))
}

/// Command timeout in seconds for blocking operations
pub const COMMAND_TIMEOUT_SECS: u64 = 30;

/// Mount timeout in seconds (longer for initial VM startup)
pub const MOUNT_TIMEOUT_SECS: u64 = 60;
