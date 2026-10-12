use crate::mqtt_client::MqttPayload;
use crate::sensors::{SystemSensor, SystemSensorType};
use crate::config::{DEVICE_MANUFACTURER, DEVICE_MODEL};
use serde_json::json;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceInfo {
    pub identifiers: Vec<String>,
    pub name: String,
    pub model: String,
    pub manufacturer: String,
    pub sw_version: Option<String>,
    pub hw_version: Option<String>,
}

impl DeviceInfo {
    pub fn from_config(device_config: &crate::config::DeviceConfig) -> Self {
        Self {
            identifiers: vec![format!("orbiq_{}", device_config.name)],
            name: device_config.name.clone(),
            model: DEVICE_MODEL.to_string(),
            manufacturer: DEVICE_MANUFACTURER.to_string(),
            sw_version: device_config
                .sw_version
                .clone()
                .or_else(|| Some(env!("CARGO_PKG_VERSION").to_string())),
            hw_version: device_config
                .hw_version
                .clone()
                .or_else(|| Some("1.0".to_string())),
        }
    }
}

fn last_segment(name: &str) -> &str {
    name.rsplit('_').next().unwrap_or("Unknown")
}

// Generate friendly names for temperature sensors
fn generate_friendly_name(sensor: &SystemSensor) -> String {
    match sensor.name.as_str() {
        name if name.contains("k10temp") => "CPU Temperature".to_string(),
        name if name.contains("nouveau") => "GPU Temperature".to_string(),
        name if name.contains("nvidia") => sensor.label.clone().unwrap_or_else(|| "GPU Temperature".to_string()),
        name if name.contains("nvme") => format!("NVMe {} Temperature", last_segment(name)),
        name if name.contains("coretemp") => format!("Core {} Temperature", last_segment(name)),
        name if name.contains("acpi") => "System Temperature".to_string(),
        name if name.contains("amdgpu") => "AMD GPU Temperature".to_string(),
        name if name.contains("radeon") => "Radeon GPU Temperature".to_string(),
        name if name.contains("asus") => "ASUS Sensor Temperature".to_string(),
        name if name.contains("iwlwifi") => "WiFi Module Temperature".to_string(),
        name if name.contains("thermal") => "Thermal Zone Temperature".to_string(),
        name if name.contains("nct6687") => sensor.label.clone().unwrap_or_else(|| name.replace('_', " ")),
        _ => format!("{} Temperature", sensor.name.replace('_', " ")),
    }
}

fn generate_friendly_name_for_fan(sensor: &SystemSensor) -> String {
    match &sensor.label {
        Some(label) => label.clone(),
        None => format!("Fan {}", sensor.name),
    }
}

fn generate_friendly_name_for_current(sensor: &SystemSensor) -> String {
    match &sensor.label {
        Some(label) => format!("{label} Current"),
        None => "Current".to_string(),
    }
}

// "disk_used_mnt_data" -> "Disk Used (MNT DATA)", "disk_used_root" -> "Disk Used (Root)"
fn disk_friendly_name(sensor: &SystemSensor, prefix: &str, title: &str) -> String {
    if sensor.name.contains("root") {
        format!("{title} (Root)")
    } else {
        let mount_name = sensor.name.replace(prefix, "").replace('_', " ");
        format!("{title} ({})", mount_name.to_uppercase())
    }
}

// Generate friendly names for system sensors
fn generate_system_friendly_name(sensor: &SystemSensor) -> String {
    match sensor.sensor_type {
        SystemSensorType::CpuUsage => "CPU Usage".to_string(),
        SystemSensorType::MemoryUsage => "Memory Usage".to_string(),
        SystemSensorType::MemoryUsed => "Memory Used".to_string(),
        SystemSensorType::MemoryTotal => "Memory Total".to_string(),
        SystemSensorType::DiskUsage => disk_friendly_name(sensor, "disk_usage_", "Disk Usage"),
        SystemSensorType::DiskUsed => disk_friendly_name(sensor, "disk_used_", "Disk Used"),
        SystemSensorType::DiskTotal => disk_friendly_name(sensor, "disk_total_", "Disk Total"),
        SystemSensorType::Fan => generate_friendly_name_for_fan(sensor),
        SystemSensorType::Current => generate_friendly_name_for_current(sensor),
        SystemSensorType::Temperature => generate_friendly_name(sensor),
        SystemSensorType::GpuUsage => sensor.label.clone().unwrap_or_else(|| "GPU Usage".to_string()),
        SystemSensorType::GpuMemoryUsage => {
            sensor.label.clone().unwrap_or_else(|| "GPU Memory Usage".to_string())
        }
        SystemSensorType::CpuFrequency => {
            if sensor.name == "cpu_max_frequency" {
                "CPU Max Frequency".to_string()
            } else {
                "CPU Frequency".to_string()
            }
        }
        SystemSensorType::CpuGovernor => "CPU Governor".to_string(),
        SystemSensorType::CpuEnergyPreference => "CPU Energy Preference".to_string(),
        SystemSensorType::CpuBoost => "CPU Boost".to_string(),
    }
}

