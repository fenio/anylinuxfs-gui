use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use crate::cli::get_path;

pub struct PtyState {
    writer: Option<Box<dyn Write + Send>>,
    master: Option<Box<dyn portable_pty::MasterPty + Send>>,
}

impl Default for PtyState {
    fn default() -> Self {
        Self {
            writer: None,
            master: None,
        }
    }
}

#[tauri::command]
pub async fn start_shell(
    app: AppHandle,
    state: tauri::State<'_, Arc<Mutex<PtyState>>>,
) -> Result<(), String> {
    let cli_path = get_path()
        .ok_or_else(|| "anylinuxfs CLI not found".to_string())?;

    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let mut cmd = CommandBuilder::new(cli_path);
    cmd.arg("shell");

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;

    // Get writer for input
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to get PTY writer: {}", e))?;

    // Store writer and master in state
    {
        let mut pty_state = state.lock().map_err(|e| format!("Lock error: {}", e))?;
        pty_state.writer = Some(writer);
        pty_state.master = Some(pair.master);
    }

    // Get reader for output
    let mut reader = {
        let pty_state = state.lock().map_err(|e| format!("Lock error: {}", e))?;
        pty_state.master.as_ref()
            .ok_or("No master PTY")?
            .try_clone_reader()
            .map_err(|e| format!("Failed to get PTY reader: {}", e))?
    };

    // Spawn thread to read output and emit events
    let app_for_output = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_for_output.emit("shell-output", data);
                }
                Err(_) => break,
            }
        }
    });

    // Spawn thread to wait for child exit and emit exit event
    let state_clone = state.inner().clone();
    std::thread::spawn(move || {
        let _ = child.wait();
        // Clean up state
        if let Ok(mut pty_state) = state_clone.lock() {
            pty_state.writer = None;
            pty_state.master = None;
        }
        let _ = app.emit("shell-exit", ());
    });

    Ok(())
}

#[tauri::command]
pub fn write_shell(
    data: String,
    state: tauri::State<'_, Arc<Mutex<PtyState>>>,
) -> Result<(), String> {
    let mut pty_state = state.lock().map_err(|e| format!("Lock error: {}", e))?;

    if let Some(ref mut writer) = pty_state.writer {
        writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("Write error: {}", e))?;
        writer.flush().map_err(|e| format!("Flush error: {}", e))?;
    } else {
        return Err("Shell not running".to_string());
    }

    Ok(())
}

#[tauri::command]
pub fn resize_shell(
    rows: u16,
    cols: u16,
    state: tauri::State<'_, Arc<Mutex<PtyState>>>,
) -> Result<(), String> {
    let pty_state = state.lock().map_err(|e| format!("Lock error: {}", e))?;

    if let Some(ref master) = pty_state.master {
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Resize error: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub fn stop_shell(state: tauri::State<'_, Arc<Mutex<PtyState>>>) -> Result<(), String> {
    let mut pty_state = state.lock().map_err(|e| format!("Lock error: {}", e))?;
    pty_state.writer = None;
    pty_state.master = None;
    Ok(())
}
