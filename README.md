# OrbIQ System Monitor

A small Linux daemon that publishes hardware and system sensors to MQTT, with Home Assistant
auto-discovery. One native binary, packaged as a `.deb` with a guided setup.

Built for servers, Proxmox hosts, NAS boxes and Raspberry Pis whose hardware sensors
(temperatures, fans, currents, CPU frequency scaling) you want in Home Assistant, without
running Home Assistant on that machine.

## Why OrbIQ

- **No runtime to manage.** A single Rust binary (about 4–5 MB), no Python, no virtualenv,
  no `pip` on an externally managed system.
- **Light.** About 4–5 MB of RAM and well under 0.1 % CPU at the default 30 s interval
  (measured on an x86 Proxmox host and a Raspberry Pi 5).
- **Guided install.** `apt install ./orbiq...deb` asks for the broker, credentials and device
  name and starts the service. Upgrades never touch your config.
- **Hardware sensors from the kernel.** Everything `hwmon` exposes (CPU, chipset, NVMe,
  drives, fans, currents), NVIDIA GPUs, and CPU frequency scaling state.
- **TLS and mutual TLS** to the broker, with your own CA or the system CA certificates.
- **Tidy in Home Assistant.** Sensors from removed hardware are deleted on the next start,
  and every sensor is marked unavailable when the daemon stops.

## Sensors

| Group | Sensors | Source |
|-------|---------|--------|
| System | CPU usage, memory usage/used/total, disk usage/used/total per mount | `sysinfo` |
| Temperatures | every `tempN_input` (CPU, chipset, NVMe, drives, Wi-Fi, ...) | `/sys/class/hwmon` |
| Fans | every `fanN_input` that has a `fanN_label`, in RPM | `/sys/class/hwmon` |
| Currents | every `currN_input`, in mA | `/sys/class/hwmon` |
| NVIDIA GPU | temperature, fan, usage, memory usage per GPU | `nvidia-smi` (if installed) |
| CPU frequency scaling | governor, energy-performance preference (EPP), boost, max and current frequency | `/sys/devices/system/cpu/cpufreq` |

Each sensor only appears when the machine exposes it. CPU frequency scaling details:

- `cpu_governor`: e.g. `performance`, `powersave`, `schedutil` (`mixed` if CPUs differ)
- `cpu_energy_preference`: e.g. `performance`, `balance_power`, `power` (amd-pstate-epp, intel_pstate)
- `cpu_boost`: `on` / `off` (`cpufreq/boost`, or the inverted `intel_pstate/no_turbo`)
- `cpu_max_frequency`: current frequency ceiling in MHz (highest `scaling_max_freq`)
- `cpu_frequency`: average current frequency of all CPUs in MHz

## Installation

### Debian package (recommended)

