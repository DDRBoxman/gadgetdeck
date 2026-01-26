# gadgetdeck

[![Crates.io](https://img.shields.io/crates/v/gadgetdeck.svg)](https://crates.io/crates/gadgetdeck)

Rust library and companion software to allow Raspberry Pi devices to show up as a Streamdeck over USB to another computer.

<img width="1200" height="600" alt="gadgetdeck" src="https://github.com/user-attachments/assets/2c6b933d-8d07-4501-94e9-c33115f27fa0" />


## Supported Streamdeck devices
* Stream Deck Mini
* Stream Deck Mk2
* Stream Deck XL
* Stream Deck Pedal
* Steam Deck Plus

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

