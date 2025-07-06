use crate::hwmon_devices::{discover_hwmon_devices, HwmonDevice};
use crate::sensors::SystemSensor;
use crate::sensors::SystemSensorType::Fan;
use std::fs;
use std::path::Path;

const FAN_FILE_PREFIX: &str = "fan";
const FAN_FILE_SUFFIX: &str = "_input";

pub fn collect_all_fans() -> Vec<SystemSensor> {
    let mut sensors = Vec::new();

    match discover_hwmon_devices() {
        Ok(devices) => {
            for device in devices {
                let device_sensors = scan_device_fans(&device);
                sensors.extend(device_sensors);
            }
        }
        Err(e) => {
            eprintln!("Failed to discover hwmon devices: {}", e);
        }
    }

    sensors
}

fn scan_device_fans(device: &HwmonDevice) -> Vec<SystemSensor> {
    let mut sensors = Vec::new();

    match fs::read_dir(&device.path) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Some(sensor) = process_fan_file(&entry.path(), device) {
                    sensors.push(sensor);
                }
            }
        }
        Err(e) => {
            eprintln!(
                "Failed to read device directory {}: {}",
                device.path.display(),
                e
            );
        }
    }

    sensors
}

fn process_fan_file(file_path: &Path, device: &HwmonDevice) -> Option<SystemSensor> {
    let filename = file_path.file_name()?.to_string_lossy();

    if !is_fan_file(&filename) {
        return None;
    }

    let fan_rpm = read_fan_value(file_path)?;
    let fan_label = get_fan_label(file_path)?;
    let fan_id = extract_fan_id(&filename)?;
    let sensor_name = format!("{}_{}_{}", device.name, fan_id, "fan");

    Some(SystemSensor {
        name: sensor_name,
        label: Some(fan_label),
        value: fan_rpm as f64,
        unit: "RPM".parse().unwrap(),
        sensor_type: Fan,
    })
}

fn is_fan_file(filename: &str) -> bool {
    filename.starts_with(FAN_FILE_PREFIX) && filename.ends_with(FAN_FILE_SUFFIX)
}

fn read_fan_value(file_path: &Path) -> Option<f32> {
    let fan_raw = fs::read_to_string(file_path).ok()?;
    let fan_rpm = fan_raw.trim().parse::<f32>().ok()?;
    Some(fan_rpm)
}

fn extract_fan_id(filename: &str) -> Option<String> {
    Some(
        filename
            .replace(FAN_FILE_PREFIX, "")
            .replace(FAN_FILE_SUFFIX, ""),
    )
}


fn get_fan_label(file_path: &Path) -> Option<String> {
    let filename = file_path.file_name()?.to_string_lossy();
    let label_filename = filename.replace("_input", "_label");
    let label_path = file_path.with_file_name(label_filename);
    let label_raw = fs::read_to_string(label_path).ok()?;
    Some(label_raw.trim().to_string())
}
