use crate::config::DaemonConfig;
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct MqttPayload {
    pub(crate) topic: String,
    pub(crate) payload: String,
    pub(crate) retain: bool,
}

#[derive(Debug, Clone)]
pub struct MqttSensorTopics {
    pub(crate) name: String,
    pub(crate) state: MqttPayload,
    pub(crate) discovery: MqttPayload,
    pub(crate) availability: MqttPayload,
}

pub fn get_mqtt_client(config: &DaemonConfig) -> (AsyncClient, EventLoop) {
    let mut mqttoptions = MqttOptions::new(
        &config.mqtt.client_id,
        &config.mqtt.broker,
        config.mqtt.port,
    );
    mqttoptions.set_keep_alive(Duration::from_secs(config.mqtt.keep_alive_secs));

    // Increase channel capacity and add auto-reconnect settings
    mqttoptions.set_max_packet_size(10240, 10240);
    mqttoptions.set_clean_session(false);

    if let (Some(username), Some(password)) = (&config.mqtt.username, &config.mqtt.password) {
        mqttoptions.set_credentials(username, password);
    }
    println!("MQTT broker: {}:{}", config.mqtt.broker, config.mqtt.port);
    AsyncClient::new(mqttoptions, 100)
}
pub async fn publish(client: &AsyncClient, data:MqttPayload) -> Result<(), rumqttc::ClientError> {
    client
        .publish(data.topic, QoS::AtLeastOnce, data.retain, data.payload)
        .await
}
