use crate::mqtt_client::MqttPayload;
use crate::sensors::{SystemSensor, SystemSensorType};
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

#[derive(Debug, Clone)]
pub struct Topic {
    sensor_name: String,
    device_name: String,
    sub_topic: String,
}

impl DeviceInfo {
    pub fn from_config(device_config: &crate::config::DeviceConfig) -> Self {
        Self {
            identifiers: vec![format!("orbiq_{}", device_config.name)],
            name: device_config.name.clone(),
            model: "OrbIQ System Monitor".to_string(),
            manufacturer: "OrbIQ".to_string(),
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

// Generate friendly names for temperature sensors
fn generate_friendly_name(sensor_name: &str) -> String {
    match sensor_name {
        name if name.contains("k10temp") => "CPU Temperature".to_string(),
        name if name.contains("nouveau") => "GPU Temperature".to_string(),
        name if name.contains("nvme") => format!(
            "NVMe {} Temperature",
            name.split('_').last().unwrap_or("Unknown")
        ),
        name if name.contains("coretemp") => format!(
            "Core {} Temperature",
            name.split('_').last().unwrap_or("Unknown")
        ),
        name if name.contains("acpi") => "System Temperature".to_string(),
        name if name.contains("amdgpu") => "AMD GPU Temperature".to_string(),
        name if name.contains("radeon") => "Radeon GPU Temperature".to_string(),
        name if name.contains("asus") => "ASUS Sensor Temperature".to_string(),
        name if name.contains("iwlwifi") => "WiFi Module Temperature".to_string(),
        name if name.contains("thermal") => "Thermal Zone Temperature".to_string(),
        _ => format!("{} Temperature", sensor_name.replace("_", " ")),
    }
}

fn generate_friendly_name_for_fan(sensor: &SystemSensor) -> String {
    match &sensor.label {
        Some(label) => label.to_string(),
        None => format!("Fan {}", sensor.name),
    }
}
// Generate friendly names for system sensors
fn generate_system_friendly_name(sensor: &SystemSensor) -> String {
    match &sensor.sensor_type {
        SystemSensorType::CpuUsage => "CPU Usage".to_string(),
        SystemSensorType::MemoryUsage => "Memory Usage".to_string(),
        SystemSensorType::MemoryUsed => "Memory Used".to_string(),
        SystemSensorType::MemoryTotal => "Memory Total".to_string(),
        SystemSensorType::DiskUsage => {
            if sensor.name.contains("root") {
                "Disk Usage (Root)".to_string()
            } else {
                let mount_name = sensor.name.replace("disk_usage_", "").replace("_", " ");
                format!("Disk Usage ({})", mount_name.to_uppercase())
            }
        }
        SystemSensorType::DiskUsed => {
            if sensor.name.contains("root") {
                "Disk Used (Root)".to_string()
            } else {
                let mount_name = sensor.name.replace("disk_used_", "").replace("_", " ");
                format!("Disk Used ({})", mount_name.to_uppercase())
            }
        }
        SystemSensorType::DiskTotal => {
            if sensor.name.contains("root") {
                "Disk Total (Root)".to_string()
            } else {
                let mount_name = sensor.name.replace("disk_total_", "").replace("_", " ");
                format!("Disk Total ({})", mount_name.to_uppercase())
            }
        }
        SystemSensorType::Fan => generate_friendly_name_for_fan(sensor),
        SystemSensorType::Temperature => generate_friendly_name(&sensor.name),
    }
}

fn topic(data: Topic) -> String {
    format!(
        "homeassistant/sensor/orbiq_{}/{}/{}",
        data.device_name, data.sensor_name, data.sub_topic
    )
}

pub fn system_state(sensor: &SystemSensor, device_name: &str) -> MqttPayload {
    let topic_data = Topic {
        device_name: device_name.parse().unwrap(),
        sensor_name: sensor.name.clone(),
        sub_topic: "state".to_string(),
    };

    let payload = json!({
        "value": sensor.value
    });
    MqttPayload {
        topic: topic(topic_data),
        payload: payload.to_string(),
        retain: false,
    }
}

pub fn system_sensor_availability(
    sensor: &SystemSensor,
    device_name: &str,
    available: bool,
) -> MqttPayload {
    let topic_data = Topic {
        device_name: device_name.parse().unwrap(),
        sensor_name: sensor.name.clone(),
        sub_topic: "availability".to_string(),
    };
    let payload = if available { "online" } else { "offline" };
    MqttPayload {
        topic: topic(topic_data),
        payload: payload.parse().unwrap(),
        retain: true,
    }
}

pub fn system_discovery_config(
    sensor: &SystemSensor,
    device_name: &str,
    device_info: &DeviceInfo,
) -> MqttPayload {
    let unique_id = format!("orbiq_{}_{}", device_name, sensor.name);
    let object_id = format!("orbiq_{}_{}", device_name, sensor.name);
    let config_topic = format!(
        "homeassistant/sensor/orbiq_{}/{}/config",
        device_name, sensor.name
    );
    let state_topic = format!(
        "homeassistant/sensor/orbiq_{}/{}/state",
        device_name, sensor.name
    );
    let availability_topic = format!(
        "homeassistant/sensor/orbiq_{}/{}/availability",
        device_name, sensor.name
    );
    let device_class = match &sensor.sensor_type {
        SystemSensorType::CpuUsage
        | SystemSensorType::MemoryUsage
        | SystemSensorType::DiskUsage => None,
        SystemSensorType::MemoryUsed
        | SystemSensorType::MemoryTotal
        | SystemSensorType::DiskUsed
        | SystemSensorType::DiskTotal => Some("data_size"),
        SystemSensorType::Temperature => Some("temperature"),
        SystemSensorType::Fan => None,
    };

    let friendly_name = generate_system_friendly_name(sensor);

    let mut config = json!({
        "name": friendly_name,
        "unique_id": unique_id,
        "object_id": object_id,
        "state_topic": state_topic,
        "unit_of_measurement": sensor.unit,
        "state_class": "measurement",
        "value_template": "{{ value_json.value }}",
        "availability": {
            "topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline"
        },
        "icon": sensor.sensor_type.icon(),
        "device": device_info
    });

    if let Some(class) = device_class {
        config["device_class"] = json!(class);
    }
    MqttPayload {
        topic: config_topic,
        payload: config.to_string(),
        retain: true,
    }
}
