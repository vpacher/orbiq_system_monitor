use std::fs;
use std::path::{Path, PathBuf};

const HWMON_BASE_PATH: &str = "/sys/class/hwmon";
const TEMP_FILE_PREFIX: &str = "temp";
const TEMP_FILE_SUFFIX: &str = "_input";
const MILLIDEGREE_TO_CELSIUS: f32 = 1000.0;

#[derive(Debug, Clone)]
pub struct TemperatureSensor {
    pub name: String,
    pub temperature: f32,
    pub file_path: PathBuf,
}

#[derive(Debug)]
pub struct HwmonDevice {
    pub path: PathBuf,
    pub name: String,
}

pub fn collect_all_temperatures() -> Vec<TemperatureSensor> {
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

fn discover_hwmon_devices() -> Result<Vec<HwmonDevice>, std::io::Error> {
    let mut devices = Vec::new();

    for entry in fs::read_dir(HWMON_BASE_PATH)? {
        let entry = entry?;
        let hwmon_path = entry.path();

        if let Some(hwmon_name) = hwmon_path.file_name() {
            let hwmon_name = hwmon_name.to_string_lossy().to_string();
            let device_name = read_device_name(&hwmon_path).unwrap_or_else(|| hwmon_name);

            devices.push(HwmonDevice {
                path: hwmon_path,
                name: device_name,
            });
        }
    }

    Ok(devices)
}

fn read_device_name(hwmon_path: &Path) -> Option<String> {
    let name_file = hwmon_path.join("name");
    fs::read_to_string(&name_file)
        .ok()
        .map(|content| content.trim().to_string())
}

fn scan_device_temperatures(device: &HwmonDevice) -> Vec<TemperatureSensor> {
    let mut sensors = Vec::new();

    match fs::read_dir(&device.path) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Some(sensor) = process_temperature_file(&entry.path(), device) {
                    println!(
                        "Found temperature: {} = {:.2}°C (from {})",
                        sensor.name,
                        sensor.temperature,
                        sensor.file_path.display()
                    );
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

fn process_temperature_file(file_path: &Path, device: &HwmonDevice) -> Option<TemperatureSensor> {
    let filename = file_path.file_name()?.to_string_lossy();

    if !is_temperature_file(&filename) {
        return None;
    }

    let temperature = read_temperature_value(file_path)?;
    let temp_number = extract_temperature_number(&filename)?;
    let sensor_name = format!("{}_{}", device.name, temp_number);

    Some(TemperatureSensor {
        name: sensor_name,
        temperature,
        file_path: file_path.to_path_buf(),
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