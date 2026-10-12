use std::cell::RefCell;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};
use crate::sensors::{SystemSensor, SystemSensorType};

// Helper function to round to specified decimal places
fn round_to_decimals(value: f64, decimals: i32) -> f64 {
    let multiplier = 10_f64.powi(decimals);
    (value * multiplier).round() / multiplier
}

thread_local! {
    // Kept alive across calls (rather than recreated with System::new_all() each time) so
    // CPU usage is computed from a real time delta between cycles instead of two back-to-back
    // refreshes, and so we don't pay for a full process/disk/network enumeration every cycle.
    static SYSTEM: RefCell<System> = RefCell::new(System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    ));
}

#[allow(clippy::cast_precision_loss)] // byte counts in GB: f64 precision is ample
fn bytes_to_gb(bytes: u64) -> f64 {
    round_to_decimals(bytes as f64 / (1024.0 * 1024.0 * 1024.0), 2)
}

#[allow(clippy::cast_precision_loss)]
fn percent(part: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    round_to_decimals(part as f64 / total as f64 * 100.0, 1)
}

pub fn collect_system_stats() -> Vec<SystemSensor> {
    let (cpu_usage, total_memory, used_memory) = SYSTEM.with(|system| {
        let mut system = system.borrow_mut();
        system.refresh_cpu_usage();
        system.refresh_memory();
        (system.global_cpu_usage(), system.total_memory(), system.used_memory())
    });

    let mut sensors = vec![
        SystemSensor::number("cpu_usage", None, round_to_decimals(f64::from(cpu_usage), 1), "%", SystemSensorType::CpuUsage),
        SystemSensor::number("memory_usage", None, percent(used_memory, total_memory), "%", SystemSensorType::MemoryUsage),
        SystemSensor::number("memory_used", None, bytes_to_gb(used_memory), "GB", SystemSensorType::MemoryUsed),
        SystemSensor::number("memory_total", None, bytes_to_gb(total_memory), "GB", SystemSensorType::MemoryTotal),
    ];

    // Disk usage for all mounted disks
    let disks = Disks::new_with_refreshed_list();
    for disk in &disks {
        let mount_point = disk.mount_point().to_string_lossy();
        let name_suffix = if mount_point == "/" {
            "root".to_string()
        } else {
            mount_point.replace(['/', ' '], "_").trim_matches('_').to_string()
        };

        let total_space = disk.total_space();
        // saturating: some filesystems (e.g. fuse/overlay) can report available > total.
        let used_space = total_space.saturating_sub(disk.available_space());

        sensors.push(SystemSensor::number(
            format!("disk_usage_{name_suffix}"), None, percent(used_space, total_space), "%", SystemSensorType::DiskUsage,
        ));
        sensors.push(SystemSensor::number(
            format!("disk_used_{name_suffix}"), None, bytes_to_gb(used_space), "GB", SystemSensorType::DiskUsed,
        ));
        sensors.push(SystemSensor::number(
            format!("disk_total_{name_suffix}"), None, bytes_to_gb(total_space), "GB", SystemSensorType::DiskTotal,
        ));
    }

    sensors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_handles_zero_total_and_rounds() {
        assert_eq!(percent(1, 0), 0.0);
        assert_eq!(percent(1, 3), 33.3);
    }

    #[test]
    fn bytes_to_gb_rounds_to_two_decimals() {
        assert_eq!(bytes_to_gb(1024 * 1024 * 1024), 1.0);
        assert_eq!(bytes_to_gb(1536 * 1024 * 1024), 1.5);
    }
}
