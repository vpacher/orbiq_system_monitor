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
            let device_name = read_device_name(&hwmon_path).unwrap_or(hwmon_name);

            devices.push(HwmonDevice {
                path: hwmon_path,
                name: sanitize_name(&device_name),
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

/// hwmon device names come straight from the kernel (some drivers embed things
/// like a network interface name, which can contain colons) and flow directly
/// into MQTT topic segments and HA entity/unique IDs downstream. Replace
/// anything outside [A-Za-z0-9_-] with '_' so a raw kernel string can never
/// produce an invalid topic (e.g. `r8169_0_2a00:00` -> `r8169_0_2a00_00`).
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name_leaves_safe_names_untouched() {
        assert_eq!(sanitize_name("nvme"), "nvme");
        assert_eq!(sanitize_name("k10temp"), "k10temp");
        assert_eq!(sanitize_name("nct6687"), "nct6687");
    }

    #[test]
    fn test_sanitize_name_replaces_illegal_characters() {
        assert_eq!(sanitize_name("r8169_0_2a00:00"), "r8169_0_2a00_00");
        assert_eq!(sanitize_name("weird/name with spaces"), "weird_name_with_spaces");
    }
}
