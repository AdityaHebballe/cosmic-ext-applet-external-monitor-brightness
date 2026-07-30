# External Monitor Brightness Applet for the COSMIC™ desktop

Change brightness of external monitors via DDC/CI protocol. You can also quickly toggle system dark mode.

## Install

```bash
yay -S cosmic-ext-applet-external-monitor-brightness-custom-git
```

## Keyboard shortcuts

Global shortcuts are configured in **COSMIC Settings → Keyboard → Custom
Shortcuts**. Assign the applet's commands there:

- Increase brightness: `cosmic-ext-applet-external-monitor-brightness --increase`
- Decrease brightness: `cosmic-ext-applet-external-monitor-brightness --decrease`

These commands contact the running applet through the session bus, so they do
not start a second applet or re-enumerate monitors. The applet must be running.
Use the gear button in the applet popup to choose the percentage step used by
both commands (5% by default).
For a Flatpak installation, use `flatpak run
io.github.cosmic_utils.cosmic-ext-applet-external-monitor-brightness --increase`
or replace `--increase` with `--decrease`.

## On-screen display helper

`cosmic-external-osd` is a small D-Bus-activated companion process
from the separate `cosmic-external-osd` project. It owns the desktop OSD surface because panel
applets run through COSMIC Panel's private Wayland socket and cannot reliably
create desktop-wide layer surfaces themselves.

The applet sends brightness values to the helper over the session bus. The
helper is idle until needed, connects directly to the desktop Wayland display,
and can also show the selected audio output for
[`cosmic-audio-switch`](../cosmic-audio-switch/).

![Screenshot](res/screenshot1.png)

## Troubleshooting

Maybe you need to setup the necessary udev rules if ddcutil is old.
For this to work you need write access to `/dev/i2c-*`.
See [https://www.ddcutil.com/i2c_permissions/](https://www.ddcutil.com/i2c_permissions/).

## Credits

Originally created by [@maciekk64](https://github.com/maciekk64)
