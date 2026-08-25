# Advanced Snapshot

Advanced Snapshot is a mobile-first, GPL-3.0-or-later camera application for
Linux phones. It is based on [GNOME Snapshot](https://gitlab.gnome.org/GNOME/snapshot),
keeps the upstream Git history and Aperture library, and installs independently
as `io.github.lolren.AdvancedSnapshot`.

This repository is the application layer of the OnePlus 6T camera work. The
tested lower stack—kernel sensor modes, libcamera software ISP, autofocus and
PipeWire control transport—lives in
[oneplus6t-pmos-fixes](https://github.com/lolren/oneplus6t-pmos-fixes).
The separation lets either project follow its own upstream without turning
postmarketOS into an unmaintainable permanent fork.

## Current features

| Feature | What it brings | Status |
| --- | --- | --- |
| Independent app ID and settings | Co-installs with GNOME Snapshot and can be rolled back separately | Implemented |
| Full-frame still selection | Saves the largest 4:3 mode up to 2048x1536 instead of preview resolution | Implemented |
| Sensor-aware tap-to-focus | Maps preview taps through letterboxing, crop and orientation into a real libcamera AF window | Implemented on supported rear cameras |
| Focus reticle | Gives immediate visible tap feedback and safely clears stale callbacks | Implemented |
| Exposure compensation | Requests standard -1 to +1 EV from the lower stack | Implemented |
| Colour, contrast and detail | Sends standard saturation, contrast and sharpness controls to preview and capture | Implemented |
| Digital zoom | Provides 1x–4x Camerabin zoom | Implemented |
| Photo, video and QR modes | Retains Snapshot's capture, recording, gallery and code-detection flows | Implemented |
| Focus-result state | Changes the reticle only when libcamera reports scanning/focused/failed | Planned; requires a truthful metadata bridge |
| HDR and calibrated colour | Multi-frame merge, tone mapping, CCM and lens shading | Not implemented and never shown as available |

“Android-class” is a feature-by-feature target, not a marketing claim. A
control is enabled only when the camera pipeline can implement and report it.
See [docs/FEATURES.md](docs/FEATURES.md) for the acceptance matrix and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the control path.

## Runtime requirements

- PipeWire and WirePlumber;
- the GStreamer PipeWire, GTK4, good, bad and ugly plugin sets;
- GTK 4.18 or newer and libadwaita 1.8 or newer;
- glycin, an XDG camera portal and matching desktop portal backend; and
- for libcamera phones, `pipewire-spa-libcamera` plus a camera stack that
  advertises the controls used by the interface.

The tested OnePlus 6T baseline is kernel r8, libcamera/IPA r24, PipeWire
libcamera SPA r6 and postmarketOS edge. Generic webcams still use the inherited
Snapshot paths; phone-specific controls degrade safely when absent.

## Build and test

The reference source is GNOME Snapshot 50.0 plus reviewable downstream commits.
Build dependencies include Meson 1.7+, Rust 1.92, Cargo, a C compiler,
pkg-config and development packages for the runtime libraries above.

```sh
meson setup build --prefix=/usr
meson compile -C build
meson test -C build --print-errorlogs
```

Install into a disposable staging root first:

```sh
DESTDIR="$PWD/stage" meson install -C build
find stage -type f -o -type l
```

Do not replace distro-owned GNOME Snapshot files. Advanced Snapshot uses a
different binary, helper, D-Bus name, icon name, schema and resource namespace.
The pinned postmarketOS APK recipe, artifact validator and installation policy
are tracked in [packaging/postmarketos](packaging/postmarketos) and
[docs/INSTALL.md](docs/INSTALL.md).

## Upstream maintenance

The `upstream` remote points to GNOME Snapshot. Device and app changes are kept
as small topical commits so a new upstream tag can be tested on a temporary
branch before it replaces the known-good base. Never force a rejected camera
patch or activate an untested dependency update on the phone. See
[docs/UPSTREAM.md](docs/UPSTREAM.md).

## Project status

The independently named baseline is under active development. The already
installed `snapshot-50.0-r3` remains the phone's known-good UI until the new
application builds, packages and passes native photo/video acceptance. The
next work is truthful focus-result metadata, the mobile control surface,
resolution/aspect/timer controls and robust video status.

No photograph, raw frame, device identifier, account credential, proprietary
Android library or vendor tuning blob belongs in this repository.

## License and attribution

Advanced Snapshot is licensed under GPL-3.0-or-later. It preserves GNOME
Snapshot's copyright, license, translation history and contributor commits.
The fork is independent and is not endorsed by the GNOME project.
