use rumqttc::{AsyncClient, QoS};
use serde_json::json;
use crate::temperature_sensor::TemperatureSensor;
use crate::system_sensor::{SystemSensor, SystemSensorType};

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
            identifiers: vec![format!("temp_daemon_{}", device_config.name)],
            name: device_config.model.clone()
                .unwrap_or_else(|| format!("{} System Monitor", device_config.name)),
            model: device_config.model.clone()
                .unwrap_or_else(|| "System Monitoring Service".to_string()),
            manufacturer: device_config.manufacturer.clone()
                .unwrap_or_else(|| "Rust System Daemon".to_string()),
            sw_version: device_config.sw_version.clone()
                .or_else(|| Some(env!("CARGO_PKG_VERSION").to_string())),
            hw_version: device_config.hw_version.clone()
                .or_else(|| Some("1.0".to_string())),
        }
    }
}

pub async fn publish_discovery_config(
    client: &AsyncClient,
    sensor: &TemperatureSensor,
    device_name: &str,
    device_info: &DeviceInfo,
) -> Result<(), rumqttc::ClientError> {
    let unique_id = format!("{}_{}_temperature", device_name, sensor.name);
    let state_topic = format!("homeassistant/sensor/{}/state", unique_id);
    let availability_topic = format!("homeassistant/sensor/{}/availability", unique_id);
    
    let config = json!({
        "name": format!("{} Temperature", sensor.name),
        "unique_id": unique_id,
        "state_topic": state_topic,
        "unit_of_measurement": "°C",
        "device_class": "temperature",
        "availability_topic": availability_topic,
        "device": device_info
    });

    let topic = format!("homeassistant/sensor/{}/config", unique_id);
    client.publish(topic, QoS::AtLeastOnce, true, config.to_string()).await
}

pub async fn publish_system_discovery_config(
    client: &AsyncClient,
    sensor: &SystemSensor,
    device_name: &str,
    device_info: &DeviceInfo,
) -> Result<(), rumqttc::ClientError> {
    let unique_id = format!("{}_{}", device_name, sensor.name);
    let state_topic = format!("homeassistant/sensor/{}/state", unique_id);
    let availability_topic = format!("homeassistant/sensor/{}/availability", unique_id);
    
    let device_class = match &sensor.sensor_type {
        SystemSensorType::CpuUsage | SystemSensorType::MemoryUsage | SystemSensorType::DiskUsage => None,
        SystemSensorType::MemoryUsed | SystemSensorType::MemoryTotal | 
        SystemSensorType::DiskUsed | SystemSensorType::DiskTotal => Some("data_size"),
    };

    let mut config = json!({
        "name": format!("{} {}", device_name, sensor.name.replace('_', " ").to_uppercase()),
        "unique_id": unique_id,
        "state_topic": state_topic,
        "unit_of_measurement": sensor.unit,
        "availability_topic": availability_topic,
        "icon": sensor.sensor_type.icon(),
        "device": device_info
    });

    if let Some(class) = device_class {
        config["device_class"] = json!(class);
    }

    let topic = format!("homeassistant/sensor/{}/config", unique_id);
    client.publish(topic, QoS::AtLeastOnce, true, config.to_string()).await
}

pub async fn publish_temperature_state(
    client: &AsyncClient,
    sensor: &TemperatureSensor,
    device_name: &str,
) -> Result<(), rumqttc::ClientError> {
    let unique_id = format!("{}_{}_temperature", device_name, sensor.name);
    let topic = format!("homeassistant/sensor/{}/state", unique_id);
    
    client.publish(topic, QoS::AtLeastOnce, false, sensor.temperature.to_string()).await
}

pub async fn publish_system_state(
    client: &AsyncClient,
    sensor: &SystemSensor,
    device_name: &str,
) -> Result<(), rumqttc::ClientError> {
    let unique_id = format!("{}_{}", device_name, sensor.name);
    let topic = format!("homeassistant/sensor/{}/state", unique_id);
    
    client.publish(topic, QoS::AtLeastOnce, false, format!("{:.2}", sensor.value)).await
}

pub async fn publish_sensor_availability(
    client: &AsyncClient,
    sensor: &TemperatureSensor,
    device_name: &str,
    available: bool,
) -> Result<(), rumqttc::ClientError> {
    let unique_id = format!("{}_{}_temperature", device_name, sensor.name);
    let topic = format!("homeassistant/sensor/{}/availability", unique_id);
    let payload = if available { "online" } else { "offline" };
    
    client.publish(topic, QoS::AtLeastOnce, true, payload).await
}

pub async fn publish_system_sensor_availability(
    client: &AsyncClient,
    sensor: &SystemSensor,
    device_name: &str,
    available: bool,
) -> Result<(), rumqttc::ClientError> {
    let unique_id = format!("{}_{}", device_name, sensor.name);
    let topic = format!("homeassistant/sensor/{}/availability", unique_id);
    let payload = if available { "online" } else { "offline" };
    
    client.publish(topic, QoS::AtLeastOnce, true, payload).await
}