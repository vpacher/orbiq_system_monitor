mod config;
mod homeassistant;
mod mqtt_client;
mod sensors;
mod system_sensor;
mod temperature_sensor;

use crate::homeassistant::system_sensor_availability;
use crate::mqtt_client::{get_mqtt_client, publish, MqttSensorTopics};
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
            // Handle all payloads
            for payload in &all_payloads {
                if !published_sensors.contains(&payload.name) {
                    //publish Discovery
                    if let Err(e) = publish(&publish_client, payload.discovery.clone()).await {
                        eprintln!("Discovery config error: {}", e);
                    } else {
                        //publish availability
                        published_sensors.insert(payload.name.clone());
                        // Mark as available immediately after discovery
                        if let Err(e) = publish(&publish_client, payload.availability.clone()).await
                        {
                            eprintln!("Availability publish error: {}", e);
                        }
                    }
                    time::sleep(Duration::from_millis(config.discovery_delay_ms)).await;
                }
                //publish state
                if let Err(e) = publish(&publish_client, payload.state.clone()).await {
                    eprintln!("State publish error: {}", e);
                }
            }

            // Publish availability for all sensors periodically (every 20 cycles to reduce message volume)
            cycle_counter += 1;
            if cycle_counter % 20 == 0 {
                // Every 20 cycles (every 10 minutes with 30-second intervals)
                println!("Refreshing sensor availability status...");

                for payload in &all_payloads {
                    if let Err(e) = publish(&publish_client, payload.availability.clone()).await {
                        eprintln!("Availability refresh error: {}", e);
                    }
                    time::sleep(Duration::from_millis(20)).await;
                }
            }

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
