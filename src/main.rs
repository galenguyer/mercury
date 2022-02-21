#![allow(unused_imports)]
use std::{env, sync::Arc, thread, time::*};

use anyhow::bail;
use log::*;

use embedded_hal::digital::v2::OutputPin;

use embedded_svc::httpd::*;
use embedded_svc::ipv4;
use embedded_svc::ping::Ping;
use embedded_svc::wifi::*;

use esp_idf_svc::netif::*;
use esp_idf_svc::nvs::*;
use esp_idf_svc::ping;
use esp_idf_svc::sysloop::*;
use esp_idf_svc::wifi::*;

use esp_idf_hal::adc;
use esp_idf_hal::delay;
use esp_idf_hal::gpio;
use esp_idf_hal::i2c;
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi;

use esp_idf_sys::esp;
use esp_idf_sys::{self, c_types};

use embedded_svc::mqtt::client::{Client, Connection, Publish, QoS};
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};

#[allow(dead_code)]
const SSID: &str = env!("ESP32_WIFI_SSID");
#[allow(dead_code)]
const PASS: &str = env!("ESP32_WIFI_PASS");
static mut MAC: &str = "";

fn main() -> Result<()> {
    esp_idf_sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    #[allow(unused)]
    let peripherals = Peripherals::take().unwrap();
    #[allow(unused)]
    let pins = peripherals.pins;

    #[allow(unused)]
    let netif_stack = Arc::new(EspNetifStack::new()?);
    #[allow(unused)]
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    #[allow(unused)]
    let default_nvs = Arc::new(EspDefaultNvs::new()?);

    #[allow(clippy::redundant_clone)]
    #[allow(unused_mut)]
    let mut wifi = wifi(
        netif_stack.clone(),
        sys_loop_stack.clone(),
        default_nvs.clone(),
    )?;
    wifi.with_client_netif_mut(|netif| {
        netif.unwrap().set_dns(ipv4::Ipv4Addr::new(129, 21, 1, 82));
    });
    wifi.with_client_netif_mut(|netif| {
        netif
            .unwrap()
            .set_secondary_dns(ipv4::Ipv4Addr::new(129, 21, 1, 92));
    });
    wifi.with_client_netif(|netif| unsafe {
        MAC = Box::leak(hex::encode(netif.unwrap().get_mac().unwrap()).into_boxed_str());
    });

    let mut mqtt_client = mqtt_connect()?;
    mqtt_send(&mut mqtt_client, "mercury", "entering main loop");

    let mut led = pins.gpio2.into_input_output_od().unwrap();

    let mut loop_count: u32 = 1;
    loop {
        led.set_high().unwrap();
        mqtt_send(
            &mut mqtt_client,
            "mercury",
            &format!("looped {} times", loop_count),
        );
        loop_count += 1;
        thread::sleep(Duration::from_secs(1));
        led.set_low().unwrap();
        thread::sleep(Duration::from_secs(4));
    }
}

#[allow(dead_code)]
fn wifi(
    netif_stack: Arc<EspNetifStack>,
    sys_loop_stack: Arc<EspSysLoopStack>,
    default_nvs: Arc<EspDefaultNvs>,
) -> Result<Box<EspWifi>> {
    let mut wifi = Box::new(EspWifi::new(netif_stack, sys_loop_stack, default_nvs)?);

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == SSID);

    let channel = if let Some(ours) = ours {
        info!(
            "Found configured access point {} on channel {}",
            SSID, ours.channel
        );
        Some(ours.channel)
    } else {
        info!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            SSID
        );
        None
    };

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.into(),
        password: PASS.into(),
        auth_method: if PASS.is_empty() {
            AuthMethod::None
        } else {
            AuthMethod::WPA2Personal
        },
        channel,
        ..Default::default()
    }))?;

    let status = wifi.get_status();

    if let Status(
        ClientStatus::Started(ClientConnectionStatus::Connected(ClientIpStatus::Done(
            _ip_settings,
        ))),
        _,
    ) = status
    {
        //ping(&ip_settings)?;
    } else {
        bail!("Unexpected Wifi status: {:?}", status);
    }

    Ok(wifi)
}

#[allow(dead_code)]
fn ping(ip_settings: &ipv4::ClientSettings) -> Result<()> {
    info!("About to do some pings for {:?}", ip_settings);

    let ping_summary =
        ping::EspPing::default().ping(ip_settings.subnet.gateway, &Default::default())?;
    if ping_summary.transmitted != ping_summary.received {
        bail!(
            "Pinging gateway {} resulted in timeouts",
            ip_settings.subnet.gateway
        );
    }

    info!("Pinging done");

    Ok(())
}

fn mqtt_connect() -> Result<esp_idf_svc::mqtt::client::EspMqttClient> {
    unsafe {
        let client_id = format!("esp32-{}", MAC);
        let conf = MqttClientConfiguration {
            client_id: Some(&client_id),
            ..Default::default()
        };

        let (mut client, mut connection) =
            EspMqttClient::new("mqtt://mercury.student.rit.edu:1883", &conf)?;

        // Need to immediately start pumping the connection for messages, or else subscribe() and publish() below will not work
        // Note that when using the alternative constructor - `EspMqttClient::new_with_callback` - you don't need to
        // spawn a new thread, as the messages will be pumped with a backpressure into the callback you provide.
        // Yet, you still need to efficiently process each message in the callback without blocking for too long.
        //
        // Note also that if you go to http://tools.emqx.io/ and then connect and send a message to topic
        // "mercury", the client configured here should receive it.
        thread::spawn(move || {
            info!("MQTT Listening for messages");

            while let Some(msg) = connection.next() {
                match msg {
                    Err(e) => info!("MQTT Message ERROR: {}", e),
                    Ok(msg) => info!("MQTT Message: {:?}", msg),
                }
            }

            info!("MQTT connection loop exit");
        });

        client.subscribe("mercury", QoS::AtMostOnce)?;

        mqtt_send(&mut client, "mercury", "connected");

        Ok(client)
    }
}

fn mqtt_send(
    client: &mut EspMqttClient,
    topic: &str,
    message: &str,
) -> Result<u32, esp_idf_sys::EspError> {
    unsafe {
        let client_id = format!("esp32-{}", MAC);
        client.publish(
            topic,
            QoS::AtLeastOnce,
            false,
            format!("{}: {}", client_id, message).as_bytes(),
        )
    }
}
