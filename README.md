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
| Always-visible image-controls entry | Puts a labelled **Image Controls** button in a direct toolbar above the preview; it opens a bounded camera-page drawer over the lower preview so changes remain visible without opening Preferences or the hamburger menu | Implemented |
| Full-frame still selection | Saves the largest 4:3 mode up to 2048x1536 instead of preview resolution | Implemented and accepted on all three OnePlus 6T sensors |
| Reliable repeated phone stills | Releases the low-power preview, opens one fixed full-resolution raw stream for the JPEG, then restores preview instead of asking legacy Camerabin to retarget one PipeWire source between incompatible modes | Implemented; six-shot IMX371 stress plus IMX519/IMX376 captures passed without negotiation or recovery errors |
| Capture failure handling | Rejects missing, empty, directory and non-local still outputs before gallery insertion | Implemented |
| Software-ISP-friendly preview | Restricts the live pipeline to a selected supported 720p-class mode when the camera advertises concrete modes, while keeping still capture at the higher photo mode | Implemented; 1280x720/30 preview recovery accepted around repeated full-resolution captures |
| Latest-frame preview scheduling | Uses a one-buffer downstream-leaky queue so a slow compositor or software ISP drops old frames instead of showing a delayed viewfinder | Implemented |
| Serialized camera lifecycle | Coalesces duplicate starts, waits for camerabin to reach NULL before camera/source reconfiguration, and invalidates stale asynchronous starts during stop, teardown or recovery | Implemented; protects libcamera/PipeWire stream ownership |
| Sensor-aware tap-to-focus | Maps preview taps through letterboxing, crop and orientation into a real libcamera AF window | Implemented on supported rear cameras |
| Truthful focus reticle | Shows amber while a request is pending, green only for metadata-confirmed focus and red for failure; stale helpers cannot update a newer tap | Implemented; requires AF-state transport |
| Manual rear focus | Exposes the simple-IPA `LensPosition` range as a debounced 0–2 slider; the selected position is held until the next tap, reset or camera switch | Implemented on the OnePlus 6T rear modules; fixed-focus front is disabled |
| Exposure compensation | Requests standard -1 to +1 EV from the lower stack | Implemented |
| Manual shutter and analogue gain | Disables automatic exposure and submits real `ExposureTime` and `AnalogueGain` controls in microseconds and linear gain units | Implemented in source and lower-layer package; sensor-scene acceptance remains separate |
| Colour, contrast and detail | Sends standard saturation, contrast and sharpness controls to preview and capture | Implemented |
| Colour-processing presets | Offers Sensor default, Neutral, Natural and Vivid starting points without changing exposure, white balance, focus or a measured matrix; manual edits are labelled Custom | Implemented |
| Gamma tone control | Exposes the standard libcamera `Gamma` control for mid-tone tuning and selects the OnePlus sensor's conservative 2.0/2.1/2.2 startup default from the stable node model | Implemented on nodes advertising `Gamma`; calibration remains scene-dependent |
| Automatic/manual white balance | Keeps statistics-driven AWB enabled by default or submits standard red/blue `ColourGains` to the software ISP while green remains 1.0 | Implemented and live-validated on IMX371, IMX376 and IMX519 |
| Writable colour correction | Sends a bounded 3×3 `ColourCorrectionMatrix` together with manual white balance so chart-derived camera RGB corrections affect preview and capture in the software ISP | Implemented when the lower stack advertises the standard control; identity and a neutral-preserving colour-boost matrix are starting points, not factory calibration |
| Per-sensor calibration tool | Lets the user tune white balance, a 3×3 colour matrix, tone, exposure and focus against a grey card or colour chart, save a versioned profile, apply it later and optionally restore a deliberate manual focus position | Implemented in the Camera Calibration dialog; version 3 profiles are stored per stable sensor identity |
| Sensor-aware startup defaults | Applies tuned colour/contrast defaults when the provider selects the first camera as well as when the user switches cameras | Implemented |
| Bounded rear hardware flash | Offers an opt-in rear-LED pulse through `pmos-camera-flash`; the helper restores the previous LED values and is disabled for the front camera | Implemented in source; phone LED/capture acceptance pending |
| Software HDR exposure fusion | Captures dark, normal and bright JPEGs, aligns bounded whole-frame handheld translation, rejects clipped samples, merges them in linear light and writes one atomically installed JPEG; temporary frames are removed on success or failure | Implemented in source; phone image-quality acceptance pending |
| Synchronized digital zoom | The image-control slider, two-finger pinch gesture and toolbar value chip share one 1x–4x Camerabin zoom value; tapping the chip resets to 1x | Live 33 ms coalesced updates; the value was moved out of the preview so it cannot overlap the photo/video/QR selector |
| Photo, video and QR modes | Retains Snapshot's capture, recording, gallery and code-detection flows | Implemented |
| Focus-result state | Correlates each accepted trigger with libcamera `AfState` request metadata instead of treating control acceptance as optical success | Implemented and accepted with the OnePlus 6T r7 transport |
| Capture-after-focus barrier | Reapplies the last tap focus or manual lens position after the preview-to-photo stream hand-off, or waits for the new stream's own AF result before a still/HDR sequence starts | Implemented on the OnePlus 6T raw still path; best-effort fallback keeps fixed-focus and older stacks usable |
| Factory-calibrated colour | Validated sensor/module CCM, lens shading and proprietary ISP tuning | Not implemented and never shown as available |

