# mercury

## esp32 setup
### rust toolchian
set up a modified rust toolchain for this
```bash
git clone https://github.com/esp-rs/rust-build
cd rust-build
./install-rust-toolchain.sh --export-file export-esp-rust.sh
source ./rust-build/export-esp-rust.sh
rustup override set esp
```

### compilation and loading
```bash
ESP32_WIFI_SSID="RIT-Legacy" ESP32_WIFI_PASS="" cargo build [--release]
esptool.py --chip esp32 elf2image target/xtensa-esp32-espidf/debug/mercury
esptool.py --chip esp32 --port /dev/ttyUSB0 --connect-attempts 21 --baud 460800 --before=default_reset --after=hard_reset write_flash --flash_mode dio --flash_freq 40m --flash_size 4MB 0x10000 target/xtensa-esp32-espidf/debug/mercury.bin
```
