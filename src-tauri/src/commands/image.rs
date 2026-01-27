use serde::Serialize;
use crate::cli::execute_command;

#[derive(Debug, Clone, Serialize)]
pub struct VmImage {
    pub name: String,
    pub installed: bool,
}

#[tauri::command]
pub fn list_images() -> Result<Vec<VmImage>, String> {
    let output = execute_command(&["image", "list"], false, None)?;

    let mut images = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let installed = line.ends_with("(installed)");
        let name = if installed {
            line.trim_end_matches("(installed)").trim().to_string()
        } else {
            line.to_string()
        };

        images.push(VmImage { name, installed });
    }

    Ok(images)
}

#[tauri::command]
pub async fn install_image(name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        execute_command(&["image", "install", &name], false, None)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

#[tauri::command]
pub async fn uninstall_image(name: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        execute_command(&["image", "uninstall", &name], false, None)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
