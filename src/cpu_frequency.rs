//! CPU frequency scaling state from the Linux cpufreq sysfs interface: governor,
//! energy-performance preference (EPP), boost, and current/maximum frequency.
//! Each sensor is only reported when its sysfs file exists, so drivers without
//! EPP or boost (e.g. many ARM boards) simply get fewer sensors.

use crate::sensors::{SystemSensor, SystemSensorType};
use std::fs;
use std::path::{Path, PathBuf};

const CPU_SYSFS: &str = "/sys/devices/system/cpu";
/// Reported when cpufreq policies disagree (e.g. a per-core governor tweak).
const MIXED: &str = "mixed";

pub fn collect_cpu_frequency() -> Vec<SystemSensor> {
    collect_from(Path::new(CPU_SYSFS))
}

fn collect_from(cpu_dir: &Path) -> Vec<SystemSensor> {
    let policies = policy_dirs(cpu_dir);
    if policies.is_empty() {
        return Vec::new();
    }

    let mut sensors = Vec::new();
    if let Some(governor) = uniform_value(&policies, "scaling_governor") {
        sensors.push(SystemSensor::text("cpu_governor", governor, SystemSensorType::CpuGovernor));
    }
    if let Some(epp) = uniform_value(&policies, "energy_performance_preference") {
        sensors.push(SystemSensor::text("cpu_energy_preference", epp, SystemSensorType::CpuEnergyPreference));
    }
    if let Some(boost) = boost_state(cpu_dir) {
        let state = if boost { "on" } else { "off" };
        sensors.push(SystemSensor::text("cpu_boost", state, SystemSensorType::CpuBoost));
    }
    let max_khz = read_khz(&policies, "scaling_max_freq");
    if let Some(max) = max_khz.iter().copied().max() {
        sensors.push(frequency_sensor("cpu_max_frequency", max));
    }
    let cur_khz = read_khz(&policies, "scaling_cur_freq");
    if !cur_khz.is_empty() {
        let avg = cur_khz.iter().sum::<u64>() / cur_khz.len() as u64;
        sensors.push(frequency_sensor("cpu_frequency", avg));
    }
    sensors
}

fn frequency_sensor(name: &str, khz: u64) -> SystemSensor {
    #[allow(clippy::cast_precision_loss)] // CPU frequencies in MHz are far below 2^52
    let mhz = (khz as f64 / 1000.0).round();
    SystemSensor::number(name, None, mhz, "MHz", SystemSensorType::CpuFrequency)
}

fn policy_dirs(cpu_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(cpu_dir.join("cpufreq")) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("policy"))
        })
        .collect();
    dirs.sort();
    dirs
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// The value of `file` if every policy that has it agrees, `MIXED` if they differ,
/// `None` if no policy has the file.
fn uniform_value(policies: &[PathBuf], file: &str) -> Option<String> {
    let mut values = policies.iter().filter_map(|p| read_trimmed(&p.join(file)));
    let first = values.next()?;
    if values.all(|v| v == first) {
        Some(first)
    } else {
        Some(MIXED.to_string())
    }
}

fn read_khz(policies: &[PathBuf], file: &str) -> Vec<u64> {
    policies
        .iter()
        .filter_map(|p| read_trimmed(&p.join(file))?.parse().ok())
        .collect()
}

