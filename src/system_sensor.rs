use sysinfo::{Disks, System};

#[derive(Debug, Clone)]
pub struct SystemSensor {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub sensor_type: SystemSensorType,
}

#[derive(Debug, Clone)]
pub enum SystemSensorType {
    CpuUsage,
    MemoryUsage,
    DiskUsage,
    MemoryUsed,
    MemoryTotal,
    DiskUsed,
    DiskTotal,
}

impl SystemSensorType {
    pub fn icon(&self) -> &str {
        match self {
            SystemSensorType::CpuUsage => "mdi:cpu-64-bit",
            SystemSensorType::MemoryUsage
            | SystemSensorType::MemoryUsed
            | SystemSensorType::MemoryTotal => "mdi:memory",
            SystemSensorType::DiskUsage
            | SystemSensorType::DiskUsed
            | SystemSensorType::DiskTotal => "mdi:harddisk",
        }
    }
}

pub fn collect_system_stats() -> Vec<SystemSensor> {
    let mut system = System::new_all();
    system.refresh_all();

    let mut sensors = Vec::new();

    // CPU usage (overall)
    let cpu_usage = system.global_cpu_usage();
    sensors.push(SystemSensor {
        name: "cpu_usage".to_string(),
        value: cpu_usage as f64,
        unit: "%".to_string(),
        sensor_type: SystemSensorType::CpuUsage,
    });

    // Memory usage
    let total_memory = system.total_memory();
    let used_memory = system.used_memory();
    let memory_usage_percent = if total_memory > 0 {
        (used_memory as f64 / total_memory as f64) * 100.0
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
        value: (used_memory as f64) / (1024.0 * 1024.0 * 1024.0), // Convert to GB
        unit: "GB".to_string(),
        sensor_type: SystemSensorType::MemoryUsed,
    });

    sensors.push(SystemSensor {
        name: "memory_total".to_string(),
        value: (total_memory as f64) / (1024.0 * 1024.0 * 1024.0), // Convert to GB
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
            (used_space as f64 / total_space as f64) * 100.0
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
            value: (used_space as f64) / (1024.0 * 1024.0 * 1024.0), // Convert to GB
            unit: "GB".to_string(),
            sensor_type: SystemSensorType::DiskUsed,
        });

        sensors.push(SystemSensor {
            name: format!("disk_total_{}", name_suffix),
            value: (total_space as f64) / (1024.0 * 1024.0 * 1024.0), // Convert to GB
            unit: "GB".to_string(),
            sensor_type: SystemSensorType::DiskTotal,
        });
    }

    sensors
}
