use crate::homeassistant::{
    discovery_wildcard_topic, extract_discovered_sensor_name, stale_sensor_removal_payloads,
};
use crate::mqtt_client::publish;
use rumqttc::{AsyncClient, Event, EventLoop, Packet, QoS};
use std::collections::HashSet;
use std::time::Duration;
use tokio::time::timeout;

const RECONCILIATION_WINDOW: Duration = Duration::from_secs(2);

// Compares sensors previously published to Home Assistant (retained MQTT discovery
// messages left over from an earlier run) against the sensors detected on this run,
// and removes any that are no longer present - e.g. after hardware changes like a
// removed GPU or fan.
pub async fn remove_stale_sensors(
    client: &AsyncClient,
    eventloop: &mut EventLoop,
    device_name: &str,
    current_sensor_names: &HashSet<String>,
) {
    let wildcard = discovery_wildcard_topic(device_name);
    if let Err(e) = client.subscribe(&wildcard, QoS::AtLeastOnce).await {
        eprintln!("Failed to subscribe for stale sensor reconciliation: {}", e);
        return;
    }

    let mut previously_published: HashSet<String> = HashSet::new();
    let _ = timeout(RECONCILIATION_WINDOW, async {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    println!("Connected to MQTT broker");
                }
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    if p.payload.is_empty() {
                        continue;
                    }
                    if let Some(name) = extract_discovered_sensor_name(&p.topic, device_name) {
                        previously_published.insert(name);
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("MQTT error during stale sensor reconciliation: {}", e);
                    break;
                }
            }
        }
    })
    .await;

    if let Err(e) = client.unsubscribe(&wildcard).await {
        eprintln!("Failed to unsubscribe after stale sensor reconciliation: {}", e);
    }

    let stale: Vec<&String> = previously_published
        .difference(current_sensor_names)
        .collect();

    if stale.is_empty() {
        println!("Sensor reconciliation: no stale sensors found");
        return;
    }

    println!(
        "Sensor reconciliation: removing {} stale sensor(s): {:?}",
        stale.len(),
        stale
    );
    for sensor_name in stale {
        for payload in stale_sensor_removal_payloads(sensor_name, device_name) {
            if let Err(e) = publish(client, payload).await {
                eprintln!("Failed to remove stale sensor {}: {}", sensor_name, e);
            }
        }
    }
}
