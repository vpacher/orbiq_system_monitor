mod config;
mod homeassistant;
mod mqtt_client;
mod sensors;
mod system_sensor;
mod temperature_sensor;

use crate::homeassistant::system_sensor_availability;
use crate::mqtt_client::{get_mqtt_client, publish, publish_handler, MqttSensorTopics};
use crate::sensors::{generate_payloads, get_all_sensors};
use config::DaemonConfig;
use homeassistant::DeviceInfo;
use rumqttc::{Event, Packet};
use std::collections::HashSet;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};
use tokio::{signal, task, time};

#[tokio::main]
async fn main() {
    let config = DaemonConfig::load_with_fallback();

    println!(
        "Starting temperature daemon with device: {}",
        config.device.name
    );

    let (client, mut eventloop) = get_mqtt_client(&config);

    // Spawn a task to publish temperatures and system stats
    let publish_client = client;
    let publish_task = task::spawn(async move {
        // Wait a bit for the connection to establish
        time::sleep(Duration::from_secs(5)).await;

        let mut published_sensors: HashSet<String> = HashSet::new();
        let device_info = DeviceInfo::from_config(&config.device);
        let mut cycle_counter = 0u32;

        loop {
            let all_sensors = get_all_sensors();
            if all_sensors.is_empty() {
                eprintln!("No sensors found");
            }

            let all_payloads: Vec<MqttSensorTopics> =
                generate_payloads(&all_sensors, &config, &device_info).collect();

            for payload in &all_payloads {
                publish_handler(
                    &publish_client,
                    &payload,
                    &mut published_sensors,
                    config.discovery_delay_ms,
                    &mut cycle_counter,
                )
                .await;
            }

            cycle_counter = cycle_counter.wrapping_add(1);

            // Check if we should exit
            tokio::select! {
                _ = time::sleep(Duration::from_secs(config.update_interval_secs)) => {},
                _ = wait_for_sigterm() => {
                    println!("Received shutdown signal, marking sensors as offline...");
                    for sensor in &all_sensors {
                        let payload = system_sensor_availability(sensor, &config.device.name, false);
                        if let Err(e) = publish(&publish_client, payload).await {
                            eprintln!("Failed to mark sensor {} as offline: {}", sensor.name, e);
                        }
                    }
                    break;
                }
            }
        }
    });

    // Handle events and connection status with auto-reconnect
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
                        println!("Attempting to reconnect in 5 seconds...");
                        time::sleep(Duration::from_secs(5)).await;
                        // The eventloop will automatically try to reconnect
                    }
                }
            }
        } => {},
        _ = publish_task => {},
        _ = signal::ctrl_c() => {
            println!("Shutting down...");
        }
        _ = wait_for_sigterm() => {
            println!("Signal received, shutting down...");
        }

    }
}

async fn wait_for_sigterm() {
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to bind SIGTERM handler");
    sigterm.recv().await;
}