“Android-class” is a feature-by-feature target, not a marketing claim. A
control is enabled only when the camera pipeline can implement and report it.
See [docs/FEATURES.md](docs/FEATURES.md) for the acceptance matrix and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the control path.

## Using the mobile controls

- Tap a subject in the rear-camera preview to request focus there. The square
  is amber while the request is pending, green only after focus metadata says
  `Focused`, and red after an optical or transport failure.
- Press the labelled **Image Controls** button in the toolbar above the preview (the
  hamburger menu keeps **Image Controls** as a fallback), then use **Manual
  focus position** to hold a rear lens at a
  chosen normalized position from 0 (far end) to 2 (near end). Moving the
  slider cancels continuous autofocus and applies the real actuator position;
  tap the preview to replace it with one-shot autofocus, or press **Reset** to
  restore continuous autofocus. The control is unavailable on the fixed-focus
  front camera.
- Still capture reapplies the last rear tap-focus window or manual lens
  position after opening the high-resolution photo stream. In automatic mode it
  waits for that new stream's autofocus scan to settle before exposing the
  sensor. A failed or unavailable result is logged and capture continues with
  the last stable lens position.
- Spread or pinch two fingers over the preview to zoom between 1x and 4x. The
  value chip in the toolbar above the preview and the **Main Menu → Image
  Controls → Zoom** slider stay in sync. Tap the value chip to return directly
  to 1x. Keeping the value in that toolbar leaves the photo/video/QR selector
  unobstructed.
- Open **Image Controls** for exposure compensation, manual focus, automatic
  white balance, red/blue gains, colour saturation, contrast, detail and
  Gamma. **Colour profile** provides Sensor default, Neutral, Natural and Vivid
  starting points; it changes only software-ISP tone/detail values, while
  exposure, white balance, focus and a measured matrix remain untouched. Any
  later tone edit is shown as **Custom**. White-balance gains affect both preview and capture in the software
  ISP; they are not a display tint. **Reset** restores automatic white balance,
  the sensor-aware tone defaults, continuous autofocus and 1x zoom.
- Select **Camera calibration** from **Image Controls** after placing a grey
  card or colour chart in even light. Start with automatic white balance, then
  disable it and adjust red/blue gains until a neutral target is neutral. If
  the camera advertises `ColourCorrectionMatrix`, enable **Use custom colour
  matrix** and tune the nine row-major camera-RGB-to-sRGB coefficients against
  a colour chart. Keep each row sum near 1 while correcting hue; **Identity**
  and **Colour boost** are safe starting points, not measured values. Adjust
  Gamma, Colour, Contrast, Detail, Exposure and focus while viewing the live
  preview, capture a reference photo, then press **Calibrate → Save Current
  Profile**. The profile is
  keyed to the stable physical sensor, so main, secondary and front-camera
  values do not overwrite one another. Leave **Restore manual focus** off for
  normal continuous autofocus; enable it only when a saved lens position is
  intentional. **Clear** removes that sensor's profile.
- Enable **Software HDR** to capture a dark, normal and bright frame and merge
  them into one JPEG. It requires automatic exposure; the three frames are
  intentionally captured without hardware flash. Small whole-frame shifts are
  aligned automatically, but keep the phone and subject still because local or
  non-rigid subject motion can still ghost.
- Leave **Automatic exposure** enabled for normal use. Turn it off to expose
  the **Shutter (µs)** and **Analogue gain** controls. These submit standard
  libcamera controls; the active sensor may clamp them to its safe range.
