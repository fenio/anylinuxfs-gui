use crate::cli::execute_command;

/// Validate package name to prevent command injection
/// Package names must contain only alphanumeric characters, dots, underscores, hyphens,
/// and optionally a version specifier like @edge
fn validate_package_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Package name cannot be empty".to_string());
    }
    if name.len() > 128 {
        return Err("Package name too long".to_string());
    }
    // Allow: alphanumeric, dots, underscores, hyphens, plus signs (for g++ etc)
    // Also allow @ for repository tags like package@edge
    let valid = name.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '+' || c == '@'
    });
    if !valid {
        return Err(format!("Package name '{}' contains invalid characters", name));
    }
    // Must not start with a dash (could be interpreted as an option)
    if name.starts_with('-') {
        return Err("Package name cannot start with '-'".to_string());
    }
    Ok(())
}

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

    // Validate all package names before executing
    for pkg in &packages {
        validate_package_name(pkg)?;
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

    // Validate all package names before executing
    for pkg in &packages {
        validate_package_name(pkg)?;
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
