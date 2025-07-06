use std::fs;
use std::path::{Path};
use crate::hwmon_devices::{discover_hwmon_devices, HwmonDevice};
use crate::sensors::SystemSensor;
use crate::sensors::SystemSensorType::Temperature;

const TEMP_FILE_PREFIX: &str = "temp";
const TEMP_FILE_SUFFIX: &str = "_input";
const MILLIDEGREE_TO_CELSIUS: f32 = 1000.0;


pub fn collect_all_temperatures() -> Vec<SystemSensor> {
    let mut sensors = Vec::new();

    match discover_hwmon_devices() {
        Ok(devices) => {
            for device in devices {
                let device_sensors = scan_device_temperatures(&device);
                sensors.extend(device_sensors);
            }
        }
        Err(e) => {
            eprintln!("Failed to discover hwmon devices: {}", e);
        }
    }

    sensors
}

fn scan_device_temperatures(device: &HwmonDevice) -> Vec<SystemSensor> {
    let mut sensors = Vec::new();

    match fs::read_dir(&device.path) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Some(sensor) = process_temperature_file(&entry.path(), device) {
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

fn process_temperature_file(file_path: &Path, device: &HwmonDevice) -> Option<SystemSensor> {
    let filename = file_path.file_name()?.to_string_lossy();

    if !is_temperature_file(&filename) {
        return None;
    }

    let temperature = read_temperature_value(file_path)?;
    let temp_number = extract_temperature_number(&filename)?;
    let sensor_name = format!("{}_{}", device.name, temp_number);
    let label = get_temperature_label(file_path);
    
    Some(SystemSensor {
        name: sensor_name,
        label,
        value: temperature as f64,
        unit: "°C".parse().unwrap(),
        sensor_type: Temperature,
    })
}

fn is_temperature_file(filename: &str) -> bool {
    filename.starts_with(TEMP_FILE_PREFIX) && filename.ends_with(TEMP_FILE_SUFFIX)
}

fn read_temperature_value(file_path: &Path) -> Option<f32> {
    let temp_raw = fs::read_to_string(file_path).ok()?;
    let temp_millidegrees = temp_raw.trim().parse::<f32>().ok()?;
    Some(temp_millidegrees / MILLIDEGREE_TO_CELSIUS)
}

fn extract_temperature_number(filename: &str) -> Option<String> {
    Some(
        filename
            .replace(TEMP_FILE_PREFIX, "")
            .replace(TEMP_FILE_SUFFIX, ""),
    )
}

fn get_temperature_label(file_path: &Path) -> Option<String> {
    let filename = file_path.file_name()?.to_string_lossy();
    let label_filename = filename.replace("_input", "_label");
    let label_path = file_path.with_file_name(label_filename);
    let label_raw = fs::read_to_string(label_path).ok()?;
    Some(label_raw.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_temperature_file() {
        assert!(is_temperature_file("temp1_input"));
        assert!(is_temperature_file("temp2_input"));
        assert!(!is_temperature_file("temp1_max"));
        assert!(!is_temperature_file("fan1_input"));
    }

    #[test]
    fn test_extract_temperature_number() {
        assert_eq!(
            extract_temperature_number("temp1_input"),
            Some("1".to_string())
        );
        assert_eq!(
            extract_temperature_number("temp12_input"),
            Some("12".to_string())
        );
    }
}
