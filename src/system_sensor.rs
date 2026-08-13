use std::cell::RefCell;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};
use crate::sensors::{SystemSensor, SystemSensorType};

// Helper function to round to specified decimal places
fn round_to_decimals(value: f64, decimals: u32) -> f64 {
    let multiplier = 10_f64.powi(decimals as i32);
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

pub fn collect_system_stats() -> Vec<SystemSensor> {
    let mut sensors = Vec::new();

    let (cpu_usage, total_memory, used_memory) = SYSTEM.with(|system| {
        let mut system = system.borrow_mut();
        system.refresh_cpu_usage();
        system.refresh_memory();
        (system.global_cpu_usage(), system.total_memory(), system.used_memory())
    });

    // CPU usage (overall) - rounded to 1 decimal place
    sensors.push(SystemSensor {
        name: "cpu_usage".to_string(),
        label: None,
        value: round_to_decimals(cpu_usage as f64, 1),
        unit: "%".to_string(),
        sensor_type: SystemSensorType::CpuUsage,
    });

    // Memory usage - rounded to 1 decimal place
    let memory_usage_percent = if total_memory > 0 {
        let percent = (used_memory as f64 / total_memory as f64) * 100.0;
        round_to_decimals(percent, 1)
    } else {
        0.0
    };

    sensors.push(SystemSensor {
        name: "memory_usage".to_string(),
        label: None,
        value: memory_usage_percent,
        unit: "%".to_string(),
        sensor_type: SystemSensorType::MemoryUsage,
    });

    sensors.push(SystemSensor {
        name: "memory_used".to_string(),
        label: None,
        value: round_to_decimals((used_memory as f64) / (1024.0 * 1024.0 * 1024.0), 2),
        unit: "GB".to_string(),
        sensor_type: SystemSensorType::MemoryUsed,
    });

    sensors.push(SystemSensor {
        name: "memory_total".to_string(),
        label: None,
        value: round_to_decimals((total_memory as f64) / (1024.0 * 1024.0 * 1024.0), 2),
        unit: "GB".to_string(),
        sensor_type: SystemSensorType::MemoryTotal,
    });

    // Disk usage for all mounted disks
    let disks = Disks::new_with_refreshed_list();
    for disk in &disks {
        let mount_point = disk.mount_point().to_string_lossy();
        let name_suffix = if mount_point == "/" {
            "root".to_string()
        } else {
            mount_point
                .replace(['/', ' '], "_")
                .trim_matches('_')
                .to_string()
        };

        let total_space = disk.total_space();
        let available_space = disk.available_space();
        let used_space = total_space - available_space;

        let usage_percent = if total_space > 0 {
            let percent = (used_space as f64 / total_space as f64) * 100.0;
            round_to_decimals(percent, 1)
        } else {
            0.0
        };

        sensors.push(SystemSensor {
            name: format!("disk_usage_{}", name_suffix),
            label: None,
            value: usage_percent,
            unit: "%".to_string(),
            sensor_type: SystemSensorType::DiskUsage,
        });

        sensors.push(SystemSensor {
            name: format!("disk_used_{}", name_suffix),
            label: None,
            value: round_to_decimals((used_space as f64) / (1024.0 * 1024.0 * 1024.0), 2),
            unit: "GB".to_string(),
            sensor_type: SystemSensorType::DiskUsed,
        });

        sensors.push(SystemSensor {
            name: format!("disk_total_{}", name_suffix),
            label: None,
            value: round_to_decimals((total_space as f64) / (1024.0 * 1024.0 * 1024.0), 2),
            unit: "GB".to_string(),
            sensor_type: SystemSensorType::DiskTotal,
        });
    }

    sensors
}