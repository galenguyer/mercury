use crate::{wifi, Message};
use anyhow::Result;
use embedded_svc::mqtt::client::{Connection, Publish, QoS};
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};
use log::info;
use std::thread;

pub fn connect(
    url: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<esp_idf_svc::mqtt::client::EspMqttClient> {
    let client_id = format!("esp32-{}", wifi::get_mac());
    let conf = MqttClientConfiguration {
        client_id: Some(&client_id),
        username,
        password,
        ..Default::default()
    };

    let (client, mut connection) = EspMqttClient::new(url, &conf)?;

    thread::spawn(move || {
        info!("MQTT Listening for messages");
        while let Some(msg) = connection.next() {
            if let Err(e) = msg {
                info!("MQTT Message ERROR: {}", e);
            }
        }
        info!("MQTT connection loop exit");
    });

    Ok(client)
}

pub fn send(
    client: &mut EspMqttClient,
    topic: &str,
    message: &mut Message,
) -> Result<u32, esp_idf_sys::EspError> {
    message.author = format!("esp32-{}", wifi::get_mac());
    client.publish(
        topic,
        QoS::AtMostOnce,
        false,
        serde_json::to_string(message).unwrap().as_bytes(),
    )
}
