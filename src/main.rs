use anyhow::bail;
use esp_idf_hal::ledc::Timer;
use esp_idf_hal::ledc::config::TimerConfig;
use log::*;

use embedded_hal::digital::v2::OutputPin;
use embedded_svc::httpd::*;
use embedded_svc::ipv4;
use embedded_svc::mqtt::client::{Connection, Publish, QoS};
use embedded_svc::ping::Ping;
use embedded_svc::wifi::*;
use esp_idf_hal::prelude::*;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};
use esp_idf_svc::netif::*;
use esp_idf_svc::nvs::*;
use esp_idf_svc::ping;
use esp_idf_svc::sysloop::*;
use esp_idf_svc::wifi::*;
use esp_idf_sys;


use std::{env, sync::Arc, thread, time::*};

use mercury::Message;
mod dht22;
use dht22::DHT22;

const SSID: &str = env!("ESP32_WIFI_SSID");
const PASS: Option<&'static str> = option_env!("ESP32_WIFI_PASS");
static mut MAC: &str = "";

fn main() -> Result<()> {
    esp_idf_sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let dht_pin = pins.gpio15.into_input_output_od().unwrap();
    let mut dht = DHT22::new(dht_pin);
    loop {
        match dht.read_blocking()  {
            Ok(reading) => {
                info!("{:#?}", reading);
                info!("{}", reading.temp_celcius());
            },
            Err(e) => {
                info!("{:?}", e);
            }
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    return Ok(());

    let netif_stack = Arc::new(EspNetifStack::new()?);
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    let default_nvs = Arc::new(EspDefaultNvs::new()?);

    let wifi = wifi(netif_stack, sys_loop_stack, default_nvs)?;

    wifi.with_client_netif(|netif| unsafe {
        MAC = Box::leak(hex::encode(netif.unwrap().get_mac().unwrap()).into_boxed_str());
    });

    let mut mqtt_client = mqtt_connect()?;
    mqtt_send(&mut mqtt_client, "mercury", "connected")?;

    let mut led = pins.gpio2.into_input_output_od().unwrap();

    let mut loop_count: u32 = 1;
    loop {
        led.set_high().unwrap();
        mqtt_send(
            &mut mqtt_client,
            "mercury",
            &format!("looped {} times", loop_count),
        )?;
        loop_count += 1;
        thread::sleep(Duration::from_secs(1));
        led.set_low().unwrap();
        thread::sleep(Duration::from_secs(4));
    }
}

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
        password: PASS.unwrap_or("").to_string(),
        auth_method: match PASS {
            Some(s) => match s.len() {
                0 => AuthMethod::None,
                _ => AuthMethod::WPA2Personal,
            },
            None => AuthMethod::None,
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

    if let Some(ns) = option_env!("ESP32_PRIMARY_DNS_SERVER") {
        wifi.with_client_netif_mut(|netif| {
            netif.unwrap().set_dns(
                ns.parse::<ipv4::Ipv4Addr>()
                    .unwrap_or_else(|_| ipv4::Ipv4Addr::new(1, 1, 1, 1)),
            );
        });
    }
    if let Some(ns) = option_env!("ESP32_SECONDARY_DNS_SERVER") {
        wifi.with_client_netif_mut(|netif| {
            netif.unwrap().set_secondary_dns(
                ns.parse::<ipv4::Ipv4Addr>()
                    .unwrap_or_else(|_| ipv4::Ipv4Addr::new(8, 8, 8, 8)),
            );
        });
    }

    Ok(wifi)
}

#[allow(unused)]
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
    message: &str,
) -> Result<u32, esp_idf_sys::EspError> {
    unsafe {
        let client_id = format!("esp32-{}", MAC);
        client.publish(
            topic,
            QoS::ExactlyOnce,
            false,
            serde_json::to_string(&Message{
                author: client_id,
                message: message.to_string(),
            }).unwrap().as_bytes(),
        )
    }
}
