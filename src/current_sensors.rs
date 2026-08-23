use crate::hwmon_devices::{discover_hwmon_devices, HwmonDevice};
use crate::sensors::SystemSensor;
use crate::sensors::SystemSensorType::Current;
use std::fs;
use std::path::Path;

const CURRENT_FILE_PREFIX: &str = "curr";
const CURRENT_FILE_SUFFIX: &str = "_input";

pub fn collect_all_currents() -> Vec<SystemSensor> {
    let mut sensors = Vec::new();

    match discover_hwmon_devices() {
        Ok(devices) => {
            for device in devices {
                let device_sensors = scan_device_currents(&device);
                sensors.extend(device_sensors);
            }
        }
        Err(e) => {
            eprintln!("Failed to discover hwmon devices: {}", e);
        }
    }

    sensors
}

fn scan_device_currents(device: &HwmonDevice) -> Vec<SystemSensor> {
    let mut sensors = Vec::new();

    match fs::read_dir(&device.path) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Some(sensor) = process_current_file(&entry.path(), device) {
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

fn process_current_file(file_path: &Path, device: &HwmonDevice) -> Option<SystemSensor> {
    let filename = file_path.file_name()?.to_string_lossy();

    if !is_current_file(&filename) {
        return None;
    }

    let current_ma = read_current_value(file_path)?;
    let current_id = extract_current_id(&filename)?;
    let sensor_name = format!("{}_{}_{}", device.name, current_id, "current");
    let label = get_current_label(file_path);

    Some(SystemSensor {
        name: sensor_name,
        label,
        value: current_ma as f64,
        unit: "mA".parse().unwrap(),
        sensor_type: Current,
    })
}

fn is_current_file(filename: &str) -> bool {
    filename.starts_with(CURRENT_FILE_PREFIX) && filename.ends_with(CURRENT_FILE_SUFFIX)
}

// hwmon's currX_input is already reported in milliamps, unlike tempX_input's millidegrees.
fn read_current_value(file_path: &Path) -> Option<f32> {
    let current_raw = fs::read_to_string(file_path).ok()?;
    current_raw.trim().parse::<f32>().ok()
}

fn extract_current_id(filename: &str) -> Option<String> {
    Some(
        filename
            .replace(CURRENT_FILE_PREFIX, "")
            .replace(CURRENT_FILE_SUFFIX, ""),
    )
}

fn get_current_label(file_path: &Path) -> Option<String> {
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
    fn test_is_current_file() {
        assert!(is_current_file("curr1_input"));
        assert!(is_current_file("curr2_input"));
        assert!(!is_current_file("curr1_max"));
        assert!(!is_current_file("temp1_input"));
        assert!(!is_current_file("fan1_input"));
    }

    #[test]
    fn test_extract_current_id() {
        assert_eq!(extract_current_id("curr1_input"), Some("1".to_string()));
        assert_eq!(extract_current_id("curr12_input"), Some("12".to_string()));
    }
}
