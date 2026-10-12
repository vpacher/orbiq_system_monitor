# OrbIQ System Monitor

A lightweight, cross-platform system monitoring daemon that publishes system metrics to MQTT with automatic Home Assistant discovery support.

## Features

- **System Metrics**: CPU usage, memory usage/total, disk usage/total for all mounted drives
- **Temperature Monitoring**: Hardware temperature sensors (CPU, GPU, motherboard, etc.)
- **Fans, Currents, GPU**: hwmon fan speeds and currents, NVIDIA GPU load/memory/fan via `nvidia-smi`
- **CPU Frequency Scaling**: governor, energy-performance preference, boost and current/max frequency
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
- **Fan Sensors**: hwmon `fanN_input` in RPM (only fans that have a `fanN_label`)
- **Current Sensors**: hwmon `currN_input` in mA
- **NVIDIA GPU**: temperature, fan, usage and memory usage per GPU (when `nvidia-smi` is installed)
- **CPU Frequency Scaling** (from `/sys/devices/system/cpu/cpufreq`, each only when the driver exposes it):
  - `cpu_governor`: e.g. `performance`, `powersave`, `schedutil` (`mixed` if CPUs differ)
  - `cpu_energy_preference`: EPP, e.g. `performance`, `balance_power`, `power` (amd-pstate-epp, intel_pstate)
  - `cpu_boost`: `on` / `off` (`cpufreq/boost`, or the inverted `intel_pstate/no_turbo`)
  - `cpu_max_frequency`: current frequency ceiling in MHz (highest `scaling_max_freq`)
  - `cpu_frequency`: average current frequency across all CPUs in MHz

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
4. Copy `config/orbiq_system_monitor.toml` to `/etc/orbiq_system_monitor/config.toml` and `chmod 600` it (it holds the MQTT password)
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

Installing the `.deb` starts a short guided setup (debconf):

| Question | Default |
|----------|---------|
| MQTT broker host name or IP | (empty: configure later) |
| MQTT port | `1883` |
| MQTT user name | (empty: anonymous) |
| MQTT password (only asked with a user name) | |
| Device name (used in topics and entity IDs) | the host name |
| Start now and on every boot? | yes |

The answers are written to `/etc/orbiq_system_monitor/config.toml` (mode `600`, root only).
The password is not kept in the debconf database.

- **Change the answers later:** `sudo dpkg-reconfigure orbiq-system-monitor`. This only
  asks again while the config file is missing or still the shipped example; a filled-in
  config is never overwritten. To start over, remove the config file first.
- **Upgrades** never ask questions and never touch an existing config. A running service
  is restarted; a stopped or disabled one stays as it is.
- **Configure by hand instead:** leave the broker empty, then
  ```bash
  sudo cp /usr/share/doc/orbiq-system-monitor/examples/config.toml /etc/orbiq_system_monitor/config.toml
  sudo chmod 600 /etc/orbiq_system_monitor/config.toml
  sudo nano /etc/orbiq_system_monitor/config.toml
  sudo systemctl enable --now orbiq_system_monitor
  ```
- **Automated installs** (Ansible, cloud-init, ...): preseed the answers, then install
  non-interactively:
  ```bash
  sudo debconf-set-selections <<'EOF'
  orbiq-system-monitor orbiq-system-monitor/mqtt_broker string mqtt.example.lan
  orbiq-system-monitor orbiq-system-monitor/mqtt_port string 1883
  orbiq-system-monitor orbiq-system-monitor/mqtt_username string orbiq
  orbiq-system-monitor orbiq-system-monitor/mqtt_password password secret
  orbiq-system-monitor orbiq-system-monitor/device_name string server-01
  orbiq-system-monitor orbiq-system-monitor/enable_service boolean true
  EOF
  sudo DEBIAN_FRONTEND=noninteractive dpkg -i orbiq_system_monitor_VERSION_amd64.deb
  ```
- **Purge** (`dpkg --purge orbiq-system-monitor`) removes the config directory and the
  stored answers.

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

- **State topics**: `homeassistant/sensor/orbiq_{device_name}/{sensor_name}/state`, payload `{"value": <number or "text">}`
- **Availability topics**: `homeassistant/sensor/orbiq_{device_name}/{sensor_name}/availability` (`online` / `offline`, retained)
- **Discovery topics**: `homeassistant/sensor/orbiq_{device_name}/{sensor_name}/config` (retained)

On shutdown (SIGTERM / Ctrl-C) every sensor is marked `offline` before the daemon disconnects.

### Example Sensors in Home Assistant

For a device named "server-01", you'll see sensors like:

- `sensor.orbiq_server_01_cpu_usage`
- `sensor.orbiq_server_01_memory_usage`
- `sensor.orbiq_server_01_memory_used`
- `sensor.orbiq_server_01_memory_total`
- `sensor.orbiq_server_01_disk_usage_root`
- `sensor.orbiq_server_01_disk_used_root`
- `sensor.orbiq_server_01_disk_total_root`
- `sensor.orbiq_server_01_cpu_governor`
- `sensor.orbiq_server_01_cpu_max_frequency`

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
# (runs `cargo clippy -- -D warnings` and `cargo test` first; any failure stops the build)
./build-releases.sh
# Create Debian packages
./package-deb.sh
# Test the package scripts (install, preseed, upgrades, purge, interactive setup)
# in a debian:bookworm container; needs podman or CONTAINER_ENGINE=docker
./test-package.sh

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
