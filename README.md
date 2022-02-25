# mercury
mercury is a project to read information from dht22 temperature sensors and report it over mqtt to a chosen backend (datadog, prometheus+granfa, etc). mercury is the firmware that runs on esp32s. see the sister project [galenguyer/hermes](https://github.com/galenguyer/hermes) for the mqtt->storage client.

## esp32 setup
### rust toolchian
set up a modified rust toolchain for this
```bash
git clone https://github.com/esp-rs/rust-build
cd rust-build
./install-rust-toolchain.sh
```

### compilation and loading
```bash
source .env
cargo +esp espflash /dev/ttyUSB0 --speed 460800 [--release] [--monitor]
```
#### .env values
```
ESP32_WIFI_SSID
ESP32_WIFI_PASS
ESP32_PRIMARY_DNS_SERVER
ESP32_SECONDARY_DNS_SERVER
ESP32_MQTT_BROKER_URL
ESP32_MQTT_USERNAME
ESP32_MQTT_PASSWORD
```
