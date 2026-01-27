use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

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
    let log_path = get_log_path();

    std::thread::spawn(move || {
        let (tx, rx) = channel();

        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to create watcher: {}", e);
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
                return;
            }
        }

        loop {
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
    });

    Ok(())
}
