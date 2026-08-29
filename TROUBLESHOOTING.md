# Troubleshooting

## Compare with upstream

Advanced Snapshot is not yet published on Flathub. When a generic webcam path
fails, compare it with the distro or
[Flathub GNOME Snapshot](https://flathub.org/apps/org.gnome.Snapshot) build and
report which application, version and PipeWire node was tested. A failure that
occurs only with the OnePlus camera stack should include the package revisions
from the VibeMarketOS manifest.

## Pipewire
Advanced Snapshot exclusively uses [PipeWire](https://gitlab.freedesktop.org/pipewire/pipewire/) (from here on **PW**) to access camera devices.

Please restart PW to ensure all camera devices are found:

```
systemctl --user restart pipewire.service
systemctl --user restart wireplumber.service
```

On pMOS, keep the socket active while restarting the two services separately.
Stopping `pipewire.socket`, `pipewire.service` and `wireplumber.service` in a
single transaction can report `job canceled` and leave a stale camera link;
the two commands above clear that state without a reboot. Close camera apps
before doing it.

A useful tool to look up information from PW is `pw-dump`. In order to check whether PW currently recognizes any camera devices, run:

```
pw-dump | grep default.video.source
```

If that is not the case, you may want to double-check that all required components for Pipewire camera support are installed, notably:

* [Wireplumber](https://gitlab.freedesktop.org/pipewire/wireplumber) (the PW "session-manager")
* potentially [libcamera](https://libcamera.org/) and the PW libcamera plugin

## XDG Desktop Portal
Snapshot uses the camera portal to request camera access. There are desktop environment specific implementations for it, thus ensure to have the matching one installed:

* [Gnome](https://gitlab.gnome.org/GNOME/xdg-desktop-portal-gnome)
* [KDE](https://github.com/KDE/xdg-desktop-portal-kde)
* [wlroots](https://github.com/emersion/xdg-desktop-portal-wlr) (Sway, Phosh, Hyprland etc.)

If Advanced Snapshot cannot find any devices, check the desktop camera-portal
permission and backend configuration. Flatseal is useful only for a Flatpak
build; a native postmarketOS package uses the host portal directly.

## Gstreamer
Snapshot uses `GstPipeWire` components. In order to list available cameras and additional information about them, look for entries that contain `gst-launch-1.0 pipewiresrc` when running:

```
flatpak run --command=gst-device-monitor-1.0 org.gnome.Snapshot Video/Source
```

for the Flatpak or

```
gst-device-monitor-1.0 Video/Source
```

for non-Flatpak installations.

In the later case, make sure to have the Gstreamer Pipewire plugin installed.

## Logs
In case the issue persists you can get debug output for the application by
running:

```
RUST_LOG=advanced_snapshot=debug,aperture=debug advanced-snapshot --debug
```

If you file an issue make sure to include the version info from the
"Troubleshooting" panel in the application's About dialog.
