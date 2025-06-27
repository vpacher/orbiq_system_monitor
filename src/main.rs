mod config;
mod homeassistant;
mod temperature_sensor;

use clap::{Parser, Subcommand};
use config::DaemonConfig;
use homeassistant::{
    publish_discovery_config, publish_sensor_availability, publish_temperature_state, DeviceInfo,
};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet};
use std::collections::HashSet;
use std::time::Duration;
use temperature_sensor::collect_all_temperatures;
use tokio::{signal, task, time};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,

    /// MQTT broker address (overrides config file)
    #[arg(long)]
    mqtt_broker: Option<String>,

    /// Device name (overrides config file)
    #[arg(long)]
    device_name: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate example configuration file
    GenConfig {
        /// Output path for the configuration file
        #[arg(short, long, default_value = "/etc/temp-daemon/config.toml")]
        output: String,
    },
    /// Run the daemon
    Run,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::GenConfig { output }) => {
            // Create directory if it doesn't exist
            if let Some(parent) = std::path::Path::new(output).parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("Failed to create directory {}: {}", parent.display(), e);
                    std::process::exit(1);
                }
            }

            match DaemonConfig::save_example(output) {
                Ok(_) => println!("Example configuration saved to: {}", output),
                Err(e) => {
                    eprintln!("Failed to save configuration: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Run) | None => {
            run_daemon(cli).await;
        }
    }
}

async fn run_daemon(cli: Cli) {
    // Load configuration
    let mut config = if let Some(config_path) = cli.config {
        match DaemonConfig::load_from_file(&config_path) {
            Ok(config) => {
                println!("Loaded configuration from: {}", config_path);
                config
            }
            Err(e) => {
                eprintln!("Failed to load config from {}: {}", config_path, e);
                std::process::exit(1);
            }
        }
    } else {
        DaemonConfig::load_with_fallback()
    };

    // Apply CLI overrides
    if let Some(broker) = cli.mqtt_broker {
        config.mqtt.broker = broker;
    }
    if let Some(device_name) = cli.device_name {
        config.device.name = device_name;
    }

    println!(
        "Starting temperature daemon with device: {}",
        config.device.name
    );
    println!("MQTT broker: {}:{}", config.mqtt.broker, config.mqtt.port);

    // Setup MQTT
    let mut mqttoptions = MqttOptions::new(
        &config.mqtt.client_id,
        &config.mqtt.broker,
        config.mqtt.port,
    );
    mqttoptions.set_keep_alive(Duration::from_secs(config.mqtt.keep_alive_secs));

    if let (Some(username), Some(password)) = (&config.mqtt.username, &config.mqtt.password) {
        mqttoptions.set_credentials(username, password);
    }

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    // Clone config for the publish task before moving it
    let config_for_task = config.clone();

    // Spawn a task to publish temperatures
    let publish_client = client.clone();
    let publish_task = task::spawn(async move {
        // Wait a bit for connection to establish
        time::sleep(Duration::from_secs(5)).await;

        let mut published_sensors: HashSet<String> = HashSet::new();
        let device_info = DeviceInfo::from_config(&config_for_task.device);

        loop {
            let sensors = collect_all_temperatures();

            if sensors.is_empty() {
                eprintln!("No temperature sensors found");
            } else {
                // Publish discovery configs for new sensors (all under the same device)
                for sensor in &sensors {
                    if !published_sensors.contains(&sensor.name) {
                        if let Err(e) = publish_discovery_config(
                            &publish_client,
                            sensor,
                            &config_for_task.device.name,
                            &device_info,
                        )
                        .await
                        {
                            eprintln!("Discovery config error: {}", e);
                        } else {
                            published_sensors.insert(sensor.name.clone());
                        }
                        time::sleep(Duration::from_millis(config_for_task.discovery_delay_ms))
                            .await;
                    }
                }

                // Publish temperature states
                for sensor in &sensors {
                    if let Err(e) = publish_temperature_state(
                        &publish_client,
                        sensor,
                        &config_for_task.device.name,
                    )
                    .await
                    {
                        eprintln!("Temperature state publish error: {}", e);
                    }
                }
            }

            // Check if we should exit
            tokio::select! {
                _ = time::sleep(Duration::from_secs(config_for_task.update_interval_secs)) => {},
                _ = signal::ctrl_c() => {
                    println!("Received shutdown signal, marking sensors as offline...");
                    let sensors = collect_all_temperatures();
                    for sensor in &sensors {
                        if let Err(e) = publish_sensor_availability(&publish_client, sensor, &config_for_task.device.name, false).await {
                            eprintln!("Failed to mark sensor {} as offline: {}", sensor.name, e);
                        }
                    }
                    break;
                }
            }
        }
    });

    // Handle events and connection status
    tokio::select! {
        _ = async {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        println!("Connected to MQTT broker");
                    }
                    Ok(Event::Incoming(_packet)) => {
                        // Optionally log incoming packets
                    }
                    Ok(Event::Outgoing(_packet)) => {
                        // Optionally log outgoing packets
                    }
                    Err(e) => {
                        eprintln!("MQTT Error: {}", e);
                        time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        } => {},
        _ = publish_task => {},
        _ = signal::ctrl_c() => {
            println!("Shutting down...");
        }
    }
}
