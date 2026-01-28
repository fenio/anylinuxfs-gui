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

/// Get the log file path
/// Checks Library/Logs on macOS, falls back to /tmp
pub fn get_log_path() -> PathBuf {
    // Try macOS log directory first
    if let Some(home) = dirs::home_dir() {
        let macos_log = home.join("Library/Logs/anylinuxfs.log");
        if macos_log.exists() {
            return macos_log;
        }
    }
    // Fallback to /tmp
    PathBuf::from("/tmp/anylinuxfs.log")
}

/// Command timeout in seconds for blocking operations
pub const COMMAND_TIMEOUT_SECS: u64 = 30;

/// Mount timeout in seconds (longer for initial VM startup)
pub const MOUNT_TIMEOUT_SECS: u64 = 60;
