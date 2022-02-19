# mercury

## esp32 setup
### micropython? no
```bash
esptool.py --chip esp32 --port /dev/ttyUSB0 erase_flash
esptool.py --chip esp32 --port /dev/ttyUSB0 --connect-attempts 21 --baud 460800 write_flash 0x1000 esp32/bin/esp32-20220117-v1.18.bin
minicom -D /dev/ttyUSB0 -b115200
```
### rust good
set up a modified rust toolchain for this
```bash
git clone https://github.com/esp-rs/rust-build
./install-rust-toolchain.sh --export-file export-esp-rust.sh
source ./rust-build/export-esp-rust.sh
rustup override set esp
```

```bash
RUST_ESP32_STD_DEMO_WIFI_SSID="RIT-Legacy" RUST_ESP32_STD_DEMO_WIFI_PASS="" cargo build [--release]
esptool.py --chip esp32 elf2image target/xtensa-esp32-espidf/debug/mercury
esptool.py --chip esp32 --port /dev/ttyUSB0 --connect-attempts 21 --baud 460800 --before=default_reset --after=hard_reset write_flash --flash_mode dio --flash_freq 40m --flash_size 4MB 0x10000 target/xtensa-esp32-espidf/debug/mercury.bin
```