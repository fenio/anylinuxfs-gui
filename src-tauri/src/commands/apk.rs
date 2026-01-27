use crate::cli::execute_command;

#[tauri::command]
pub fn list_packages() -> Result<Vec<String>, String> {
    let output = execute_command(&["apk", "info"], false, None)?;

    let packages: Vec<String> = output
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    Ok(packages)
}

#[tauri::command]
pub async fn add_packages(packages: Vec<String>) -> Result<(), String> {
    if packages.is_empty() {
        return Err("No packages specified".to_string());
    }

    tokio::task::spawn_blocking(move || {
        let mut args = vec!["apk", "add"];
        let pkg_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
        args.extend(pkg_refs);
        execute_command(&args, false, None)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

#[tauri::command]
pub async fn remove_packages(packages: Vec<String>) -> Result<(), String> {
    if packages.is_empty() {
        return Err("No packages specified".to_string());
    }

    tokio::task::spawn_blocking(move || {
        let mut args = vec!["apk", "del"];
        let pkg_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
        args.extend(pkg_refs);
        execute_command(&args, false, None)?;
        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