Packages for `amd64` and `arm64` (Raspberry Pi OS 64-bit, Ubuntu, Debian, Proxmox) are on the
[releases page](https://github.com/vpacher/orbiq_system_monitor/releases/latest), together
with `SHA256SUMS`.

```bash
VERSION=$(curl -fsSL https://api.github.com/repos/vpacher/orbiq_system_monitor/releases/latest \
  | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
ARCH=$(dpkg --print-architecture)   # amd64 or arm64
curl -LO "https://github.com/vpacher/orbiq_system_monitor/releases/download/v${VERSION}/orbiq_system_monitor_${VERSION}_${ARCH}.deb"
curl -LO "https://github.com/vpacher/orbiq_system_monitor/releases/download/v${VERSION}/SHA256SUMS"
sha256sum -c SHA256SUMS --ignore-missing
sudo apt install "./orbiq_system_monitor_${VERSION}_${ARCH}.deb"
```

The install starts a short guided setup:

| Question | Default |
|----------|---------|
| MQTT broker host name or IP | (empty: configure later) |
| Connect with TLS? | no |
| MQTT port | `1883`, or `8883` with TLS |
| CA certificate file (only asked with TLS) | (empty: system CA certificates) |
| MQTT user name | (empty: anonymous) |
| MQTT password (only asked with a user name) | |
| Device name (used in topics and entity IDs) | the host name |
| Start now and on every boot? | yes |

The answers go to `/etc/orbiq_system_monitor/config.toml` (mode `600`, root only). The
password is not kept in the debconf database.

- **Change the answers later:** `sudo dpkg-reconfigure orbiq-system-monitor`. This asks again
  only while the config file is missing or still the shipped example; a filled-in config is
  never overwritten. To start over, remove the config file first.
- **Upgrades** never ask questions and never touch an existing config. A running service is
  restarted; a stopped or disabled one stays as it is.
- **Configure by hand instead:** leave the broker empty, then:
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
  sudo DEBIAN_FRONTEND=noninteractive apt install ./orbiq_system_monitor_VERSION_amd64.deb
  ```
- **Remove completely:** `sudo apt purge orbiq-system-monitor` removes the config directory
  and the stored answers.

### Manual installation (any Linux with systemd)

1. Download `orbiq_system_monitor-VERSION-linux-amd64` (or `-linux-arm64`) from the releases
   page and install it as `/usr/bin/orbiq_system_monitor` (`chmod 755`).
2. Create `/etc/orbiq_system_monitor/config.toml` (see [Configuration](#configuration)) and
   `chmod 600` it: it holds the MQTT password.
3. Create `/etc/systemd/system/orbiq_system_monitor.service`:
   ```ini
   [Unit]
   Description=OrbIQ System Monitor (system sensors to MQTT / Home Assistant)
   Wants=network-online.target
   After=network-online.target

   [Service]
   ExecStart=/usr/bin/orbiq_system_monitor
   Restart=on-failure
   RestartSec=10

   [Install]
   WantedBy=multi-user.target
   ```
4. `sudo systemctl daemon-reload && sudo systemctl enable --now orbiq_system_monitor`

## Configuration

The daemon reads the first file that exists:

1. `/etc/orbiq_system_monitor/config.toml`
2. `/etc/orbiq/config.toml`
3. `./orbiq_system_monitor.toml`
4. `./config.toml`

```toml
[mqtt]
broker = "mqtt.example.lan"
port = 1883
username = "orbiq"        # optional
password = "secret"       # optional
keep_alive_secs = 30

# TLS (optional)
# tls = true                                    # usually with port = 8883
# ca_file = "/etc/ssl/certs/homelab-ca.pem"     # CA of the broker; system CAs when unset
# client_cert_file = "/etc/orbiq_system_monitor/client.crt"   # mutual TLS: both or neither
# client_key_file = "/etc/orbiq_system_monitor/client.key"

[device]
name = "server-01"        # letters, digits, "-" and "_" only

# Seconds between two sensor updates
update_interval_secs = 30

# Delay between Home Assistant discovery messages on the first publish, in milliseconds
discovery_delay_ms = 200
```

### TLS

With `tls = true` the broker certificate is verified against `ca_file` (PEM, may contain
several certificates) or, without it, against the system CA certificates. The broker name in
`broker` must match the certificate (a host name in the SAN, or an IP address SAN when you
connect by IP). For mutual TLS, set `client_cert_file` and `client_key_file` (PEM; RSA, ECDSA
or Ed25519 keys). A wrong path or a key that does not match the certificate stops the daemon
with a clear message; a certificate the broker rejects shows up in the log as an MQTT error.

## Home Assistant

Sensors appear automatically when:

1. the MQTT integration is set up in Home Assistant,
2. Home Assistant and OrbIQ use the same broker,
3. MQTT discovery is enabled (the default).

Each machine becomes one device, `orbiq_<device name>`, with entities such as
`sensor.orbiq_server_01_cpu_usage`, `sensor.orbiq_server_01_k10temp_1` or
`sensor.orbiq_server_01_cpu_governor`.

### MQTT topics

| Topic | Payload |
|-------|---------|
| `homeassistant/sensor/orbiq_<device>/<sensor>/config` | discovery config (retained) |
| `homeassistant/sensor/orbiq_<device>/<sensor>/state` | `{"value": <number or "text">}` |
| `homeassistant/sensor/orbiq_<device>/<sensor>/availability` | `online` / `offline` (retained) |

On start, sensors that were published by an earlier run but no longer exist (e.g. a removed
disk or fan) are removed from Home Assistant. On shutdown (SIGTERM, Ctrl-C) every sensor is
marked `offline` before the daemon disconnects.

## Service management

```bash
sudo systemctl status orbiq_system_monitor
sudo journalctl -u orbiq_system_monitor -f
sudo systemctl restart orbiq_system_monitor
```

## Troubleshooting

**The service does not start.** Check the log (`journalctl -u orbiq_system_monitor`). Typical
causes are a TOML syntax error in the config file or an unreachable broker.

**No sensors in Home Assistant.** Check that the MQTT integration works and uses the same
broker, and that discovery is enabled. `mosquitto_sub -v -t 'homeassistant/sensor/orbiq_#'`
shows what OrbIQ publishes.

**TLS: `invalid peer certificate: UnknownIssuer`.** The broker certificate is not signed by a
CA OrbIQ trusts. Set `ca_file` to the CA that signed it. `NotValidForName` means the
`broker` name does not match the certificate.

**A temperature or fan is missing.** OrbIQ shows what the kernel exposes under
`/sys/class/hwmon`. If `sensors` (lm-sensors) does not show it either, the hardware needs a
driver (e.g. `nct6687` for some MSI boards). Fans are only published when they have a label.

**Inside a VM or container** you only see the guest's virtual hardware. Install OrbIQ on the
host (e.g. the Proxmox node) to get the real sensors.

## Building from source

Requirements: a Rust toolchain; for the amd64 + arm64 builds [`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild) and `zig` (binaries pinned to glibc 2.17), or with `BUILD=cross`, `cross` with podman or Docker. The container tests need podman or Docker.

```bash
cargo build --release                       # current platform
./build-releases.sh                         # clippy -D warnings + tests, then amd64 + arm64 builds (cargo-zigbuild)
./package-deb.sh                            # .deb packages from the cross builds
./test-package.sh                           # package install/upgrade/purge tests in a Debian container
./test-tls.sh                               # end-to-end TLS and mutual-TLS tests against Mosquitto in a container
./release.sh --dry-run                      # full gate, release assets and notes, no publishing
./release.sh                                # tag v<version>, GitHub release with SHA256SUMS
```

## Contributing

Issues and pull requests are welcome. Before a pull request, please run:

```bash
cargo clippy --all-targets -- -D warnings
cargo test
```

If you change anything in `debian/`, also run `./test-package.sh`; for MQTT connection or TLS
changes, `./test-tls.sh`.

## Support

Please open an issue on [GitHub](https://github.com/vpacher/orbiq_system_monitor/issues).
Include the OrbIQ version (`dpkg -s orbiq-system-monitor`), your distribution and
architecture, and the relevant log lines.

## License

MIT, see [LICENSE](LICENSE).