fn topic(device_name: &str, sensor_name: &str, sub_topic: &str) -> String {
    format!("homeassistant/sensor/orbiq_{device_name}/{sensor_name}/{sub_topic}")
}

pub fn system_state(sensor: &SystemSensor, device_name: &str) -> MqttPayload {
    MqttPayload {
        topic: topic(device_name, &sensor.name, "state"),
        payload: json!({ "value": sensor.value }).to_string(),
        retain: false,
    }
}

pub fn system_sensor_availability(
    sensor: &SystemSensor,
    device_name: &str,
    available: bool,
) -> MqttPayload {
    MqttPayload {
        topic: topic(device_name, &sensor.name, "availability"),
        payload: if available { "online" } else { "offline" }.to_string(),
        retain: true,
    }
}

// Topic pattern used to subscribe for previously-published discovery configs on startup,
// so they can be diffed against currently-detected sensors and stale ones removed.
pub fn discovery_wildcard_topic(device_name: &str) -> String {
    topic(device_name, "+", "config")
}

// Recovers the sensor name from a discovery config topic matching discovery_wildcard_topic.
pub fn extract_discovered_sensor_name(topic_str: &str, device_name: &str) -> Option<String> {
    let prefix = format!("homeassistant/sensor/orbiq_{device_name}/");
    topic_str
        .strip_prefix(prefix.as_str())?
        .strip_suffix("/config")
        .map(str::to_string)
}

// An empty retained payload on the config topic tells Home Assistant to remove the entity.
// The availability topic is cleared too so no stale retained state lingers on the broker.
pub fn stale_sensor_removal_payloads(sensor_name: &str, device_name: &str) -> Vec<MqttPayload> {
    ["config", "availability"]
        .into_iter()
        .map(|sub_topic| MqttPayload {
            topic: topic(device_name, sensor_name, sub_topic),
            payload: String::new(),
            retain: true,
        })
        .collect()
}

