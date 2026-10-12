use crate::config::DaemonConfig;
use crate::cpu_frequency::collect_cpu_frequency;
use crate::gpu_sensors::collect_all_gpu_sensors;
use crate::homeassistant::{
    system_discovery_config, system_sensor_availability, system_state, DeviceInfo,
};
use crate::hwmon_sensors::collect_all_hwmon_sensors;
use crate::mqtt_client::MqttSensorTopics;
use crate::system_sensor::collect_system_stats;
use serde::Serialize;

/// A sensor reading is either a measurement (published with a unit and
/// `state_class: measurement`) or a plain text state such as a CPU governor name.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SensorValue {
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone)]
pub struct SystemSensor {
    pub name: String,
    pub label: Option<String>,
    pub value: SensorValue,
    /// `None` for text sensors, which have no unit in Home Assistant.
    pub unit: Option<String>,
    pub sensor_type: SystemSensorType,
}

impl SystemSensor {
    pub fn number(
        name: impl Into<String>,
        label: Option<String>,
        value: f64,
        unit: &str,
        sensor_type: SystemSensorType,
    ) -> Self {
        Self {
            name: name.into(),
            label,
            value: SensorValue::Number(value),
            unit: Some(unit.to_string()),
            sensor_type,
        }
    }

    pub fn text(name: impl Into<String>, value: impl Into<String>, sensor_type: SystemSensorType) -> Self {
        Self {
            name: name.into(),
            label: None,
            value: SensorValue::Text(value.into()),
            unit: None,
            sensor_type,
        }
    }

    pub fn is_text(&self) -> bool {
        matches!(self.value, SensorValue::Text(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Current,
    GpuUsage,
    GpuMemoryUsage,
    CpuFrequency,
    CpuGovernor,
    CpuEnergyPreference,
    CpuBoost,
}

// All per-type Home Assistant metadata lives here, so adding a sensor type means
// extending this enum and its matches rather than editing several modules.
impl SystemSensorType {
    pub fn icon(self) -> &'static str {
        match self {
            SystemSensorType::CpuUsage => "mdi:cpu-64-bit",
            SystemSensorType::MemoryUsage
            | SystemSensorType::MemoryUsed
            | SystemSensorType::MemoryTotal
            | SystemSensorType::GpuMemoryUsage => "mdi:memory",
            SystemSensorType::DiskUsage
            | SystemSensorType::DiskUsed
            | SystemSensorType::DiskTotal => "mdi:harddisk",
            SystemSensorType::Temperature => "mdi:thermometer",
            SystemSensorType::Fan => "mdi:fan",
            SystemSensorType::Current => "mdi:current-dc",
            SystemSensorType::GpuUsage => "mdi:expansion-card-variant",
            SystemSensorType::CpuFrequency => "mdi:sine-wave",
            SystemSensorType::CpuGovernor | SystemSensorType::CpuEnergyPreference => "mdi:speedometer",
            SystemSensorType::CpuBoost => "mdi:rocket-launch",
        }
    }

    pub fn device_class(self) -> Option<&'static str> {
        match self {
            SystemSensorType::MemoryUsed
            | SystemSensorType::MemoryTotal
            | SystemSensorType::DiskUsed
            | SystemSensorType::DiskTotal => Some("data_size"),
            SystemSensorType::Temperature => Some("temperature"),
            SystemSensorType::Current => Some("current"),
            SystemSensorType::CpuFrequency => Some("frequency"),
            SystemSensorType::CpuUsage
            | SystemSensorType::MemoryUsage
            | SystemSensorType::DiskUsage
            | SystemSensorType::GpuUsage
            | SystemSensorType::GpuMemoryUsage
            | SystemSensorType::Fan
            | SystemSensorType::CpuGovernor
            | SystemSensorType::CpuEnergyPreference
            | SystemSensorType::CpuBoost => None,
        }
    }
}

pub fn get_all_sensors() -> impl Iterator<Item = SystemSensor> {
    collect_all_hwmon_sensors()
        .into_iter()
        .chain(collect_system_stats())
        .chain(collect_cpu_frequency())
        .chain(collect_all_gpu_sensors())
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
