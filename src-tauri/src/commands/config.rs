use serde::{Deserialize, Serialize};
use crate::cli::execute_command;

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
                5 => "trace",
                _ => return Err(D::Error::custom(format!("invalid numeric log level: {}. Expected 0-5 (off=0, error=1, warn=2, info=3, debug=4, trace=5)", i))),
            };
            Ok(Some(level.to_string()))
        }
        _ => Err(D::Error::custom("expected string or integer for log_level")),
    }
}

#[tauri::command]
pub fn get_config() -> Result<AppConfig, String> {
    // Run `anylinuxfs config` to get full config with defaults
    let output = execute_command(&["config"], false, None)?;

    // Fix unquoted string values (CLI outputs `log_level = off` instead of `log_level = "off"`)
    let fixed_output = fix_unquoted_strings(&output);

    let toml_config: TomlConfig = toml::from_str(&fixed_output)
        .map_err(|e| format!("Failed to parse config: {}", e))?;

    Ok(AppConfig {
        ram_mb: toml_config.krun.as_ref().and_then(|k| k.ram_size_mib),
        vcpus: toml_config.krun.as_ref().and_then(|k| k.num_vcpus),
        log_level: toml_config.krun.as_ref().and_then(|k| k.log_level.clone()),
    })
}

fn fix_unquoted_strings(input: &str) -> String {
    // Fix unquoted string values in TOML output from anylinuxfs CLI
    input
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            // Skip section headers, empty lines, comments
            if trimmed.starts_with('[') || trimmed.is_empty() || trimmed.starts_with('#') {
                return line.to_string();
            }

            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let value = trimmed[eq_pos + 1..].trim();

                // Skip if already quoted, empty, array, or looks like a number/boolean
                if value.is_empty()
                    || value.starts_with('"')
                    || value.starts_with('\'')
                    || value.starts_with('[')
                    || value.starts_with('{')
                    || value == "true"
                    || value == "false"
                    || value.parse::<i64>().is_ok()
                    || value.parse::<f64>().is_ok()
                {
                    return line.to_string();
                }

                // Quote the unquoted string value
                return format!("{} = \"{}\"", key, value);
            }

            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Valid configuration ranges
const MIN_RAM_MB: u32 = 256;
const MAX_RAM_MB: u32 = 65536;
const MIN_VCPUS: u32 = 1;
const MAX_VCPUS: u32 = 32;
const VALID_LOG_LEVELS: &[&str] = &["off", "error", "warn", "info", "debug", "trace"];

#[tauri::command]
pub async fn update_config(ram_mb: Option<u32>, vcpus: Option<u32>, log_level: Option<String>) -> Result<(), String> {
    // Validate inputs before running commands
    if let Some(ram) = ram_mb {
        if ram < MIN_RAM_MB || ram > MAX_RAM_MB {
            return Err(format!("Invalid RAM value: {}MB. Must be between {} and {} MB.", ram, MIN_RAM_MB, MAX_RAM_MB));
        }
    }

    if let Some(cpus) = vcpus {
        if cpus < MIN_VCPUS || cpus > MAX_VCPUS {
            return Err(format!("Invalid vCPU value: {}. Must be between {} and {}.", cpus, MIN_VCPUS, MAX_VCPUS));
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
