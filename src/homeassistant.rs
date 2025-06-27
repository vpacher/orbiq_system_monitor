use crate::config::DeviceConfig;
use crate::temperature_sensor::TemperatureSensor;
use rumqttc::{AsyncClient, QoS};

#[derive(Debug)]
pub struct HomeAssistantConfig {
    pub unique_id: String,
    pub state_topic: String,
    pub config_topic: String,
    pub availability_topic: String,
    pub friendly_name: String,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub identifiers: Vec<String>,
    pub name: String,
    pub model: String,
    pub manufacturer: String,
    pub sw_version: Option<String>,
    pub hw_version: Option<String>,
}

impl DeviceInfo {
    pub fn from_config(config: &DeviceConfig) -> Self {
        Self {
            identifiers: vec![format!("temp_daemon_{}", config.name)],
            name: format!("{} Temperature Monitor", config.name),
            model: config
                .model
                .clone()
                .unwrap_or_else(|| "Temperature Monitoring System".to_string()),
            manufacturer: config
                .manufacturer
                .clone()
                .unwrap_or_else(|| "Rust Temperature Daemon".to_string()),
            sw_version: config.sw_version.clone(),
            hw_version: config.hw_version.clone(),
        }
    }
}

impl HomeAssistantConfig {
    pub fn new(device_name: &str, sensor: &TemperatureSensor) -> Self {
        let unique_id = format!("temp_daemon_{}_{}", device_name, sensor.name);
        let object_id = format!(
            "{}_{}_temperature",
            device_name,
            sensor.name.replace("_", "")
        );
        let state_topic = format!("homeassistant/sensor/{}/state", object_id);
        let config_topic = format!("homeassistant/sensor/{}/config", object_id);
        let availability_topic = format!("homeassistant/sensor/{}/availability", object_id);

        let friendly_name = Self::generate_friendly_name(&sensor.name);

        Self {
            unique_id,
            state_topic,
            config_topic,
            availability_topic,
            friendly_name,
        }
    }

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
            _ => format!("{} Temperature", sensor_name.replace("_", " ")),
        }
    }
}

pub fn create_discovery_payload(config: &HomeAssistantConfig, device_info: &DeviceInfo) -> String {
    let device_json = format!(
        r#"{{
        "identifiers": [{}],
        "name": "{}",
        "model": "{}",
        "manufacturer": "{}",
        "sw_version": "{}",
        "hw_version": "{}"
    }}"#,
        device_info
            .identifiers
            .iter()
            .map(|id| format!("\"{}\"", id))
            .collect::<Vec<_>>()
            .join(","),
        device_info.name,
        device_info.model,
        device_info.manufacturer,
        device_info
            .sw_version
            .as_ref()
            .unwrap_or(&"unknown".to_string()),
        device_info
            .hw_version
            .as_ref()
            .unwrap_or(&"unknown".to_string())
    );

    format!(
        r#"{{
        "name": "{}",
        "unique_id": "{}",
        "state_topic": "{}",
        "device_class": "temperature",
        "unit_of_measurement": "°C",
        "state_class": "measurement",
        "value_template": "{{{{ value_json.temperature }}}}",
        "availability_topic": "{}",
        "payload_available": "online",
        "payload_not_available": "offline",
        "device": {}
    }}"#,
        config.friendly_name,
        config.unique_id,
        config.state_topic,
        config.availability_topic,
        device_json
    )
}

pub fn create_temperature_state_payload(temperature: f32) -> String {
    format!("{{\"temperature\": {:.2}}}", temperature)
}

pub async fn publish_sensor_availability(
    client: &AsyncClient,
    sensor: &TemperatureSensor,
    device_name: &str,
    available: bool,
) -> Result<(), rumqttc::ClientError> {
    let config = HomeAssistantConfig::new(device_name, sensor);
    let payload = if available { "online" } else { "offline" };

    client
        .publish(&config.availability_topic, QoS::AtLeastOnce, true, payload)
        .await
}

pub async fn publish_discovery_config(
    client: &AsyncClient,
    sensor: &TemperatureSensor,
    device_name: &str,
    device_info: &DeviceInfo,
) -> Result<(), rumqttc::ClientError> {
    let config = HomeAssistantConfig::new(device_name, sensor);
    let discovery_payload = create_discovery_payload(&config, device_info);

    // First publish the sensor as available
    if let Err(e) = publish_sensor_availability(client, sensor, device_name, true).await {
        eprintln!("Failed to publish sensor availability: {}", e);
    }

    match client
        .publish(
            &config.config_topic,
            QoS::AtLeastOnce,
            true,
            discovery_payload,
        )
        .await
    {
        Ok(_) => {
            println!(
                "Discovery config published for {} ({})",
                config.friendly_name, sensor.name
            );
            Ok(())
        }
        Err(e) => {
            eprintln!(
                "Failed to publish discovery config for {}: {}",
                sensor.name, e
            );
            Err(e)
        }
    }
}

pub async fn publish_temperature_state(
    client: &AsyncClient,
    sensor: &TemperatureSensor,
    device_name: &str,
) -> Result<(), rumqttc::ClientError> {
    let config = HomeAssistantConfig::new(device_name, sensor);
    let payload = create_temperature_state_payload(sensor.temperature);

    println!("Publishing to {}: {}", config.state_topic, payload);

    // Publish both state and availability
    let state_result = client
        .publish(&config.state_topic, QoS::AtLeastOnce, false, payload)
        .await;
    let availability_result = publish_sensor_availability(client, sensor, device_name, true).await;

    match (state_result, availability_result) {
        (Ok(_), Ok(_)) => {
            println!("State published successfully for {}", config.friendly_name);
            Ok(())
        }
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("Failed to publish for {}: {}", sensor.name, e);
            Err(e)
        }
    }
}
