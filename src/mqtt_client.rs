use crate::config::DaemonConfig;
use rumqttc::{AsyncClient, EventLoop, MqttOptions};
use std::time::Duration;

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
    AsyncClient::new(mqttoptions, 100)
}
