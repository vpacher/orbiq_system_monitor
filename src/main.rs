mod config;
mod cpu_frequency;
mod gpu_sensors;
mod homeassistant;
mod hwmon_devices;
mod hwmon_sensors;
mod mqtt_client;
mod sensors;
mod stale_sensor_cleanup;
mod system_sensor;

use crate::homeassistant::system_sensor_availability;
use crate::mqtt_client::{get_mqtt_client, publish, publish_handler};
use crate::sensors::{generate_payload, get_all_sensors};
use crate::stale_sensor_cleanup::remove_stale_sensors;
use config::DaemonConfig;
use homeassistant::DeviceInfo;
use rumqttc::{AsyncClient, Event, EventLoop, Outgoing, Packet};
use std::collections::HashSet;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{sleep, timeout};

/// Upper bound for delivering the "offline" availability messages on shutdown.
const SHUTDOWN_FLUSH: Duration = Duration::from_secs(3);

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let config = DaemonConfig::load_with_fallback();
    println!("Starting temperature daemon for device: {}", config.device.name);

    let (client, mut eventloop) = get_mqtt_client(&config);

    // Remove any sensors from Home Assistant that were published on a previous run
    // but are no longer detected on this one (e.g. hardware was removed).
    let current_sensor_names: HashSet<String> = get_all_sensors().map(|s| s.name).collect();
    remove_stale_sensors(&client, &mut eventloop, &config.device.name, &current_sensor_names).await;

    // Both loops run forever; the select only ends on a shutdown signal.
    tokio::select! {
        () = drive_event_loop(&mut eventloop) => {},
        () = publish_loop(&client, &config) => {},
        _ = tokio::signal::ctrl_c() => {},
        () = wait_for_sigterm() => {},
    }

    // The select dropped the event-loop future, so it has to be driven again here
    // or the queued offline messages never leave the process.
    mark_offline(&client, &mut eventloop, &config).await;
}

/// Polls the MQTT event loop forever. rumqttc reconnects on the next poll after an error.
async fn drive_event_loop(eventloop: &mut EventLoop) {
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::ConnAck(_))) => println!("Connected to MQTT broker"),
            Ok(_) => {}
            Err(e) => {
                eprintln!("MQTT Error: {e}");
                println!("Attempting to reconnect in 5 seconds...");
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

async fn publish_loop(client: &AsyncClient, config: &DaemonConfig) {
    // Wait a bit for the connection to establish
    sleep(Duration::from_secs(5)).await;
    let mut published_sensors: HashSet<String> = HashSet::new();
    let device_info = DeviceInfo::from_config(&config.device);
    let mut cycle = 0u32;

    loop {
        for sensor in get_all_sensors() {
            let topics = generate_payload(&sensor, config, &device_info);
            publish_handler(client, &topics, &mut published_sensors, config.discovery_delay_ms, cycle).await;
        }
        cycle = cycle.wrapping_add(1);
        sleep(Duration::from_secs(config.update_interval_secs)).await;
    }
}

/// Publishes "offline" for every sensor and disconnects cleanly, driving the event
/// loop concurrently so the messages are actually sent (bounded by `SHUTDOWN_FLUSH`).
async fn mark_offline(client: &AsyncClient, eventloop: &mut EventLoop, config: &DaemonConfig) {
    let send = async {
        for sensor in get_all_sensors() {
            let payload = system_sensor_availability(&sensor, &config.device.name, false);
            if let Err(e) = publish(client, payload).await {
                eprintln!("Failed to mark sensor {} as offline: {e}", sensor.name);
            }
        }
        if let Err(e) = client.disconnect().await {
            eprintln!("MQTT disconnect request failed: {e}");
        }
    };
    let flush = async {
        loop {
            match eventloop.poll().await {
                Ok(Event::Outgoing(Outgoing::Disconnect)) | Err(_) => break,
                Ok(_) => {}
            }
        }
    };
    if timeout(SHUTDOWN_FLUSH, async { tokio::join!(send, flush) }).await.is_err() {
        eprintln!("Shutdown: offline messages not fully delivered within {SHUTDOWN_FLUSH:?}");
    }
}

async fn wait_for_sigterm() {
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to bind SIGTERM handler");
    sigterm.recv().await;
}
