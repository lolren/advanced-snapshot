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
| Software-ISP-friendly preview | Selects a supported 720p-class live mode when available, while keeping still capture at the higher photo mode | Implemented |
| Sensor-aware tap-to-focus | Maps preview taps through letterboxing, crop and orientation into a real libcamera AF window | Implemented on supported rear cameras |
| Truthful focus reticle | Shows amber while a request is pending, green only for metadata-confirmed focus and red for failure; stale helpers cannot update a newer tap | Implemented; requires AF-state transport |
| Exposure compensation | Requests standard -1 to +1 EV from the lower stack | Implemented |
| Colour, contrast and detail | Sends standard saturation, contrast and sharpness controls to preview and capture | Implemented |
| Synchronized digital zoom | The image-control slider, two-finger pinch gesture and on-preview value chip share one 1x–4x Camerabin zoom value; tapping the chip resets to 1x | Live 33 ms coalesced updates installed in r4 |
| Photo, video and QR modes | Retains Snapshot's capture, recording, gallery and code-detection flows | Implemented |
| Focus-result state | Correlates each accepted trigger with libcamera `AfState` request metadata instead of treating control acceptance as optical success | Implemented and accepted with the OnePlus 6T r7 transport |
| HDR and calibrated colour | Multi-frame merge, tone mapping, CCM and lens shading | Not implemented and never shown as available |

“Android-class” is a feature-by-feature target, not a marketing claim. A
control is enabled only when the camera pipeline can implement and report it.
See [docs/FEATURES.md](docs/FEATURES.md) for the acceptance matrix and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the control path.

## Using the mobile controls

- Tap a subject in the rear-camera preview to request focus there. The square
  is amber while the request is pending, green only after focus metadata says
  `Focused`, and red after an optical or transport failure.
- Spread or pinch two fingers over the preview to zoom between 1x and 4x. The
  value chip and the **Main Menu → Image Controls → Zoom** slider stay in sync.
  Tap the value chip to return directly to 1x.
- Open **Image Controls** for exposure compensation, colour saturation,
  contrast and detail. **Reset** restores the sensor-aware defaults and 1x
  zoom.
- The countdown button offers the inherited 0, 3, 5 and 10 second choices;
  composition guidelines remain a persisted preference.

Zoom is a digital crop performed by Camerabin, not optical lens zoom. HDR,
manual shutter/ISO and flash remain unavailable until the lower camera stack
can implement and report them truthfully.

## Runtime requirements

- PipeWire and WirePlumber;
- the GStreamer PipeWire, GTK4, good, bad and ugly plugin sets;
- GTK 4.18 or newer and libadwaita 1.8 or newer;
- glycin, an XDG camera portal and matching desktop portal backend; and
- for libcamera phones, `pipewire-spa-libcamera` plus a camera stack that
  advertises the controls used by the interface.

The installed OnePlus 6T baseline is kernel r8, libcamera/IPA r24, PipeWire
libcamera SPA r7, Advanced Snapshot r4 and postmarketOS edge. The signed r7/r4
generation passed a coherent offline installation, all-sensor stream test,
correlated rear-focus result test, fixed-focus front fallback, packaged D-Bus
launch and a compositor-level live-pinch trace from 1.0x to 3.0x.
Generic webcams still use the inherited Snapshot paths; phone-specific
controls degrade safely when absent.

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

On the OnePlus 6T reference stack, the bounded non-image autofocus acceptance
test is documented in [tests/device](tests/device).

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

The independently named r4 build is installed beside `snapshot-50.0-r3`.
Truthful focus-result handling, its matching PipeWire transport and synchronized
pinch zoom have signed AArch64 packages, automated source/package checks and
coherent phone runtime acceptance. The original r2 gesture was correctly
rejected after physical testing showed that the full-screen controls overlay
kept touch sequences from a controller attached only to the viewfinder. In r3,
the gesture runs in capture phase on the controls/viewfinder common ancestor;
that fixed gesture recognition, but physical testing found that the camera crop
did not track a sustained pinch smoothly. r4 keeps the label immediate while
coalescing camera writes into a bounded, latest-value-wins 33 ms scheduler and
flushes the exact final value on gesture end, cancellation and capture. An
automated device trace now records intermediate 1.0x, 1.5x, 1.9x and 2.7x
states before the exact 3.0x endpoint. Physical r4 visual acceptance, saved
photos and video remain separate gates. The next application work is
resolution/aspect selection and more robust video status.

No photograph, raw frame, device identifier, account credential, proprietary
Android library or vendor tuning blob belongs in this repository.

## License and attribution

Advanced Snapshot is licensed under GPL-3.0-or-later. It preserves GNOME
Snapshot's copyright, license, translation history and contributor commits.
The fork is independent and is not endorsed by the GNOME project.
