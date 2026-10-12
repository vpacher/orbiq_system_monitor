use crate::hwmon_devices::{discover_hwmon_devices, HwmonDevice};
use crate::sensors::{SystemSensor, SystemSensorType};
use std::fs;
use std::path::Path;

const INPUT_SUFFIX: &str = "_input";
const LABEL_SUFFIX: &str = "_label";

/// One kind of hwmon `<prefix><N>_input` file. The three kinds differ only in
/// these fields, so a single scan handles all of them.
struct HwmonKind {
    prefix: &'static str,
    /// Appended to `<device>_<N>` to build the sensor name; empty for temperatures,
    /// whose names predate the suffix scheme and must not change (HA entity IDs).
    name_suffix: &'static str,
    unit: &'static str,
    /// Raw value is divided by this (temperatures are reported in millidegrees).
    divisor: f64,
    /// Fans without a `_label` file are skipped, as before the modules were merged.
    label_required: bool,
    sensor_type: SystemSensorType,
}

const KINDS: [HwmonKind; 3] = [
    HwmonKind {
        prefix: "temp",
        name_suffix: "",
        unit: "°C",
        divisor: 1000.0,
        label_required: false,
        sensor_type: SystemSensorType::Temperature,
    },
    HwmonKind {
        prefix: "fan",
        name_suffix: "_fan",
        unit: "RPM",
        divisor: 1.0,
        label_required: true,
        sensor_type: SystemSensorType::Fan,
    },
    // currN_input is already in milliamps.
    HwmonKind {
        prefix: "curr",
        name_suffix: "_current",
        unit: "mA",
        divisor: 1.0,
        label_required: false,
        sensor_type: SystemSensorType::Current,
    },
];

pub fn collect_all_hwmon_sensors() -> Vec<SystemSensor> {
    match discover_hwmon_devices() {
        Ok(devices) => devices.iter().flat_map(scan_device).collect(),
        Err(e) => {
            eprintln!("Failed to discover hwmon devices: {e}");
            Vec::new()
        }
    }
}

fn scan_device(device: &HwmonDevice) -> Vec<SystemSensor> {
    match fs::read_dir(&device.path) {
        Ok(entries) => entries
            .flatten()
            .filter_map(|entry| read_sensor(&entry.path(), &device.name))
            .collect(),
        Err(e) => {
            eprintln!("Failed to read device directory {}: {e}", device.path.display());
            Vec::new()
        }
    }
}

fn read_sensor(file_path: &Path, device_name: &str) -> Option<SystemSensor> {
    let filename = file_path.file_name()?.to_str()?;
    let (kind, id) = classify(filename)?;
    let raw: f64 = fs::read_to_string(file_path).ok()?.trim().parse().ok()?;
    let label = read_label(file_path, filename);
    if kind.label_required && label.is_none() {
        return None;
    }
    Some(SystemSensor::number(
        format!("{device_name}_{id}{}", kind.name_suffix),
        label,
        raw / kind.divisor,
        kind.unit,
        kind.sensor_type,
    ))
}

/// Maps `temp3_input` to (temperature kind, "3"); anything else to `None`.
fn classify(filename: &str) -> Option<(&'static HwmonKind, &str)> {
    let stem = filename.strip_suffix(INPUT_SUFFIX)?;
    KINDS.iter().find_map(|kind| {
        let id = stem.strip_prefix(kind.prefix)?;
        (!id.is_empty()).then_some((kind, id))
    })
}

fn read_label(input_path: &Path, input_filename: &str) -> Option<String> {
    let stem = input_filename.strip_suffix(INPUT_SUFFIX)?;
    let label = fs::read_to_string(input_path.with_file_name(format!("{stem}{LABEL_SUFFIX}"))).ok()?;
    Some(label.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("orbiq_hwmon_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn classify_recognises_the_three_kinds() {
        assert_eq!(classify("temp1_input").map(|(k, id)| (k.prefix, id)), Some(("temp", "1")));
        assert_eq!(classify("fan12_input").map(|(k, id)| (k.prefix, id)), Some(("fan", "12")));
        assert_eq!(classify("curr2_input").map(|(k, id)| (k.prefix, id)), Some(("curr", "2")));
    }

    #[test]
    fn classify_rejects_other_files() {
        for name in ["temp1_max", "temp1_label", "in0_input", "power1_input", "temp_input", "name"] {
            assert!(classify(name).is_none(), "{name} should not be classified");
        }
    }

    #[test]
    fn sensor_names_match_the_pre_merge_scheme() {
        // Existing Home Assistant entity IDs depend on these exact names.
        let dir = fixture_dir("names");
        fs::write(dir.join("temp1_input"), "45125\n").unwrap();
        fs::write(dir.join("fan2_input"), "1200\n").unwrap();
        fs::write(dir.join("fan2_label"), "CPU Fan\n").unwrap();
        fs::write(dir.join("curr1_input"), "350\n").unwrap();

        let device = HwmonDevice { path: dir.clone(), name: "nct6687".to_string() };
        let mut sensors = scan_device(&device);
        sensors.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<&str> = sensors.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["nct6687_1", "nct6687_1_current", "nct6687_2_fan"]);

        let temp = sensors.iter().find(|s| s.name == "nct6687_1").unwrap();
        assert_eq!(temp.value, crate::sensors::SensorValue::Number(45.125));
        assert_eq!(temp.unit.as_deref(), Some("°C"));
        let fan = sensors.iter().find(|s| s.name == "nct6687_2_fan").unwrap();
        assert_eq!(fan.label.as_deref(), Some("CPU Fan"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unlabelled_fans_are_skipped() {
        let dir = fixture_dir("fan_label");
        fs::write(dir.join("fan1_input"), "900\n").unwrap();
        let device = HwmonDevice { path: dir.clone(), name: "dev".to_string() };
        assert!(scan_device(&device).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
}
