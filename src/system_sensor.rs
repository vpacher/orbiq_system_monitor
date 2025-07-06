use sysinfo::{Disks, System};
use crate::sensors::{SystemSensor, SystemSensorType};

// Helper function to round to specified decimal places
fn round_to_decimals(value: f64, decimals: u32) -> f64 {
    let multiplier = 10_f64.powi(decimals as i32);
    (value * multiplier).round() / multiplier
}



pub fn collect_system_stats() -> Vec<SystemSensor> {
    let mut system = System::new_all();
    system.refresh_all();

    let mut sensors = Vec::new();

    // CPU usage (overall) - rounded to 1 decimal place
    let cpu_usage = system.global_cpu_usage();
    sensors.push(SystemSensor {
        name: "cpu_usage".to_string(),
        value: round_to_decimals(cpu_usage as f64, 1),
        unit: "%".to_string(),
        sensor_type: SystemSensorType::CpuUsage,
    });

    // Memory usage - rounded to 1 decimal place
    let total_memory = system.total_memory();
    let used_memory = system.used_memory();
    let memory_usage_percent = if total_memory > 0 {
        let percent = (used_memory as f64 / total_memory as f64) * 100.0;
        round_to_decimals(percent, 1)
    } else {
        0.0
    };

    sensors.push(SystemSensor {
        name: "memory_usage".to_string(),
        value: memory_usage_percent,
        unit: "%".to_string(),
        sensor_type: SystemSensorType::MemoryUsage,
    });

    sensors.push(SystemSensor {
        name: "memory_used".to_string(),
        value: round_to_decimals((used_memory as f64) / (1024.0 * 1024.0 * 1024.0), 2),
        unit: "GB".to_string(),
        sensor_type: SystemSensorType::MemoryUsed,
    });

    sensors.push(SystemSensor {
        name: "memory_total".to_string(),
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
            value: usage_percent,
            unit: "%".to_string(),
            sensor_type: SystemSensorType::DiskUsage,
        });

        sensors.push(SystemSensor {
            name: format!("disk_used_{}", name_suffix),
            value: round_to_decimals((used_space as f64) / (1024.0 * 1024.0 * 1024.0), 2),
            unit: "GB".to_string(),
            sensor_type: SystemSensorType::DiskUsed,
        });

        sensors.push(SystemSensor {
            name: format!("disk_total_{}", name_suffix),
            value: round_to_decimals((total_space as f64) / (1024.0 * 1024.0 * 1024.0), 2),
            unit: "GB".to_string(),
            sensor_type: SystemSensorType::DiskTotal,
        });
    }

    sensors
}