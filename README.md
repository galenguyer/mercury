# mercury

## esp32 setup
```bash
esptool.py --chip esp32 --port /dev/ttyUSB0 erase_flash
esptool.py --chip esp32 --port /dev/ttyUSB0 --connect-attempts 21 --baud 460800 write_flash 0x1000 esp32/bin/esp32-20220117-v1.18.bin
minicom -D /dev/ttyUSB0 -b115200
```
