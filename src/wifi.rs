use anyhow::{bail, Result};
use embedded_svc::ipv4;
use embedded_svc::ping::{self, Ping};
use embedded_svc::wifi::{
    AuthMethod, ClientConfiguration, ClientConnectionStatus, ClientIpStatus, ClientStatus,
    Configuration, Status, Wifi,
};
use esp_idf_svc::netif::EspNetifStack;
use esp_idf_svc::nvs::EspDefaultNvs;
use esp_idf_svc::ping::EspPing;
use esp_idf_svc::sysloop::EspSysLoopStack;
use esp_idf_svc::wifi::EspWifi;
use lazy_static::lazy_static;
use log::info;
use std::sync::{Arc, Mutex};

lazy_static! {
    static ref MAC: Mutex<String> = Mutex::new(String::from(""));
}

/// Connect to wifi
pub fn connect(
    netif_stack: Arc<EspNetifStack>,
    sys_loop_stack: Arc<EspSysLoopStack>,
    default_nvs: Arc<EspDefaultNvs>,
    ssid: &'static str,
    pass: Option<&'static str>,
    primary_dns: Option<&'static str>,
    secondary_dns: Option<&'static str>,
) -> Result<Box<EspWifi>> {
    let mut wifi = Box::new(EspWifi::new(netif_stack, sys_loop_stack, default_nvs)?);

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == ssid);

    let channel = if let Some(ours) = ours {
        info!(
            "Found configured access point {} on channel {}",
            ssid, ours.channel
        );
        Some(ours.channel)
    } else {
        info!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            ssid
        );
        None
    };

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid.into(),
        password: pass.unwrap_or("").to_string(),
        auth_method: match pass {
            Some(s) => match s.len() {
                0 => AuthMethod::None,
                _ => AuthMethod::WPA2Personal,
            },
            None => AuthMethod::None,
        },
        channel,
        ..ClientConfiguration::default()
    }))?;

    wifi.wait_status_with_timeout(std::time::Duration::from_secs(20), |status| {
        !status.is_transitional()
    })
    .map_err(|e| anyhow::anyhow!("Unexpected Wifi status: {:?}", e))?;

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
        info!("Unexpected Wifi status: {:?}", status);
    }

    // Set DNS from environment in case DHCP doesn't provide DNS servers for us
    if let Some(ns) = primary_dns {
        wifi.with_client_netif_mut(|netif| {
            netif.unwrap().set_dns(
                ns.parse::<ipv4::Ipv4Addr>()
                    .unwrap_or_else(|_| ipv4::Ipv4Addr::new(1, 1, 1, 1)),
            );
        });
    }
    if let Some(ns) = secondary_dns {
        wifi.with_client_netif_mut(|netif| {
            netif.unwrap().set_secondary_dns(
                ns.parse::<ipv4::Ipv4Addr>()
                    .unwrap_or_else(|_| ipv4::Ipv4Addr::new(8, 8, 8, 8)),
            );
        });
    }

    // Set the MAC address static variable
    wifi.with_client_netif(|netif| {
        *MAC.lock().unwrap() = hex::encode(netif.unwrap().get_mac().unwrap());
    });

    Ok(wifi)
}

/// Get the MAC address of the Wifi interface after connecting
pub fn get_mac() -> String {
    return (*MAC.lock().unwrap()).to_string();
}

#[allow(unused)]
fn ping(ip_settings: &ipv4::ClientSettings) -> Result<()> {
    info!("About to do some pings for {:?}", ip_settings);

    let ping_summary =
        EspPing::default().ping(ip_settings.subnet.gateway, &ping::Configuration::default())?;
    if ping_summary.transmitted != ping_summary.received {
        bail!(
            "Pinging gateway {} resulted in timeouts",
            ip_settings.subnet.gateway
        );
    }

    info!("Pinging done");

    Ok(())
}
