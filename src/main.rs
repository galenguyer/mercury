use log::*;

use embedded_hal::digital::v2::OutputPin;
use embedded_svc::httpd::*;
use embedded_svc::mqtt::client::{Connection, Publish, QoS};
use esp_idf_hal::prelude::*;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};
use esp_idf_svc::netif::*;
use esp_idf_svc::nvs::*;
use esp_idf_svc::sysloop::*;

use std::{env, sync::Arc, thread, time::*};

use mercury::Message;
mod dht22;
mod wifi;

static mut MAC: &str = "";

fn main() -> Result<()> {
    esp_idf_sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let netif_stack = Arc::new(EspNetifStack::new()?);
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    let default_nvs = Arc::new(EspDefaultNvs::new()?);

    let wifi = wifi::wifi_connect(
        netif_stack,
        sys_loop_stack,
        default_nvs,
        env!("ESP32_WIFI_SSID"),
        option_env!("ESP32_WIFI_PASS"),
    )?;

    wifi.with_client_netif(|netif| unsafe {
        MAC = Box::leak(hex::encode(netif.unwrap().get_mac().unwrap()).into_boxed_str());
    });

    let mut mqtt_client = mqtt_connect()?;

    let mut led = pins.gpio2.into_input_output_od().unwrap();
    let dht_pin = pins.gpio15.into_input_output_od().unwrap();
    let mut dht = dht22::DHT22::new(dht_pin);

    loop {
        led.set_high().unwrap();
        if let Ok(reading) = dht.read_blocking() {
            mqtt_send(
                &mut mqtt_client,
                "mercury",
                &mut Message {
                    author: "".to_string(),
                    temperature_c: format!("{:.1}", reading.clone().temp_celcius())
                        .parse::<f32>()
                        .unwrap(),
                    temperature_f: format!("{:.1}", reading.clone().temp_fahrenheit())
                        .parse::<f32>()
                        .unwrap(),
                    humidity: reading.clone().humidity_percent().round(),
                    message: "".to_string(),
                },
            )?;
            thread::sleep(Duration::from_secs(1));
        }
        led.set_low().unwrap();
        thread::sleep(Duration::from_secs(4));
    }
}

fn mqtt_connect() -> Result<esp_idf_svc::mqtt::client::EspMqttClient> {
    unsafe {
        let client_id = format!("esp32-{}", MAC);
        let conf = MqttClientConfiguration {
            client_id: Some(&client_id),
            ..Default::default()
        };

        let (client, mut connection) =
            EspMqttClient::new("mqtt://mercury.student.rit.edu:1883", &conf)?;

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
}

fn mqtt_send(
    client: &mut EspMqttClient,
    topic: &str,
    message: &mut Message,
) -> Result<u32, esp_idf_sys::EspError> {
    unsafe {
        message.author = format!("esp32-{}", MAC);
        client.publish(
            topic,
            QoS::ExactlyOnce,
            false,
            serde_json::to_string(message).unwrap().as_bytes(),
        )
    }
}
