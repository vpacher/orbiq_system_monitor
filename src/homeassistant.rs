use crate::mqtt_client::MqttPayload;
use crate::system_sensor::{SystemSensor, SystemSensorType};
use crate::temperature_sensor::TemperatureSensor;
use rumqttc::{AsyncClient, QoS};
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
    }
}
pub fn discovery_config(
    sensor: &TemperatureSensor,
    device_name: &str,
    device_info: &DeviceInfo,
) -> MqttPayload {
    let unique_id = format!("orbiq_{}_{}", device_name, sensor.name);
    let object_id = format!("orbiq_{}_{}", device_name, sensor.name);

    // Home Assistant discovery format with node_id support:
    // homeassistant/{component}/{node_id}/{object_id}/config
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

    let friendly_name = generate_friendly_name(&sensor.name);

    let config = json!({
        "name": friendly_name,
        "unique_id": unique_id,
        "object_id": object_id,
        "state_topic": state_topic,
        "unit_of_measurement": "°C",
        "device_class": "temperature",
        "state_class": "measurement",
        "value_template": "{{ value_json.temperature }}",
        "availability": {
            "topic": availability_topic,
            "payload_available": "online",
            "payload_not_available": "offline"
        },
        "device": device_info
    });

    MqttPayload {
        topic: config_topic,
        payload: config.to_string(),
        retain: true,
    }
}

fn topic(data: Topic) -> String {
    format!(
        "homeassistant/sensor/orbiq_{}/{}/{}",
        data.device_name, data.sensor_name, data.sub_topic
    )
}

pub fn temperature_state(sensor: &TemperatureSensor, device_name: &str) -> MqttPayload {
    let topic_data = Topic {
        device_name: device_name.parse().unwrap(),
        sensor_name: sensor.name.clone(),
        sub_topic: "state".to_string(),
    };
    let payload = json!({
        "temperature": sensor.temperature
    });
    MqttPayload {
        topic: topic(topic_data),
        payload: payload.to_string(),
        retain: false,
    }
}
pub fn sensor_availability(
    sensor: &TemperatureSensor,
    device_name: &str,
    available: bool,
) -> MqttPayload {
    let topic_data = Topic {
        device_name: device_name.parse().unwrap(),
        sensor_name: sensor.name.clone(),
        sub_topic: "availability".to_string(),
    };

    MqttPayload {
        topic: topic(topic_data),
        payload: if available {
            "online".parse().unwrap()
        } else {
            "offline".parse().unwrap()
        },
        retain: true,
    }
}

pub async fn publish_sensor_availability(
    client: &AsyncClient,
    sensor: &TemperatureSensor,
    device_name: &str,
    available: bool,
) -> Result<(), rumqttc::ClientError> {
    let availability_topic = format!(
        "homeassistant/sensor/orbiq_{}/{}/availability",
        device_name, sensor.name
    );
    let payload = if available { "online" } else { "offline" };

    client
        .publish(&availability_topic, QoS::AtLeastOnce, true, payload)
        .await
}

pub async fn publish_system_discovery_config(
    client: &AsyncClient,
    sensor: &SystemSensor,
    device_name: &str,
    device_info: &DeviceInfo,
) -> Result<(), rumqttc::ClientError> {
    let unique_id = format!("orbiq_{}_{}", device_name, sensor.name);
    let object_id = format!("orbiq_{}_{}", device_name, sensor.name);

    // Home Assistant discovery format with node_id support:
    // homeassistant/{component}/{node_id}/{object_id}/config
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

    client
        .publish(config_topic, QoS::AtLeastOnce, true, config.to_string())
        .await
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

pub async fn publish_system_sensor_availability(
    client: &AsyncClient,
    sensor: &SystemSensor,
    device_name: &str,
    available: bool,
) -> Result<(), rumqttc::ClientError> {
    let availability_topic = format!(
        "homeassistant/sensor/orbiq_{}/{}/availability",
        device_name, sensor.name
    );
    let payload = if available { "online" } else { "offline" };

    client
        .publish(&availability_topic, QoS::AtLeastOnce, true, payload)
        .await
}
