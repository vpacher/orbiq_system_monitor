mod config;
mod fan_sensors;
mod homeassistant;
mod hwmon_devices;
mod mqtt_client;
mod sensors;
mod system_sensor;
mod temperature_sensor;

use crate::homeassistant::system_sensor_availability;
use crate::mqtt_client::{get_mqtt_client, publish, publish_handler, MqttSensorTopics};
use crate::sensors::{generate_payload, get_all_sensors, SystemSensor};
use config::DaemonConfig;
use homeassistant::DeviceInfo;
use rumqttc::{AsyncClient, Event, EventLoop, Packet};
use std::collections::HashSet;
use std::time::Duration;
use time::sleep;
use tokio::signal::unix::{signal, SignalKind};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio::{signal, task, time};

#[tokio::main]
async fn main() {
    let config: DaemonConfig = DaemonConfig::load_with_fallback();

    println!(
        "Starting temperature daemon for device: {}",
        config.device.name
    );

    let (publish_client, mut eventloop): (AsyncClient, EventLoop) = get_mqtt_client(&config);

    //needed for the exit task
    let finish_client = publish_client.clone();
    let finish_config = config.clone();

    // Spawn a task to publish temperatures and system stats
    let publish_task: JoinHandle<()> = task::spawn(async move {
        // Wait a bit for the connection to establish
        sleep(Duration::from_secs(5)).await;
        let mut published_sensors: HashSet<String> = HashSet::new();
        let device_info: DeviceInfo = DeviceInfo::from_config(&config.device);
        let mut cycle_counter = 0u32;

        loop {
            for s in get_all_sensors() {
                let sensor_topics: MqttSensorTopics = generate_payload(&s, &config, &device_info);
                publish_handler(
                    &publish_client,
                    &sensor_topics,
                    &mut published_sensors,
                    0,
                    &mut cycle_counter,
                )
                .await;
            }

            cycle_counter = cycle_counter.wrapping_add(1);
            sleep(Duration::from_secs(config.update_interval_secs)).await;
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
        _ = signal::ctrl_c()  => finish_task(&finish_client, &finish_config).await,
        _ = wait_for_sigterm() => finish_task(&finish_client, &finish_config).await

    }
}

async fn finish_task(finish_client: &AsyncClient, finish_config: &DaemonConfig) {
    let all_sensors: Vec<SystemSensor> = get_all_sensors().collect();
    for sensor in &all_sensors {
        let payload = system_sensor_availability(sensor, &finish_config.device.name, false);
        if let Err(e) = timeout(Duration::from_millis(20), publish(&finish_client, payload)).await {
            eprintln!("Failed to mark sensor {} as offline: {}", sensor.name, e);
        }
    }
}

async fn wait_for_sigterm() {
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to bind SIGTERM handler");
    sigterm.recv().await;
}
