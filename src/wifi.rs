use anyhow::{bail, Result};
use embedded_svc::ipv4;
use embedded_svc::ping::Ping;
use embedded_svc::wifi::*;
use esp_idf_svc::netif::*;
use esp_idf_svc::nvs::*;
use esp_idf_svc::ping;
use esp_idf_svc::sysloop::*;
use esp_idf_svc::wifi::*;
use log::*;
use std::sync::Arc;

pub fn wifi_connect(
    netif_stack: Arc<EspNetifStack>,
    sys_loop_stack: Arc<EspSysLoopStack>,
    default_nvs: Arc<EspDefaultNvs>,
    ssid: &'static str,
    pass: Option<&'static str>,
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
