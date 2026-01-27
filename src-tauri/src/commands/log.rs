use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

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

fn get_log_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join("Library/Logs/anylinuxfs.log")
    } else {
        PathBuf::from("/tmp/anylinuxfs.log")
    }
}

#[tauri::command]
pub fn get_log_content(lines: Option<usize>) -> Result<Vec<String>, String> {
    let log_path = get_log_path();

    if !log_path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(&log_path)
        .map_err(|e| format!("Failed to open log file: {}", e))?;

    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines()
        .filter_map(|l| l.ok())
        .collect();

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

    let log_path = get_log_path();

    // Clone what we need for the thread
    let state_clone = app.state::<Arc<WatcherState>>().inner().clone();

    std::thread::spawn(move || {
        let (tx, rx) = channel();

        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create watcher: {}", e);
                state_clone.log_watcher_running.store(false, Ordering::SeqCst);
                return;
            }
        };

        // Get initial file size
        let mut last_pos = if log_path.exists() {
            std::fs::metadata(&log_path)
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        // Watch the log directory (parent of log file)
        if let Some(parent) = log_path.parent() {
            if watcher.watch(parent, RecursiveMode::NonRecursive).is_err() {
                eprintln!("Failed to watch log directory");
                state_clone.log_watcher_running.store(false, Ordering::SeqCst);
                return;
            }
        }

        loop {
            // Check if we should stop
            if state_clone.log_watcher_stop.load(Ordering::SeqCst) {
                break;
            }

            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(Ok(event)) => {
                    // Check if this event is for our log file
                    let is_our_file = event.paths.iter().any(|p| p == &log_path);

                    if is_our_file {
                        match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) => {
                                // Read new lines
                                if let Ok(mut file) = File::open(&log_path) {
                                    let file_len = file.metadata()
                                        .map(|m| m.len())
                                        .unwrap_or(0);

                                    if file_len > last_pos {
                                        if file.seek(SeekFrom::Start(last_pos)).is_ok() {
                                            let reader = BufReader::new(&file);
                                            for line in reader.lines() {
                                                if let Ok(line) = line {
                                                    let _ = app.emit("log-line", line);
                                                }
                                            }
                                        }
                                        last_pos = file_len;
                                    } else if file_len < last_pos {
                                        // File was truncated, read from beginning
                                        if file.seek(SeekFrom::Start(0)).is_ok() {
                                            let reader = BufReader::new(&file);
                                            for line in reader.lines() {
                                                if let Ok(line) = line {
                                                    let _ = app.emit("log-line", line);
                                                }
                                            }
                                        }
                                        last_pos = file_len;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("Watch error: {:?}", e);
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
                eprintln!("Failed to create disk watcher: {}", e);
                state_clone.disk_watcher_running.store(false, Ordering::SeqCst);
                return;
            }
        };

        // Watch /Volumes for mount/unmount events
        let volumes_path = PathBuf::from("/Volumes");
        if watcher.watch(&volumes_path, RecursiveMode::NonRecursive).is_err() {
            eprintln!("Failed to watch /Volumes");
            state_clone.disk_watcher_running.store(false, Ordering::SeqCst);
            return;
        }

        // Track pending event - we wait for events to settle before emitting
        let mut pending_event: Option<Instant> = None;
        let settle_duration = Duration::from_millis(1500); // Wait 1.5s after last event

        loop {
            // Check if we should stop
            if state_clone.disk_watcher_stop.load(Ordering::SeqCst) {
                break;
            }

            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Ok(event)) => {
                    match event.kind {
                        EventKind::Create(_) | EventKind::Remove(_) => {
                            // Mark that we have a pending event, reset settle timer
                            pending_event = Some(Instant::now());
                        }
                        _ => {}
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("Disk watch error: {:?}", e);
                }
                Err(_) => {
                    // Timeout - check if we have a pending event that has settled
                    if let Some(event_time) = pending_event {
                        if event_time.elapsed() >= settle_duration {
                            // Events have settled, emit and clear
                            let _ = app.emit("disks-changed", ());
                            pending_event = None;
                        }
                    }
                }
            }
        }

        state_clone.disk_watcher_running.store(false, Ordering::SeqCst);
    });

    Ok(())
}

#[tauri::command]
pub fn stop_watchers(app: AppHandle) -> Result<(), String> {
    let state = app.state::<Arc<WatcherState>>();
    state.log_watcher_stop.store(true, Ordering::SeqCst);
    state.disk_watcher_stop.store(true, Ordering::SeqCst);
    Ok(())
}
