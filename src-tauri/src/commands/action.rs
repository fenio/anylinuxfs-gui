use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAction {
    pub name: String,
    pub description: String,
    pub before_mount: String,
    pub after_mount: String,
    pub before_unmount: String,
    pub environment: Vec<String>,
    pub capture_environment: Vec<String>,
    pub override_nfs_export: String,
    pub required_os: String,
    pub is_upstream: bool,
}

#[derive(Debug, Deserialize)]
struct ActionConfig {
    #[serde(default)]
    description: String,
    #[serde(default)]
    before_mount: String,
    #[serde(default)]
    after_mount: String,
    #[serde(default)]
    before_unmount: String,
    #[serde(default)]
    environment: Vec<String>,
    #[serde(default)]
    capture_environment: Vec<String>,
    #[serde(default)]
    override_nfs_export: String,
    #[serde(default)]
    required_os: String,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    custom_actions: HashMap<String, ActionConfig>,
}

fn get_user_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".anylinuxfs/config.toml")
}

fn get_upstream_config_path() -> PathBuf {
    PathBuf::from("/opt/homebrew/etc/anylinuxfs.toml")
}

fn parse_actions_from_file(path: &PathBuf, is_upstream: bool) -> Vec<CustomAction> {
    let mut actions = Vec::new();

    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(config) = toml::from_str::<ConfigFile>(&content) {
            for (name, action_config) in config.custom_actions {
                actions.push(CustomAction {
                    name,
                    description: action_config.description,
                    before_mount: action_config.before_mount,
                    after_mount: action_config.after_mount,
                    before_unmount: action_config.before_unmount,
                    environment: action_config.environment,
                    capture_environment: action_config.capture_environment,
                    override_nfs_export: action_config.override_nfs_export,
                    required_os: action_config.required_os,
                    is_upstream,
                });
            }
        }
    }

    actions
}

#[tauri::command]
pub fn list_custom_actions() -> Result<Vec<CustomAction>, String> {
    let mut all_actions = Vec::new();

    // Load upstream actions (read-only)
    let upstream_path = get_upstream_config_path();
    all_actions.extend(parse_actions_from_file(&upstream_path, true));

    // Load user actions
    let user_path = get_user_config_path();
    all_actions.extend(parse_actions_from_file(&user_path, false));

    // Sort by name
    all_actions.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(all_actions)
}

#[tauri::command]
pub fn create_custom_action(
    name: String,
    description: String,
    before_mount: String,
    after_mount: String,
    before_unmount: String,
    environment: Vec<String>,
    capture_environment: Vec<String>,
    override_nfs_export: String,
    required_os: String,
) -> Result<(), String> {
    let config_path = get_user_config_path();

    // Ensure config directory exists
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Read existing config
    let content = fs::read_to_string(&config_path).unwrap_or_default();

    // Parse as raw TOML value to preserve other sections
    let mut doc: toml::Table = toml::from_str(&content).unwrap_or_default();

    // Get or create custom_actions section
    let custom_actions = doc
        .entry("custom_actions")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or("Invalid config format")?;

    // Check if action already exists
    if custom_actions.contains_key(&name) {
        return Err(format!("Action '{}' already exists", name));
    }

    // Create action table
    let mut action_table = toml::Table::new();
    action_table.insert("description".to_string(), toml::Value::String(description));
    action_table.insert("before_mount".to_string(), toml::Value::String(before_mount));
    action_table.insert("after_mount".to_string(), toml::Value::String(after_mount));
    action_table.insert("before_unmount".to_string(), toml::Value::String(before_unmount));
    action_table.insert(
        "environment".to_string(),
        toml::Value::Array(environment.into_iter().map(toml::Value::String).collect()),
    );
    action_table.insert(
        "capture_environment".to_string(),
        toml::Value::Array(capture_environment.into_iter().map(toml::Value::String).collect()),
    );
    action_table.insert("override_nfs_export".to_string(), toml::Value::String(override_nfs_export));
    action_table.insert("required_os".to_string(), toml::Value::String(required_os));

    custom_actions.insert(name, toml::Value::Table(action_table));

    // Write back
    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_path, new_content)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn update_custom_action(
    name: String,
    description: String,
    before_mount: String,
    after_mount: String,
    before_unmount: String,
    environment: Vec<String>,
    capture_environment: Vec<String>,
    override_nfs_export: String,
    required_os: String,
) -> Result<(), String> {
    let config_path = get_user_config_path();

    // Read existing config
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;

    // Parse as raw TOML value
    let mut doc: toml::Table = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config: {}", e))?;

    // Get custom_actions section
    let custom_actions = doc
        .get_mut("custom_actions")
        .and_then(|v| v.as_table_mut())
        .ok_or("No custom_actions section found")?;

    // Check if action exists
    if !custom_actions.contains_key(&name) {
        return Err(format!("Action '{}' not found", name));
    }

    // Update action table
    let mut action_table = toml::Table::new();
    action_table.insert("description".to_string(), toml::Value::String(description));
    action_table.insert("before_mount".to_string(), toml::Value::String(before_mount));
    action_table.insert("after_mount".to_string(), toml::Value::String(after_mount));
    action_table.insert("before_unmount".to_string(), toml::Value::String(before_unmount));
    action_table.insert(
        "environment".to_string(),
        toml::Value::Array(environment.into_iter().map(toml::Value::String).collect()),
    );
    action_table.insert(
        "capture_environment".to_string(),
        toml::Value::Array(capture_environment.into_iter().map(toml::Value::String).collect()),
    );
    action_table.insert("override_nfs_export".to_string(), toml::Value::String(override_nfs_export));
    action_table.insert("required_os".to_string(), toml::Value::String(required_os));

    custom_actions.insert(name, toml::Value::Table(action_table));

    // Write back
    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_path, new_content)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn delete_custom_action(name: String) -> Result<(), String> {
    let config_path = get_user_config_path();

    // Check if config file exists
    if !config_path.exists() {
        return Err(format!("Action '{}' not found", name));
    }

    // Read existing config
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;

    // Parse as raw TOML value
    let mut doc: toml::Table = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config: {}", e))?;

    // Get custom_actions section
    let custom_actions = match doc.get_mut("custom_actions").and_then(|v| v.as_table_mut()) {
        Some(actions) => actions,
        None => return Err(format!("Action '{}' not found", name)),
    };

    // Remove action
    if custom_actions.remove(&name).is_none() {
        return Err(format!("Action '{}' not found", name));
    }

    // Write back
    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_path, new_content)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}
