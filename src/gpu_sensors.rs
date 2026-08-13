use crate::sensors::{SystemSensor, SystemSensorType};
use std::io::ErrorKind;
use std::process::Command;

const NVIDIA_SMI_QUERY: &str = "index,temperature.gpu,fan.speed,utilization.gpu,utilization.memory";

pub fn collect_all_gpu_sensors() -> Vec<SystemSensor> {
    match query_nvidia_smi() {
        Ok(output) => parse_nvidia_smi_output(&output),
        // No nvidia-smi binary means no NVIDIA GPU present - not an error.
        Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            eprintln!("Failed to query nvidia-smi: {}", e);
            Vec::new()
        }
    }
}

fn query_nvidia_smi() -> std::io::Result<String> {
    let output = Command::new("nvidia-smi")
        .arg(format!("--query-gpu={}", NVIDIA_SMI_QUERY))
        .arg("--format=csv,noheader,nounits")
        .output()?;

    if !output.status.success() {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_nvidia_smi_output(output: &str) -> Vec<SystemSensor> {
    output.lines().flat_map(parse_gpu_line).collect()
}

fn parse_gpu_line(line: &str) -> Vec<SystemSensor> {
    let fields: Vec<&str> = line.split(',').map(str::trim).collect();
    if fields.len() != 5 {
        return Vec::new();
    }
    let index = fields[0];

    let mut sensors = Vec::new();

    if let Ok(value) = fields[1].parse::<f64>() {
        sensors.push(gpu_sensor(index, "temp", "Temperature", value, "°C", SystemSensorType::Temperature));
    }
    if let Ok(value) = fields[2].parse::<f64>() {
        sensors.push(gpu_sensor(index, "fan", "Fan", value, "%", SystemSensorType::Fan));
    }
    if let Ok(value) = fields[3].parse::<f64>() {
        sensors.push(gpu_sensor(index, "usage", "Usage", value, "%", SystemSensorType::GpuUsage));
    }
    if let Ok(value) = fields[4].parse::<f64>() {
        sensors.push(gpu_sensor(index, "memory_usage", "Memory Usage", value, "%", SystemSensorType::GpuMemoryUsage));
    }

    sensors
}

fn gpu_sensor(
    index: &str,
    name_suffix: &str,
    label_suffix: &str,
    value: f64,
    unit: &str,
    sensor_type: SystemSensorType,
) -> SystemSensor {
    SystemSensor {
        name: format!("nvidia_gpu{}_{}", index, name_suffix),
        label: Some(format!("GPU {} {}", index, label_suffix)),
        value,
        unit: unit.to_string(),
        sensor_type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gpu_line() {
        let sensors = parse_gpu_line("0, 45, 30, 12, 8");
        assert_eq!(sensors.len(), 4);
        assert_eq!(sensors[0].name, "nvidia_gpu0_temp");
        assert_eq!(sensors[0].value, 45.0);
        assert_eq!(sensors[1].name, "nvidia_gpu0_fan");
        assert_eq!(sensors[2].name, "nvidia_gpu0_usage");
        assert_eq!(sensors[3].name, "nvidia_gpu0_memory_usage");
    }

    #[test]
    fn test_parse_gpu_line_with_na_fields() {
        // nvidia-smi reports "[N/A]" for unsupported metrics (e.g. fan on some cards)
        let sensors = parse_gpu_line("0, 45, [N/A], 12, 8");
        assert_eq!(sensors.len(), 3);
        assert!(!sensors.iter().any(|s| s.name == "nvidia_gpu0_fan"));
    }

    #[test]
    fn test_parse_multiple_gpus() {
        let output = "0, 45, 30, 12, 8\n1, 50, 40, 20, 15\n";
        let sensors = parse_nvidia_smi_output(output);
        assert_eq!(sensors.len(), 8);
        assert_eq!(sensors[4].name, "nvidia_gpu1_temp");
    }

    #[test]
    fn test_parse_malformed_line_ignored() {
        assert!(parse_gpu_line("garbage").is_empty());
        assert!(parse_gpu_line("").is_empty());
    }
}
