# OrbIQ System Monitor

A lightweight, cross-platform system monitoring daemon that publishes system metrics to MQTT with automatic Home Assistant discovery support.

## Features

- **System Metrics**: CPU usage, memory usage/total, disk usage/total for all mounted drives
- **Temperature Monitoring**: Hardware temperature sensors (CPU, GPU, motherboard, etc.)
- **MQTT Integration**: Publishes metrics to MQTT broker with configurable intervals
- **Home Assistant Auto-Discovery**: Automatically creates sensors in Home Assistant
- **Cross-Platform**: Supports Linux (x86_64 and ARM64/aarch64)
- **Lightweight**: Minimal resource usage with efficient data collection
- **Systemd Integration**: Runs as a system service with proper lifecycle management

## Monitored Metrics

- **CPU Usage**: Overall CPU utilization percentage
- **Memory Usage**: RAM usage percentage and absolute values (used/total in GB)
- **Disk Usage**: Disk usage percentage and absolute values (used/total in GB) for all mounted filesystems
- **Temperature Sensors**: Hardware temperature readings from available sensors (CPU, GPU, motherboard, etc.)

# Installation

## Option 1: Debian Package (Recommended)

Download the appropriate `.deb` package for your architecture from the [releases page](https://github.com/your-repo/releases):
bash
### For x86_64/AMD64 systems
sudo dpkg -i orbiq_system_monitor_VERSION_amd64.deb
### For ARM64/aarch64 systems (Raspberry Pi, etc.)
sudo dpkg -i orbiq_system_monitor_VERSION_arm64.deb

## Option 2: Manual Installation

1. Download the binary for your architecture from the releases page
2. Copy to `/usr/bin/orbiq_system_monitor`
3. Create configuration directory: `sudo mkdir -p /etc/orbiq_system_monitor`
4. Copy the configuration file to `/etc/orbiq_system_monitor/config.toml`
5. Create and enable the systemd service (see example service file below)

## Configuration

The service looks for configuration files in the following order:

1. `/etc/orbiq_system_monitor/config.toml`
2. `/etc/orbiq/config.toml`
3. `./orbiq_system_monitor.toml`
4. `./config.toml`

### Configuration File Example
toml
#### OrbIQ System Monitoring Configuration
[mqtt] broker = "your-mqtt-broker.local" port = 1883 username = "your_mqtt_username" password = "your_mqtt_password" keep_alive_secs = 30
[device] name = "server-01"
#### Update interval in seconds
update_interval_secs = 30
#### Delay between discovery messages in milliseconds
discovery_delay_ms = 200

### Configuration Options

- **mqtt.broker**: MQTT broker hostname or IP address
- **mqtt.port**: MQTT broker port (default: 1883)
- **mqtt.username**: MQTT username (optional)
- **mqtt.password**: MQTT password (optional)
- **mqtt.keep_alive_secs**: MQTT keep-alive interval
- **device.name**: Unique device name (used in MQTT topics and Home Assistant entity names)
- **update_interval_secs**: How often to collect and publish metrics
- **discovery_delay_ms**: Delay between Home Assistant discovery messages

## Usage

### First-time Setup

1. Install the package
2. Edit the configuration file:
   ```bash
   sudo nano /etc/orbiq_system_monitor/config.toml
   ```
3. Start the service:
   ```bash
   sudo systemctl start orbiq_system_monitor
   sudo systemctl enable orbiq_system_monitor
   ```

### Service Management
bash
# Check service status
sudo systemctl status orbiq_system_monitor
# View logs
sudo journalctl -u orbiq_system_monitor -f
# Restart service
sudo systemctl restart orbiq_system_monitor
# Stop service
sudo systemctl stop orbiq_system_monitor

## Home Assistant Integration

The service automatically publishes Home Assistant discovery messages, so sensors will appear automatically in your Home Assistant instance if:

1. MQTT integration is configured in Home Assistant
2. The MQTT broker is accessible from both the monitor and Home Assistant
3. Home Assistant discovery is enabled (default)

### MQTT Topics

The service publishes to the following topic structure:

- **State topics**: `orbiq/{device_name}/sensor/{sensor_name}/state`
- **Discovery topics**: `homeassistant/sensor/orbiq_{device_name}/{sensor_name}/config`

### Example Sensors in Home Assistant

For a device named "server-01", you'll see sensors like:

- `sensor.orbiq_server_01_cpu_usage`
- `sensor.orbiq_server_01_memory_usage`
- `sensor.orbiq_server_01_memory_used`
- `sensor.orbiq_server_01_memory_total`
- `sensor.orbiq_server_01_disk_usage_root`
- `sensor.orbiq_server_01_disk_used_root`
- `sensor.orbiq_server_01_disk_total_root`

## Building from Source

### Prerequisites

- Rust toolchain
- `cross` for cross-compilation (optional)

### Build Commands
bash
# Build for current platform
cargo build --release
# Cross-compile for Linux x86_64
cross build --release --target x86_64-unknown-linux-gnu
# Cross-compile for Linux ARM64
cross build --release --target aarch64-unknown-linux-gnu
# Build all targets and create releases
./build-releases.sh
# Create Debian packages
./package-deb.sh

## Systemd Service File
ini [Unit] Description=OrbIQ System Monitor After=network.target Wants=network.target
[Service] Type=simple ExecStart=/usr/bin/orbiq_system_monitor Restart=always RestartSec=10 User=root Group=root
[Install] WantedBy=multi-user.target


If installing manually, create `/etc/systemd/system/orbiq_system_monitor.service`:

## Troubleshooting

### Service won't start
1. Check configuration file syntax: `toml` format validation
2. Verify MQTT broker connectivity
3. Check logs: `sudo journalctl -u orbiq_system_monitor -f`

### Sensors not appearing in Home Assistant
1. Verify MQTT integration is working in Home Assistant
2. Check MQTT broker logs for incoming messages
3. Ensure Home Assistant discovery is enabled
4. Check that the device name doesn't contain invalid characters

### High resource usage
1. Increase `update_interval_secs` to reduce collection frequency
2. Increase `discovery_delay_ms` if publishing too quickly

## License

[Add your license here]

## Contributing

[Add contribution guidelines here]

## Support

[Add support information here]
