use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use crate::paths::{get_log_dir, get_log_paths};

/// State to track and control watcher threads
pub struct WatcherState {
    log_watcher_running: AtomicBool,
    log_watcher_stop: AtomicBool,
    disk_watcher_running: AtomicBool,
    disk_watcher_stop: AtomicBool,
}

impl Default for WatcherState {
    fn default() -> Self {
        Self {
            log_watcher_running: AtomicBool::new(false),
            log_watcher_stop: AtomicBool::new(false),
            disk_watcher_running: AtomicBool::new(false),
            disk_watcher_stop: AtomicBool::new(false),
        }
    }
}

impl WatcherState {
    pub fn shutdown(&self) {
        self.log_watcher_stop.store(true, Ordering::SeqCst);
        self.disk_watcher_stop.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LogFileInfo {
    pub path: String,
    pub name: String,
    pub label: String,
    pub timestamp: Option<String>,
    pub size: u64,
}

/// Extract device and mount name from the first few lines of a log file.
/// Looks for "macOS: disk: /dev/diskXsY" and "macOS: mount name: XXX"
fn extract_log_label(path: &std::path::Path) -> String {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let reader = BufReader::new(file);
    let mut device = None;
    let mut mount_name = None;

    for line in reader.lines().take(15).flatten() {
        if line.starts_with("macOS: disk: ") {
            device = Some(line.trim_start_matches("macOS: disk: ").to_string());
        } else if line.starts_with("macOS: mount name: ") {
            mount_name = Some(line.trim_start_matches("macOS: mount name: ").to_string());
        }
        if device.is_some() && mount_name.is_some() {
            break;
        }
    }

    match (device, mount_name) {
        (Some(d), Some(m)) => format!("{} ({})", d, m),
        (Some(d), None) => d,
        (None, Some(m)) => m,
        (None, None) => String::new(),
    }
}

#[tauri::command]
pub fn list_log_files() -> Result<Vec<LogFileInfo>, String> {
    let log_paths = get_log_paths();
    let files: Vec<LogFileInfo> = log_paths.into_iter().map(|p| {
        let meta = std::fs::metadata(&p);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let timestamp = meta.ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let elapsed = t.elapsed().unwrap_or_default();
                let secs = elapsed.as_secs();
                if secs < 60 { format!("{}s ago", secs) }
                else if secs < 3600 { format!("{}m ago", secs / 60) }
                else if secs < 86400 { format!("{}h ago", secs / 3600) }
                else { format!("{}d ago", secs / 86400) }
            });
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let label = extract_log_label(&p);
        LogFileInfo {
            path: p.to_string_lossy().to_string(),
            name,
            label,
            timestamp,
            size,
        }
    }).collect();
    Ok(files)
}

#[tauri::command]
pub fn get_log_content(lines: Option<usize>, file_path: Option<String>) -> Result<Vec<String>, String> {
    let paths_to_read = if let Some(ref fp) = file_path {
        // Validate the path is actually an anylinuxfs log
        let p = PathBuf::from(fp);
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if !name.starts_with("anylinuxfs") || !name.ends_with(".log") {
            return Err("Invalid log file path".to_string());
        }
        vec![p]
    } else {
        get_log_paths()
    };

    if paths_to_read.is_empty() {
        return Ok(Vec::new());
    }

    // Read lines from log files (oldest first = chronological order)
    let mut all_lines: Vec<String> = Vec::new();
    for log_path in &paths_to_read {
        if let Ok(file) = File::open(log_path) {
            let reader = BufReader::new(file);
            let file_lines: Vec<String> = reader.lines()
                .filter_map(|l| l.ok())
                .collect();
            all_lines.extend(file_lines);
        }
    }

    let max_lines = lines.unwrap_or(500);
    let start = if all_lines.len() > max_lines {
        all_lines.len() - max_lines
    } else {
        0
    };

    Ok(all_lines[start..].to_vec())
}

#[tauri::command]
pub fn start_log_stream(app: AppHandle) -> Result<(), String> {
    let state = app.state::<Arc<WatcherState>>();

    // Check if already running
    if state.log_watcher_running.swap(true, Ordering::SeqCst) {
        return Ok(()); // Already running, don't start another
    }

    // Reset stop flag
    state.log_watcher_stop.store(false, Ordering::SeqCst);

    let log_dir = get_log_dir();

    // Clone what we need for the thread
    let state_clone = app.state::<Arc<WatcherState>>().inner().clone();

    std::thread::spawn(move || {
        let (tx, rx) = channel();

        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                log::error!("Failed to create watcher: {}", e);
                state_clone.log_watcher_running.store(false, Ordering::SeqCst);
                return;
            }
        };

        // Track last read position per log file
        let mut file_positions: std::collections::HashMap<PathBuf, u64> = std::collections::HashMap::new();

        // Initialize positions for existing log files
        for path in get_log_paths() {
            let pos = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            file_positions.insert(path, pos);
        }

        // Watch the log directory for all anylinuxfs log files
        if watcher.watch(&log_dir, RecursiveMode::NonRecursive).is_err() {
            log::error!("Failed to watch log directory");
            state_clone.log_watcher_running.store(false, Ordering::SeqCst);
            return;
        }

        // Helper: check if a path is an anylinuxfs log file we care about
        let is_anylinuxfs_log = |p: &std::path::Path| -> bool {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            (name.starts_with("anylinuxfs-") || name == "anylinuxfs.log")
                && name.ends_with(".log")
                && !name.contains("kernel") && !name.contains("nethelper")
        };

        loop {
            if state_clone.log_watcher_stop.load(Ordering::SeqCst) {
                break;
            }

            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(Ok(event)) => {
                    for path in &event.paths {
                        if !is_anylinuxfs_log(path) {
                            continue;
                        }

                        match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) => {
                                if let Ok(mut file) = File::open(path) {
                                    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
                                    let last_pos = file_positions.get(path).copied().unwrap_or(0);

                                    if file_len > last_pos {
                                        if file.seek(SeekFrom::Start(last_pos)).is_ok() {
                                            let reader = BufReader::new(&file);
                                            let lines: Vec<String> = reader.lines()
                                                .filter_map(|l| l.ok())
                                                .collect();
                                            if !lines.is_empty() {
                                                let _ = app.emit("log-lines", lines);
                                            }
                                        }
                                        file_positions.insert(path.clone(), file_len);
                                    } else if file_len < last_pos {
                                        // File was truncated, read from beginning
                                        if file.seek(SeekFrom::Start(0)).is_ok() {
                                            let reader = BufReader::new(&file);
                                            let lines: Vec<String> = reader.lines()
                                                .filter_map(|l| l.ok())
                                                .collect();
                                            if !lines.is_empty() {
                                                let _ = app.emit("log-lines", lines);
                                            }
                                        }
                                        file_positions.insert(path.clone(), file_len);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Err(e)) => {
                    log::error!("Watch error: {:?}", e);
                }
                Err(_) => {
                    // Timeout, continue watching
                }
            }
        }

        state_clone.log_watcher_running.store(false, Ordering::SeqCst);
    });

    Ok(())
}

