use crate::config::DaemonConfig;
use crate::fan_sensors::collect_all_fans;
use crate::gpu_sensors::collect_all_gpu_sensors;
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
    GpuUsage,
    GpuMemoryUsage,
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
            SystemSensorType::GpuUsage => "mdi:expansion-card-variant",
            SystemSensorType::GpuMemoryUsage => "mdi:memory",
        }
    }
}
pub fn get_all_sensors() -> impl Iterator<Item = SystemSensor> {
    let temp_sensors = collect_all_temperatures();
    let system_sensors = collect_system_stats();
    let fan_sensors = collect_all_fans();
    let gpu_sensors = collect_all_gpu_sensors();

    temp_sensors
        .into_iter()
        .chain(system_sensors)
        .chain(fan_sensors)
        .chain(gpu_sensors)
}

pub fn generate_payload(
    sensor: &SystemSensor,
    config: &DaemonConfig,
    device_info: &DeviceInfo,
) -> MqttSensorTopics {
    MqttSensorTopics {
        name: sensor.name.clone(),
        state: system_state(sensor, &config.device.name),
        discovery: system_discovery_config(sensor, &config.device.name, device_info),
        availability: system_sensor_availability(sensor, &config.device.name, true),
    }
}