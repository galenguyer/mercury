use anyhow::Result;
use embedded_hal::digital::v2::OutputPin;
use esp_idf_hal::prelude::Peripherals;
use esp_idf_svc::netif::EspNetifStack;
use esp_idf_svc::nvs::EspDefaultNvs;
use esp_idf_svc::sntp;
use esp_idf_svc::sysloop::EspSysLoopStack;
use log::info;
use std::{
    env,
    sync::Arc,
    thread,
    time::{Duration, SystemTime},
};

use mercury::Message;
mod dht22;
mod mqtt;
mod wifi;

fn main() -> Result<()> {
    // A hack to make sure that the rwlock implementation and the esp32c3 atomics
    // are linked to the final executable. Call this function once e.g. in the
    // beginning of your main function
    esp_idf_sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    // Set up networking stack and connect to wifi
    let netif_stack = Arc::new(EspNetifStack::new()?);
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    let default_nvs = Arc::new(EspDefaultNvs::new()?);
    let _wifi = wifi::connect(
        netif_stack,
        sys_loop_stack,
        default_nvs,
        env!("ESP32_WIFI_SSID"),
        option_env!("ESP32_WIFI_PASS"),
        option_env!("ESP32_PRIMARY_DNS_SERVER"),
        option_env!("ESP32_SECONDARY_DNS_SERVER"),
    )?;

    // We have to change the NTP server because the underlying libraries don't know
    // how to handle more than one A record, which the default of 0.pool.ntp.org
    // returns.
    let sntp_conf = sntp::SntpConf {
        servers: [String::from("ntp.rit.edu")],
        operating_mode: sntp::OperatingMode::Poll,
        sync_mode: sntp::SyncMode::Immediate,
    };
    let sntp = sntp::EspSntp::new(&sntp_conf).unwrap();
    while sntp.get_sync_status() != sntp::SyncStatus::Completed {
        // Wait for SNTP to complete
        thread::sleep(Duration::from_secs(1));
    }
    info!("NTP synchronized");

    // Connect to MQTT
    let mut mqtt_client = mqtt::connect(
        env!("ESP32_MQTT_BROKER_URL"),
        option_env!("ESP32_MQTT_USERNAME"),
        option_env!("ESP32_MQTT_PASSWORD"),
    )?;

    // Set up GPIO
    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let mut led = pins.gpio2.into_input_output_od().unwrap();
    let dht_pin = pins.gpio15.into_input_output_od().unwrap();
    let mut dht = dht22::DHT22::new(dht_pin);

    loop {
        // turn on the LED
        led.set_high().unwrap();
        let unix_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();
        // There's no cleaner way I've found to truncate a float to a single decimal
        // point :(
        if let Ok(reading) = dht.read_blocking() {
            mqtt::send(
                &mut mqtt_client,
                "mercury",
                &mut Message {
                    author: "".to_string(),
                    timestamp: unix_time,
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
            // We sleep one second here so the light stays on, otherwise it just blinks very
            // quickly
            thread::sleep(Duration::from_secs(1));
        }
        // Turn off the LED and wait before taking another reading
        led.set_low().unwrap();
        thread::sleep(Duration::from_secs(4));
    }
}
