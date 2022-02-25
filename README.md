# mercury

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
ESP32_PRIMARY_DNS_SERVER
ESP32_SECONDARY_DNS_SERVER
ESP32_MQTT_BROKER_URL
ESP32_MQTT_USERNAME
ESP32_MQTT_PASSWORD
```
