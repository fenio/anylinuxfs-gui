use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::cli::execute_command;

fn get_config_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".anylinuxfs/config.toml")
    } else {
        PathBuf::from("/tmp/anylinuxfs-config.toml")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub ram_mb: Option<u32>,
    pub vcpus: Option<u32>,
    pub log_level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TomlConfig {
    krun: Option<KrunConfig>,
}

#[derive(Debug, Deserialize)]
struct KrunConfig {
    ram_size_mib: Option<u32>,
    num_vcpus: Option<u32>,
    #[serde(deserialize_with = "deserialize_log_level", default)]
    log_level: Option<String>,
}

fn deserialize_log_level<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let value: toml::Value = serde::Deserialize::deserialize(deserializer)?;

    match value {
        toml::Value::String(s) => Ok(Some(s)),
        toml::Value::Integer(i) => {
            // Convert numeric log levels to string names
            // Mapping: off=0, error=1, warn=2, info=3, debug=4, trace=5
            let level = match i {
                0 => "off",
                1 => "error",
                2 => "warn",
                3 => "info",
                4 => "debug",
                5 | _ => "trace",
            };
            Ok(Some(level.to_string()))
        }
        _ => Err(D::Error::custom("expected string or integer for log_level")),
    }
}

#[tauri::command]
pub fn get_config() -> Result<AppConfig, String> {
    let config_path = get_config_path();

    if !config_path.exists() {
        return Ok(AppConfig::default());
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;

    let toml_config: TomlConfig = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config: {}", e))?;

    Ok(AppConfig {
        ram_mb: toml_config.krun.as_ref().and_then(|k| k.ram_size_mib),
        vcpus: toml_config.krun.as_ref().and_then(|k| k.num_vcpus),
        log_level: toml_config.krun.as_ref().and_then(|k| k.log_level.clone()),
    })
}

// Valid configuration values (must match frontend options)
const VALID_RAM_OPTIONS: &[u32] = &[512, 1024, 2048, 4096, 8192, 16384];
const VALID_VCPU_OPTIONS: &[u32] = &[1, 2, 4, 8, 16];
const VALID_LOG_LEVELS: &[&str] = &["off", "error", "warn", "info", "debug", "trace"];

#[tauri::command]
pub async fn update_config(ram_mb: Option<u32>, vcpus: Option<u32>, log_level: Option<String>) -> Result<(), String> {
    // Validate inputs before running commands
    if let Some(ram) = ram_mb {
        if !VALID_RAM_OPTIONS.contains(&ram) {
            return Err(format!("Invalid RAM value: {}MB. Valid options: {:?}", ram, VALID_RAM_OPTIONS));
        }
    }

    if let Some(cpus) = vcpus {
        if !VALID_VCPU_OPTIONS.contains(&cpus) {
            return Err(format!("Invalid vCPU value: {}. Valid options: {:?}", cpus, VALID_VCPU_OPTIONS));
        }
    }

    if let Some(ref level) = log_level {
        if !VALID_LOG_LEVELS.contains(&level.as_str()) {
            return Err(format!("Invalid log level: '{}'. Valid options: {:?}", level, VALID_LOG_LEVELS));
        }
    }

    // Run in blocking task to avoid freezing UI
    tokio::task::spawn_blocking(move || {
        // Use the CLI to update config values
        if let Some(ram) = ram_mb {
            execute_command(&["config", "-r", &ram.to_string()], false, None)?;
        }

        if let Some(cpus) = vcpus {
            execute_command(&["config", "-n", &cpus.to_string()], false, None)?;
        }

        if let Some(level) = log_level {
            execute_command(&["config", "-l", &level], false, None)?;
        }

        Ok(())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}
