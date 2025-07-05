use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

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
            model: "OrbIQ System Monitor".to_string(), // Hardcoded
            manufacturer: "OrbIQ".to_string(), // Hardcoded
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

        let mut config: DaemonConfig = toml::from_str(&content)
            .map_err(|e| ConfigError::Parse(e))?;

        // Always override these hardcoded values regardless of what's in the config file
        config.device.model = "OrbIQ System Monitor".to_string();
        config.device.manufacturer = "OrbIQ".to_string();
        
        // Always derive client_id from device name
        config.mqtt.client_id = format!("orbiq-{}", config.device.name);

        Ok(config)
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
                        println!("Loaded configuration from: {}", path);
                        return config;
                    }
                    Err(e) => {
                        eprintln!("Failed to load config from {}: {}", path, e);
                    }
                }
            }
        }

        println!("No configuration file found, using defaults");
        let mut default_config = Self::default();
        // Always ensure proper client ID and hardcoded values
        default_config.mqtt.client_id = format!("orbiq-{}", default_config.device.name);
        default_config.device.model = "OrbIQ System Monitor".to_string();
        default_config.device.manufacturer = "OrbIQ".to_string();
        default_config
    }

}

#[derive(Debug)]
pub enum ConfigError {
    FileRead(std::path::PathBuf, std::io::Error),
    Parse(toml::de::Error),
    Serialize(toml::ser::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::FileRead(path, e) => write!(f, "Failed to read config file {}: {}", path.display(), e),
            ConfigError::Parse(e) => write!(f, "Failed to parse config: {}", e),
            ConfigError::Serialize(e) => write!(f, "Failed to serialize config: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}