/// Global boost switch: `cpufreq/boost` (acpi-cpufreq, amd-pstate) or the inverted
/// `intel_pstate/no_turbo`.
fn boost_state(cpu_dir: &Path) -> Option<bool> {
    if let Some(v) = read_trimmed(&cpu_dir.join("cpufreq/boost")) {
        return Some(v == "1");
    }
    read_trimmed(&cpu_dir.join("intel_pstate/no_turbo")).map(|v| v == "0")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensors::SensorValue;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("orbiq_cpufreq_{name}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("cpufreq")).unwrap();
            Fixture(dir)
        }

        fn policy(&self, n: u32, files: &[(&str, &str)]) -> &Self {
            let p = self.0.join(format!("cpufreq/policy{n}"));
            fs::create_dir_all(&p).unwrap();
            for (f, v) in files {
                fs::write(p.join(f), format!("{v}\n")).unwrap();
            }
            self
        }

        fn file(&self, rel: &str, v: &str) -> &Self {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, format!("{v}\n")).unwrap();
            self
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn value_of(sensors: &[SystemSensor], name: &str) -> Option<SensorValue> {
        sensors.iter().find(|s| s.name == name).map(|s| s.value.clone())
    }

    fn text(v: &str) -> Option<SensorValue> {
        Some(SensorValue::Text(v.to_string()))
    }

    #[test]
    fn amd_pstate_epp_performance_profile() {
        let f = Fixture::new("amd");
        for n in 0..2 {
            f.policy(n, &[
                ("scaling_governor", "performance"),
                ("energy_performance_preference", "performance"),
                ("scaling_max_freq", "5086181"),
                ("scaling_cur_freq", if n == 0 { "3600000" } else { "4000000" }),
            ]);
        }
        f.file("cpufreq/boost", "1");

        let s = collect_from(&f.0);
        assert_eq!(value_of(&s, "cpu_governor"), text("performance"));
        assert_eq!(value_of(&s, "cpu_energy_preference"), text("performance"));
        assert_eq!(value_of(&s, "cpu_boost"), text("on"));
        assert_eq!(value_of(&s, "cpu_max_frequency"), Some(SensorValue::Number(5086.0)));
        assert_eq!(value_of(&s, "cpu_frequency"), Some(SensorValue::Number(3800.0)));
        assert!(s.iter().filter(|x| x.name.starts_with("cpu_freq") || x.name == "cpu_max_frequency")
            .all(|x| x.unit.as_deref() == Some("MHz")));
        assert!(s.iter().filter(|x| x.is_text()).all(|x| x.unit.is_none()));
    }

    #[test]
    fn powersave_with_boost_off_is_reported() {
        let f = Fixture::new("save");
        f.policy(0, &[("scaling_governor", "powersave"), ("energy_performance_preference", "power"),
                      ("scaling_max_freq", "3401000")]);
        f.file("cpufreq/boost", "0");
        let s = collect_from(&f.0);
        assert_eq!(value_of(&s, "cpu_governor"), text("powersave"));
        assert_eq!(value_of(&s, "cpu_boost"), text("off"));
        assert_eq!(value_of(&s, "cpu_max_frequency"), Some(SensorValue::Number(3401.0)));
    }

    #[test]
    fn differing_governors_are_reported_as_mixed() {
        let f = Fixture::new("mixed");
        f.policy(0, &[("scaling_governor", "performance")]);
        f.policy(1, &[("scaling_governor", "powersave")]);
        assert_eq!(value_of(&collect_from(&f.0), "cpu_governor"), text(MIXED));
    }

    #[test]
    fn intel_no_turbo_is_inverted() {
        let f = Fixture::new("intel");
        f.policy(0, &[("scaling_governor", "powersave")]);
        f.file("intel_pstate/no_turbo", "1");
        assert_eq!(value_of(&collect_from(&f.0), "cpu_boost"), text("off"));
    }

    #[test]
    fn missing_files_produce_no_sensor() {
        // e.g. cpufreq-dt on a Raspberry Pi: no EPP, no boost switch.
        let f = Fixture::new("arm");
        f.policy(0, &[("scaling_governor", "ondemand"), ("scaling_cur_freq", "1500000")]);
        let s = collect_from(&f.0);
        let names: Vec<&str> = s.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["cpu_governor", "cpu_frequency"]);
    }

    #[test]
    fn no_cpufreq_directory_means_no_sensors() {
        let dir = std::env::temp_dir().join(format!("orbiq_cpufreq_none_{}", std::process::id()));
        assert!(collect_from(&dir).is_empty());
    }
}
