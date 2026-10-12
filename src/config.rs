use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const DEVICE_MODEL: &str = "OrbIQ System Monitor";
pub const DEVICE_MANUFACTURER: &str = "OrbIQ";

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct MqttConfig {
    pub broker: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(skip)] // Don't serialize/deserialize client_id - it's auto-generated
    pub client_id: String,
    pub keep_alive_secs: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct DeviceConfig {
    pub name: String,
    #[serde(skip)] // Don't serialize/deserialize - these are hardcoded
    pub model: String,
    #[serde(skip)] // Don't serialize/deserialize - these are hardcoded
    pub manufacturer: String,
    pub sw_version: Option<String>,
    pub hw_version: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct DaemonConfig {
    pub mqtt: MqttConfig,
    pub device: DeviceConfig,
    pub update_interval_secs: u64,
    pub discovery_delay_ms: u64,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            broker: "localhost".to_string(),
            port: 1883,
            username: None,
            password: None,
            client_id: "orbiq-default".to_string(), // Will be overridden
            keep_alive_secs: 30,
        }
    }
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            name: "system-monitor".to_string(),
            model: DEVICE_MODEL.to_string(),
            manufacturer: DEVICE_MANUFACTURER.to_string(),
            sw_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            hw_version: Some("1.0".to_string()),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            mqtt: MqttConfig::default(),
            device: DeviceConfig::default(),
            update_interval_secs: 30,
            discovery_delay_ms: 100,
        }
    }
}

impl DaemonConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(&path)
            .map_err(|e| ConfigError::FileRead(path.as_ref().to_path_buf(), e))?;

        let config: DaemonConfig = toml::from_str(&content).map_err(ConfigError::Parse)?;
        Ok(config.with_derived_fields())
    }

    /// Model and manufacturer are fixed regardless of the config file, and the MQTT
    /// client ID is always derived from the device name.
    fn with_derived_fields(mut self) -> Self {
        self.device.model = DEVICE_MODEL.to_string();
        self.device.manufacturer = DEVICE_MANUFACTURER.to_string();
        self.mqtt.client_id = format!("orbiq-{}", self.device.name);
        self
    }

    pub fn load_with_fallback() -> Self {
        // Try to load from standard locations in order of preference
        let config_paths = [
            "/etc/orbiq_system_monitor/config.toml",
            "/etc/orbiq/config.toml",
            "./orbiq_system_monitor.toml",
            "./config.toml",
        ];

        for path in &config_paths {
            if Path::new(path).exists() {
                match Self::load_from_file(path) {
                    Ok(config) => {
                        println!("Loaded configuration from: {path}");
                        return config;
                    }
                    Err(e) => {
                        eprintln!("Failed to load config from {path}: {e}");
                    }
                }
            }
        }

        println!("No configuration file found, using defaults");
        Self::default().with_derived_fields()
    }
}

#[derive(Debug)]
pub enum ConfigError {
    FileRead(std::path::PathBuf, std::io::Error),
    Parse(toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::FileRead(path, e) => write!(f, "Failed to read config file {}: {e}", path.display()),
            ConfigError::Parse(e) => write!(f, "Failed to parse config: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_fields_override_the_config_file() {
        let toml = r#"
            [mqtt]
            broker = "broker.example"
            [device]
            name = "box1"
            model = "something else"
        "#;
        let config: DaemonConfig = toml::from_str::<DaemonConfig>(toml).unwrap().with_derived_fields();
        assert_eq!(config.mqtt.client_id, "orbiq-box1");
        assert_eq!(config.device.model, DEVICE_MODEL);
        assert_eq!(config.device.manufacturer, DEVICE_MANUFACTURER);
        assert_eq!(config.mqtt.broker, "broker.example");
        assert_eq!(config.update_interval_secs, 30);
    }
}
