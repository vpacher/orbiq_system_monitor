mod config;
mod homeassistant;
mod mqtt_client;
mod system_sensor;
mod temperature_sensor;

use crate::homeassistant::{
    discovery_config, sensor_availability, system_discovery_config, system_sensor_availability,
    system_state, temperature_state,
};
use crate::mqtt_client::{get_mqtt_client, publish, MqttSensorTopics};
use config::DaemonConfig;
use homeassistant::DeviceInfo;
use rumqttc::{Event, Packet};
use std::collections::HashSet;
use std::time::Duration;
use system_sensor::collect_system_stats;
use temperature_sensor::collect_all_temperatures;
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
    let publish_client = client.clone();
    let publish_task = task::spawn(async move {
        // Wait a bit for connection to establish
        time::sleep(Duration::from_secs(5)).await;

        let mut published_temp_sensors: HashSet<String> = HashSet::new();
        let device_info = DeviceInfo::from_config(&config.device);
        let mut cycle_counter = 0u32;

        loop {
            let temp_sensors = collect_all_temperatures();
            let system_sensors = collect_system_stats();

            let temp_payloads: Vec<MqttSensorTopics> = temp_sensors
                .iter()
                .map(|sensor| MqttSensorTopics {
                    name: sensor.name.clone(),
                    state: temperature_state(sensor, &config.device.name),
                    discovery: discovery_config(sensor, &config.device.name, &device_info),
                    availability: sensor_availability(sensor, &config.device.name, true),
                })
                .collect::<Vec<_>>();
            
            let system_payloads: Vec<MqttSensorTopics> = system_sensors
                .iter()
                .map(|sensor| MqttSensorTopics {
                    name: sensor.name.clone(),
                    state: system_state(sensor, &config.device.name),
                    discovery: system_discovery_config(sensor, &config.device.name, &device_info),
                    availability: system_sensor_availability(sensor, &config.device.name, true),
                })
                .collect::<Vec<_>>();

            let all_payloads: Vec<MqttSensorTopics> =
                temp_payloads.into_iter().chain(system_payloads).collect();

            // Handle all payloads
            if all_payloads.is_empty() {
                eprintln!("No sensors found");
            } else {
                for payload in &all_payloads {
                    if !published_temp_sensors.contains(&payload.name) {
                        //publish Discovery
                        if let Err(e) = publish(&publish_client, payload.discovery.clone()).await {
                            eprintln!("Discovery config error: {}", e);
                        } else {
                            //publish availability
                            published_temp_sensors.insert(payload.name.clone());
                            // Mark as available immediately after discovery
                            if let Err(e) =
                                publish(&publish_client, payload.availability.clone()).await
                            {
                                eprintln!("Availability publish error: {}", e);
                            }
                        }
                        time::sleep(Duration::from_millis(config.discovery_delay_ms))
                            .await;
                    }
                    //publish state
                    if let Err(e) = publish(&publish_client, payload.state.clone()).await {
                        eprintln!("State publish error: {}", e);
                    }
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
                _ = signal::ctrl_c() => {
                    println!("Received shutdown signal, marking sensors as offline...");
                    let temp_sensors = collect_all_temperatures();
                    for sensor in &temp_sensors {
                        let payload = sensor_availability(sensor, &config.device.name, false);
                        if let Err(e) = publish(&publish_client, payload).await {
                            eprintln!("Failed to mark temperature sensor {} as offline: {}", sensor.name, e);
                        }
                        time::sleep(Duration::from_millis(50)).await;
                    }
                    let system_sensors = collect_system_stats();
                    for sensor in &system_sensors {
                        let payload = system_sensor_availability(sensor, &config.device.name, false);
                        if let Err(e) = publish(&publish_client, payload).await {
                            eprintln!("Failed to mark system sensor {} as offline: {}", sensor.name, e);
                        }
                        time::sleep(Duration::from_millis(50)).await;
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
    }
}