- On a rear camera, enable **Image Controls → Hardware flash** for one bounded
  LED pulse during the next still capture. It is off by default, requires the
  optional `oneplus6t-pmos-fixes` helper, and is disabled for the fixed-focus
  front camera.
- The countdown button offers the inherited 0, 3, 5 and 10 second choices;
  composition guidelines remain a persisted preference.

Zoom is a digital crop performed by Camerabin, not optical lens zoom. Software
HDR is exposure fusion, not the OnePlus vendor HDR pipeline: alignment is
limited to one global translation per bracket, with no local motion model,
local tone mapping, automatic flash metering, a factory-calibrated CCM or
lens-shading tables. A user-supplied colour matrix can be applied, but it is
not factory calibration unless it was measured with a controlled chart and
illuminant. Manual analogue gain is not the same thing as a vendor ISO
mode, and no vendor-specific ISO calibration is claimed. The hardware-flash switch is an
explicit, bounded LED pulse and is not used during HDR capture.

## Runtime requirements

- PipeWire and WirePlumber;
- the GStreamer PipeWire, GTK4, good, bad and ugly plugin sets;
- GTK 4.18 or newer and libadwaita 1.8 or newer;
- glycin, an XDG camera portal and matching desktop portal backend; and
- for libcamera phones, `pipewire-spa-libcamera` plus a camera stack that
  advertises the controls used by the interface.

