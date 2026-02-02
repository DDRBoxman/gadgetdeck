# gadgetdeck

[![Crates.io](https://img.shields.io/crates/v/gadgetdeck.svg)](https://crates.io/crates/gadgetdeck)

Rust library and companion software to allow Raspberry Pi devices to show up as a Streamdeck over USB to another computer.

https://github.com/user-attachments/assets/324c3d54-91d9-449a-9869-3c7f4a09c775

<img width="1200" height="600" alt="gadgetdeck" src="https://github.com/user-attachments/assets/2c6b933d-8d07-4501-94e9-c33115f27fa0" />


## Supported Streamdeck devices
* Stream Deck Mini
* Stream Deck Mk2
* Stream Deck XL
* Stream Deck Pedal
* Stream Deck Plus
* Stream Deck Neo

## Gadgetdeck Library

The rust library and core of both programs. It uses FunctionFS to emulate a Stream Deck device.

[More Info](crates/gadgetdeck/README.md)

## Programs

### gadgetdeck-display

Emulate a streamdeck on a PI device that has a touchscreen. Renders directly to the framebuffer without a desktop environemnt.

[More Info](crates/gadgetdeck-display/README.md)

### gadgetdeck-server

Emulate a streamdeck on a PI device and allow it to be accessable in a web browser.

[More Info](crates/gadgetdeck-server/README.md)

## Setup examples

### Headless Pi zero 2 W

Install Raspberry Pi OS lite (no desktop)
(Raspberry pi imager makes this easy https://www.raspberrypi.com/software/)

Use the image to configure your device and configure wifi and setup ssh credentials.

Plug in the device to the second usb port (not the power one) and plug that into the computer running the streamdeck software.

SSH into the device once it connects to wifi.

```
sudo vi /boot/firmware/config.txt
```

Add `dtoverlay=dwc2` to the bottom of the file and reboot

> [!CAUTION]
> The below command runs a web server with root permissions this is dangerous.
> There is a config script and systemd configs coming soon to work around this 
```
sudo ./gadgetdeck-server --device mk2
```

Visit the local address of the pi on port 3000 from another machine ex: `http://10.0.0.10:3000`

<img width="740" height="566" alt="Screenshot 2026-01-25 at 10 16 39 PM" src="https://github.com/user-attachments/assets/620d49cc-7f5b-40e1-8fcc-215fbbd3c098" />



### Pi 5 with touchscreen

I'm currently using a Pi 5 and this waveshare touchscreen https://www.waveshare.com/wiki/9.3inch_1600x600_LCD

Install Raspberry Pi OS lite (no desktop)
(Raspberry pi imager makes this easy https://www.raspberrypi.com/software/)

Use the image to configure your device and configure wifi and setup ssh credentials.

Plug in the device to the main usbc port and plug that into the computer running the streamdeck software.

SSH into the device once it connects to wifi.

```
sudo vi /boot/firmware/config.txt
```

Add `dtoverlay=dwc2` to the bottom of the file and reboot

```
sudo ./gadgetdeck-display --device plus
```

The device should show up on your display and respond to touch events.

<img width="1600" height="600" alt="screenshot_drm" src="https://github.com/user-attachments/assets/00030d97-3fb8-49dd-8e2a-dc9e4fc5538f" />

### Pi with a super cheap tft touchscreen

This was the one I grabbed: https://www.amazon.com/dp/B0DY5BVGNH

```
sudo apt install libgles2 libegl1 libgbm1 libdrm2

sudo tee /etc/systemd/system/tft-symlink.service << 'EOF'
[Unit]
Description=Create TFT DRM symlink
After=systemd-udev-settle.service

[Service]
Type=oneshot
ExecStart=/bin/bash -c 'mkdir -p /dev/dri/by-path && for card in /dev/dri/card*; do if grep -q ili9486 /sys/class/drm/$(basename $card)/device/uevent 2>/dev/null; then ln -sf $card /dev/dri/by-path/platform-gpu-card; break; fi; done'
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl restart tft-symlink.service
```

```
sudo vim /boot/firmware/config.txt
```

Add:
```
dtoverlay=dwc2
dtoverlay=piscreen,drm,speed=32000000,invy
```

Reboot

```
sudo ./gadgetdeck-display -W 480 -H 320 -d mini
```

![IMG_0780](https://github.com/user-attachments/assets/45122883-70fd-4f34-896e-47e65ab688c7)


## Helpful Dev Tools

**5V USB-C Dual Supply**

Allows you to power a Raspberry PI with a USB C cable while splitting the data off into another cable to hook up to your dev computer.

https://www.tindie.com/products/8086net/5v-usb-c-dual-supply-dual-ideal-diodes/