pub fn system_discovery_config(
    sensor: &SystemSensor,
    device_name: &str,
    device_info: &DeviceInfo,
) -> MqttPayload {
    let mut config = json!({
        "name": generate_system_friendly_name(sensor),
        "unique_id": format!("orbiq_{device_name}_{}", sensor.name),
        "default_entity_id": format!("sensor.orbiq_{device_name}_{}", sensor.name),
        "state_topic": topic(device_name, &sensor.name, "state"),
        "value_template": "{{ value_json.value }}",
        "availability": {
            "topic": topic(device_name, &sensor.name, "availability"),
            "payload_available": "online",
            "payload_not_available": "offline"
        },
        "icon": sensor.sensor_type.icon(),
        "device": device_info
    });

    // Text sensors (e.g. a governor name) must not carry a unit or state_class:
    // Home Assistant would then expect a number and reject the state.
    if !sensor.is_text() {
        config["unit_of_measurement"] = json!(sensor.unit);
        config["state_class"] = json!("measurement");
    }
    if let Some(class) = sensor.sensor_type.device_class() {
        config["device_class"] = json!(class);
    }
    MqttPayload {
        topic: topic(device_name, &sensor.name, "config"),
        payload: config.to_string(),
        retain: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_discovered_sensor_name_matches_config_topic() {
        let config_topic = format!(
            "homeassistant/sensor/orbiq_{}/{}/config",
            "my-device", "cpu_usage"
        );
        assert_eq!(
            extract_discovered_sensor_name(&config_topic, "my-device"),
            Some("cpu_usage".to_string())
        );
    }

    #[test]
    fn test_extract_discovered_sensor_name_rejects_other_topics() {
        assert_eq!(
            extract_discovered_sensor_name(
                "homeassistant/sensor/orbiq_my-device/cpu_usage/state",
                "my-device"
            ),
            None
        );
        assert_eq!(
            extract_discovered_sensor_name(
                "homeassistant/sensor/orbiq_other-device/cpu_usage/config",
                "my-device"
            ),
            None
        );
    }

    #[test]
    fn test_system_discovery_config_uses_default_entity_id_not_deprecated_object_id() {
        let sensor = SystemSensor::number("cpu_usage", None, 12.3, "%", SystemSensorType::CpuUsage);
        let device_info = DeviceInfo {
            identifiers: vec!["orbiq_my-device".to_string()],
            name: "my-device".to_string(),
            model: "OrbIQ System Monitor".to_string(),
            manufacturer: "OrbIQ".to_string(),
            sw_version: None,
            hw_version: None,
        };

        let payload = system_discovery_config(&sensor, "my-device", &device_info);
        let config: serde_json::Value = serde_json::from_str(&payload.payload).unwrap();

        assert_eq!(config["unique_id"], "orbiq_my-device_cpu_usage");
        assert_eq!(config["default_entity_id"], "sensor.orbiq_my-device_cpu_usage");
        // The "object_id" key is deprecated (HA 2026.4+ ignores it for new entities,
        // silently falling back to auto-slugging from "name") -- must not be sent.
        assert!(config.get("object_id").is_none());
    }

    #[test]
    fn test_stale_sensor_removal_payloads() {
        let payloads = stale_sensor_removal_payloads("cpu_usage", "my-device");
        assert_eq!(payloads.len(), 2);
        for payload in &payloads {
            assert!(payload.payload.is_empty());
            assert!(payload.retain);
        }
        assert!(payloads[0].topic.ends_with("/config"));
        assert!(payloads[1].topic.ends_with("/availability"));
    }

    fn device_info() -> DeviceInfo {
        DeviceInfo {
            identifiers: vec!["orbiq_my-device".to_string()],
            name: "my-device".to_string(),
            model: DEVICE_MODEL.to_string(),
            manufacturer: DEVICE_MANUFACTURER.to_string(),
            sw_version: None,
            hw_version: None,
        }
    }

    #[test]
    fn text_sensor_discovery_has_no_unit_or_state_class() {
        let sensor = SystemSensor::text("cpu_governor", "performance", SystemSensorType::CpuGovernor);
        let config: serde_json::Value =
            serde_json::from_str(&system_discovery_config(&sensor, "my-device", &device_info()).payload).unwrap();
        assert_eq!(config["name"], "CPU Governor");
        assert!(config.get("unit_of_measurement").is_none());
        assert!(config.get("state_class").is_none());
        assert!(config.get("device_class").is_none());
    }

    #[test]
    fn numeric_sensor_discovery_keeps_unit_state_class_and_device_class() {
        let sensor = SystemSensor::number("cpu_max_frequency", None, 5086.0, "MHz", SystemSensorType::CpuFrequency);
        let config: serde_json::Value =
            serde_json::from_str(&system_discovery_config(&sensor, "my-device", &device_info()).payload).unwrap();
        assert_eq!(config["name"], "CPU Max Frequency");
        assert_eq!(config["unit_of_measurement"], "MHz");
        assert_eq!(config["state_class"], "measurement");
        assert_eq!(config["device_class"], "frequency");
    }

    #[test]
    fn state_payload_serialises_numbers_and_text_unquoted_and_quoted() {
        let num = SystemSensor::number("cpu_usage", None, 12.5, "%", SystemSensorType::CpuUsage);
        let txt = SystemSensor::text("cpu_boost", "on", SystemSensorType::CpuBoost);
        assert_eq!(system_state(&num, "d").payload, r#"{"value":12.5}"#);
        assert_eq!(system_state(&txt, "d").payload, r#"{"value":"on"}"#);
        assert_eq!(system_state(&txt, "d").topic, "homeassistant/sensor/orbiq_d/cpu_boost/state");
    }
}