The optional rear-flash control additionally needs the `pmos-camera-flash`
command from [oneplus6t-pmos-fixes](https://github.com/lolren/oneplus6t-pmos-fixes)
and writable `*:flash` LED channels. Without it, the rest of the application
continues to work and the switch remains unavailable.

Software HDR additionally installs the `advanced-snapshot-hdr` helper under
`/usr/libexec`. The helper only accepts three same-sized decoded images,
limits processing to 40 megapixels, estimates at most 96 pixels of global
translation from exposure-resistant luminance gradients, and keeps zero shift
when the match is ambiguous. It writes a temporary JPEG in the destination
directory and renames it atomically. It can also be run independently for
reproducible testing:

```sh
advanced-snapshot-hdr --output merged.jpg \
  --input dark.jpg --input normal.jpg --input bright.jpg
```

The installed OnePlus 6T lower-layer baseline is kernel r10, libcamera/IPA r33,
PipeWire libcamera SPA r8 and postmarketOS edge. The current app package is the
source-built r34 development line. The lower layer passes all-sensor stream
tests, correlated rear-focus results, fixed-focus front fallback and the
manual lens-position sweep. The r33 simple-IPA profiles add a bounded,
row-sum-preserving green-cast correction to all three sensors and expose a
reproducible equal-channel test-pattern check. This reduces the measured green
excess in controlled IMX519 and IMX376 rear captures while keeping neutral
frames neutral; it is not a factory CCM, lens-shading table or Android vendor
ISP replacement. The application also passes repeated native still capture on
IMX371 and one full-resolution capture after tap-focus on each rear module.
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
test and the standalone Camerabin negotiation probe are documented in
[tests/device](tests/device).

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

## Current OnePlus 6T acceptance

The current AArch64 package was built from commit
`0376f68c6808517fdc368d8e92ce67a0463ce960`. It includes the labelled
**Image Controls** entry, Gamma, sensor-model tone defaults, per-sensor
automatic/manual white balance, writable colour-matrix calibration, narrow-
screen **Camera calibration** profiles, an unobstructed toolbar zoom chip and
the reliable standalone full-resolution
still path. The exact package pair is recorded in `docs/VALIDATION.md` and is
installed on the connected OnePlus 6T without reboot. The main APK is
`advanced-snapshot-0.1.0-r34.apk`; its exact artifact hashes are recorded in
`packaging/postmarketos/README.md` and `docs/VALIDATION.md`.

The source-equivalent r28 candidate completed six consecutive IMX371 captures
and one capture after tap-focus on each rear sensor. Every file was a valid
2048x1536 baseline JPEG, preview recovered after every capture and the logs
contained no timeout, negotiation, bus, recovery or panic error. The exact
r29 package then repeated a full-resolution IMX371 capture and preview
recovery with the same zero-error result. This validates package installation,
repeated still lifecycle and all-sensor routing; it does not claim a factory
colour matrix, lens-shading calibration, proprietary
denoise or Android-vendor ISP parity.

The exact r30 package was subsequently launched at 1280x720/30 and switched
cleanly across IMX519, IMX376 and IMX371. Deliberately extreme manual red/blue
gain requests changed each live stream in the expected direction, and
restoring automatic white balance returned each stream to statistics-driven
neutral output. All sensors were left in automatic mode after the test.

With libcamera/IPA r30 and PipeWire SPA r8, both rear modules pass the native
focus helper regression and the all-camera Waydroid probe. Manual rear focus
is a normalized 0–2 device range, not a factory-calibrated distance scale.
Saved-photo colour/quality comparison against a controlled chart and Android
vendor processing remains an explicit acceptance gate. Current reference
images show softness/low local contrast on IMX519 and a green cast with
fixed-pattern grid noise on IMX376; these defects are tracked as calibration
work rather than hidden behind a stronger saturation preset.

## Project status

The current camera-quality line adds a real rear manual-focus control and
capture barrier to the existing tap-focus path. A tap is handled by a capture-
phase gesture on the Camera ancestor, so the full-screen controls overlay cannot
steal it; the request maps the displayed point through the preview crop and
orientation, waits for the correlated autofocus result, and leaves the lens at
the selected focus instead of starting a delayed reset scan. The manual slider
uses the same PipeWire helper and the standard `LensPosition` control. Reset
explicitly returns to continuous autofocus. IMX371, IMX376 and IMX519 startup
tone defaults are aligned with the lower-layer tuning (contrast 1.10,
saturation 1.35); this is a conservative software-ISP improvement, not a
calibrated colour-science or vendor-ISP result.

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
photos, preview latency and video remain separate gates. Video finalization now
keeps the shutter disabled until camerabin emits `video-done`, rejects empty or
non-file outputs before gallery insertion, and reports a failed save to the
user. The remaining video gate is a playable-file test on the recovered phone;
the next application work is on-phone preview-latency acceptance, followed by
resolution/aspect selection.

Commit `fed2784` adds a one-buffer, downstream-leaky queue to every preview
branch so slow conversion or composition drops stale frames instead of
displaying a delayed viewfinder. The latest source tree passes all four
application and eight Aperture unit tests in a clean native GTK/GStreamer build
environment. Its new AArch64 package is not claimed as installed: a matching
pmbootstrap device buildroot is still required before physical
preview-latency acceptance.

Revision `3ac146f` closes a negotiation gap in that optimization: when a
camera advertises concrete modes, the live caps now contain only the selected
720p-class mode instead of merely putting it first in a larger list. This
prevents a software ISP from silently selecting a full-resolution preview.
Range-only camera advertisements keep the generic fallback path. The revision
is source-tested but is not installed on the wedged reference phone yet.

The camera lifecycle guard coalesces repeated `start_stream()` calls and tags
each asynchronous GStreamer state request with a generation. `stop_stream()`,
camera changes, widget teardown and pipeline errors invalidate older
generations before changing `camerabin` to NULL. Camera changes and pipeline
restarts also wait for that NULL transition to complete before changing the
camera source or recording configuration. This matters on libcamera devices
because a late PLAYING transition or an early source replacement can otherwise
reconfigure a source after its buffers have already been released, producing
intermittent `not-negotiated`, allocator or stream-drain errors during rapid
open-close testing. The guard is generic and does not depend on OnePlus-specific
node names.

The current r32 source is commit
`aa9fea6464c580c308cefecc6383f57c58910102` and includes the same lifecycle
guard plus the Camerabin NULL barrier, GStreamer state-tuple compatibility fix,
rear manual-focus slider, explicit return to continuous autofocus, the
always-visible Image Controls entry, Gamma, per-sensor calibration profiles and
the bounded standalone full-resolution still path. Calibration profiles persist
automatic/manual white balance, bounded red/blue gains and an optional 3×3
colour matrix. The 1.0× chip is contained by the toolbar instead of covering
the capture-mode selector, and matrix coefficients use nine phone-width rows.
Native focus, all-sensor
preview and repeated saved stills are live-tested; calibrated colour, video and
long-run battery acceptance remain separate device gates.

No photograph, raw frame, device identifier, account credential, proprietary
Android library or vendor tuning blob belongs in this repository.

## License and attribution

Advanced Snapshot is licensed under GPL-3.0-or-later. It preserves GNOME
Snapshot's copyright, license, translation history and contributor commits.
The fork is independent and is not endorsed by the GNOME project.
