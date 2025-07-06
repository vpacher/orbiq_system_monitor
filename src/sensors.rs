use crate::config::DaemonConfig;
use crate::fan_sensors::collect_all_fans;
use crate::homeassistant::{
    system_discovery_config, system_sensor_availability, system_state, DeviceInfo,
};
use crate::mqtt_client::MqttSensorTopics;
use crate::system_sensor::collect_system_stats;
use crate::temperature_sensor::collect_all_temperatures;

#[derive(Debug, Clone)]
pub struct SystemSensor {
    pub name: String,
    pub label: Option<String>,
    pub value: f64,
    pub unit: String,
    pub sensor_type: SystemSensorType,
}

#[derive(Debug, Clone)]
pub enum SystemSensorType {
    CpuUsage,
    MemoryUsage,
    DiskUsage,
    MemoryUsed,
    MemoryTotal,
    DiskUsed,
    DiskTotal,
    Temperature,
    Fan,
}

impl SystemSensorType {
    pub fn icon(&self) -> &str {
        match self {
            SystemSensorType::CpuUsage => "mdi:cpu-64-bit",
            SystemSensorType::MemoryUsage
            | SystemSensorType::MemoryUsed
            | SystemSensorType::MemoryTotal => "mdi:memory",
            SystemSensorType::DiskUsage
            | SystemSensorType::DiskUsed
            | SystemSensorType::DiskTotal => "mdi:harddisk",
            SystemSensorType::Temperature => "mdi:thermometer",
            SystemSensorType::Fan => "mdi:fan",
        }
    }
}
pub fn get_all_sensors() -> Vec<SystemSensor> {
    let temp_sensors = collect_all_temperatures();
    let system_sensors = collect_system_stats();
    let fan_sensors = collect_all_fans();

    temp_sensors.into_iter().chain(system_sensors).chain(fan_sensors).collect()
}

pub fn generate_payloads<'a>(
    sensors: &'a [SystemSensor],
    config: &'a DaemonConfig,
    device_info: &'a DeviceInfo,
) -> impl Iterator<Item = MqttSensorTopics> + 'a {
    sensors.iter().map(move |sensor| MqttSensorTopics {
        name: sensor.name.clone(),
        state: system_state(sensor, &config.device.name),
        discovery: system_discovery_config(sensor, &config.device.name, device_info),
        availability: system_sensor_availability(sensor, &config.device.name, true),
    })
}
