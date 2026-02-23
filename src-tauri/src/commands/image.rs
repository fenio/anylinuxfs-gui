use serde::Serialize;
use crate::cli::execute_command;

/// Validate image name format to prevent path traversal or command injection.
/// Image names should only contain alphanumeric characters, hyphens, dots, and underscores.
pub fn validate_image_name(image: &str) -> Result<(), String> {
    if image.is_empty() {
        return Err("Image name cannot be empty".to_string());
    }
    let valid = image.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_');
    if !valid {
        return Err(format!("Invalid image name '{}': contains invalid characters", image));
    }
    if image.contains("..") {
        return Err("Image name cannot contain '..'".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct VmImage {
    pub name: String,
    pub installed: bool,
}

#[tauri::command]
pub fn list_images() -> Result<Vec<VmImage>, String> {
    let output = execute_command(&["image", "list"], false, None, false)?;

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
    validate_image_name(&name)?;
    tokio::task::spawn_blocking(move || {
        execute_command(&["image", "install", &name], false, None, false)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

#[tauri::command]
pub async fn uninstall_image(name: String) -> Result<(), String> {
    validate_image_name(&name)?;
    tokio::task::spawn_blocking(move || {
        execute_command(&["image", "uninstall", &name], false, None, false)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