#[tauri::command]
pub fn start_disk_watcher(app: AppHandle) -> Result<(), String> {
    let state = app.state::<Arc<WatcherState>>();

    // Check if already running
    if state.disk_watcher_running.swap(true, Ordering::SeqCst) {
        return Ok(()); // Already running, don't start another
    }

    // Reset stop flag
    state.disk_watcher_stop.store(false, Ordering::SeqCst);

    // Clone what we need for the thread
    let state_clone = app.state::<Arc<WatcherState>>().inner().clone();

    std::thread::spawn(move || {
        let (tx, rx) = channel();

        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                log::error!("Failed to create disk watcher: {}", e);
                state_clone.disk_watcher_running.store(false, Ordering::SeqCst);
                return;
            }
        };

        // Watch /Volumes for mount/unmount events
        let volumes_path = PathBuf::from("/Volumes");
        if watcher.watch(&volumes_path, RecursiveMode::NonRecursive).is_err() {
            log::error!("Failed to watch /Volumes");
            state_clone.disk_watcher_running.store(false, Ordering::SeqCst);
            return;
        }

        // Also watch /dev for physical disk connect/disconnect events
        // This catches Linux-only disks that don't get mounted to /Volumes
        let dev_path = PathBuf::from("/dev");
        if watcher.watch(&dev_path, RecursiveMode::NonRecursive).is_err() {
            log::error!("Failed to watch /dev (continuing with /Volumes only)");
            // Don't return - /Volumes watching is still useful
        }

        // Track pending event - we wait for events to settle before emitting
        let mut pending_event: Option<Instant> = None;
        let settle_duration = Duration::from_millis(1500); // Wait 1.5s after last event

        // Track disk count for polling fallback (for Linux-only disks not in /Volumes)
        let mut last_disk_count = count_disks();
        let mut last_poll = Instant::now();
        let poll_interval = Duration::from_secs(3); // Poll every 3 seconds

        loop {
            // Check if we should stop
            if state_clone.disk_watcher_stop.load(Ordering::SeqCst) {
                break;
            }

            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Ok(event)) => {
                    match event.kind {
                        EventKind::Create(_) | EventKind::Remove(_) => {
                            // Filter /dev events to only disk-related changes
                            let is_disk_event = event.paths.iter().any(|p| {
                                let path_str = p.to_string_lossy();
                                // Match /Volumes/* or /dev/disk*
                                path_str.starts_with("/Volumes/") ||
                                (path_str.starts_with("/dev/disk") && !path_str.contains("s"))
                            });

                            if is_disk_event {
                                // Mark that we have a pending event, reset settle timer
                                pending_event = Some(Instant::now());
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Err(e)) => {
                    log::error!("Disk watch error: {:?}", e);
                }
                Err(_) => {
                    // Timeout - check if we have a pending event that has settled
                    if let Some(event_time) = pending_event {
                        if event_time.elapsed() >= settle_duration {
                            // Events have settled, emit and clear
                            let _ = app.emit("disks-changed", ());
                            pending_event = None;
                            last_disk_count = count_disks(); // Update count after emit
                        }
                    }

                    // Polling fallback: check disk count periodically
                    // This catches Linux-only disks that don't trigger /Volumes events
                    if last_poll.elapsed() >= poll_interval {
                        let current_count = count_disks();
                        if current_count != last_disk_count {
                            pending_event = Some(Instant::now());
                            last_disk_count = current_count;
                        }
                        last_poll = Instant::now();
                    }
                }
            }
        }

        state_clone.disk_watcher_running.store(false, Ordering::SeqCst);
    });

    Ok(())
}

/// Count physical disks by checking /dev/disk* entries
fn count_disks() -> usize {
    std::fs::read_dir("/dev")
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name();
                    let name_str = name.to_string_lossy();
                    // Match disk0, disk1, etc. but not disk0s1 (partitions)
                    name_str.starts_with("disk") &&
                    name_str[4..].chars().all(|c| c.is_ascii_digit())
                })
                .count()
        })
        .unwrap_or(0)
}

#[tauri::command]
pub fn stop_watchers(app: AppHandle) -> Result<(), String> {
    let state = app.state::<Arc<WatcherState>>();
    state.log_watcher_stop.store(true, Ordering::SeqCst);
    state.disk_watcher_stop.store(true, Ordering::SeqCst);
    Ok(())
}
