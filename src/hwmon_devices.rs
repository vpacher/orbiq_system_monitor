use std::fs;
use std::path::{Path, PathBuf};

const HWMON_BASE_PATH: &str = "/sys/class/hwmon";

#[derive(Debug)]
pub struct HwmonDevice {
    pub path: PathBuf,
    pub name: String,
}
pub fn discover_hwmon_devices() -> Result<Vec<HwmonDevice>, std::io::Error> {
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
