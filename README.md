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

## Current release and support status

This is the current application release record as of 2026-08-31:

| Item | Current value |
| --- | --- |
| Application version | `advanced-snapshot-0.1.0-r38` plus `advanced-snapshot-lang-0.1.0-r38` |
| Source commit | [`5e102b7d4b6bf6b4dcfeabe8f9040ffff8cc1ffd`](https://github.com/lolren/advanced-snapshot/commit/5e102b7d4b6bf6b4dcfeabe8f9040ffff8cc1ffd) |
| Release | [`r38-fresh-still-autofocus`](https://github.com/lolren/advanced-snapshot/releases/tag/r38-fresh-still-autofocus) |
| Target | postmarketOS edge, AArch64, musl |
| Matching phone lower layer | kernel r10, libcamera/IPA r35 and PipeWire SPA r8 |
| Main APK SHA-256 | `91d2c1c65d1eecbf7dca7e9f90eb69a78e60a123f9f66b662c48d5ebd81e27d5` |
| Language APK SHA-256 | `6ad6645feb9861c8d2305b19357b30c40d7572c4f957c0aa4985c92dfb568417` |
| Release verification key | `pmos@local-6a92d930.rsa.pub` |
| Verification-key SHA-256 | `c1f8892b9576ce1807732a985243311d272ab422fc30958a2fb78d5bfc8d36a6` |

r38 is an application-only update. It changes the independently named
Advanced Snapshot executable, resources, settings schema, language package and
HDR helper; it does not replace distro GNOME Snapshot, libcamera, PipeWire, the
kernel, firmware, Waydroid or any boot partition. The application-only update
does not require a reboot. Keep the previous application pair until the new
one has passed the phone-side checks you care about.

The release is source-built, signed and artifact-validated. The source and
container gates pass 35 workspace tests in total (15 application, 10 HDR
helper and 10 Aperture tests), and the package validator checks the AArch64
executables, exact file manifest, language split, metadata, schema, resources,
mobile layout contract and zero ownership overlap with distro Snapshot. The
r38 package is installed on the reference OnePlus 6T as an application-only
update. Final physical saved-photo acceptance still needs a normal graphical
user session and a repeatable focus target; build success is not presented as
proof of Android camera quality.

## Installation methods

There are four supported ways to use this project. Choose one method for an
installation; do not copy individual binaries into `/usr` or replace files
owned by the distro `snapshot` package.

### Method 1: install the signed r38 release APKs

This is the recommended method for a user of the OnePlus 6T. It needs a
booted AArch64 postmarketOS installation, `apk-tools`, `curl` or a browser,
and root permission for the package transaction. Run the application as the
normal graphical user, not from fastboot, EDL, an SSH root shell or a different
desktop user. A working graphical session is needed to launch and visually
test the camera, but not to download or verify the APKs.

Download the five release assets with GitHub CLI:

```sh
mkdir -p "$HOME/Downloads/advanced-snapshot-r38"
cd "$HOME/Downloads/advanced-snapshot-r38"
gh release download r38-fresh-still-autofocus \
  --repo lolren/advanced-snapshot \
  --pattern advanced-snapshot-0.1.0-r38.apk \
  --pattern advanced-snapshot-lang-0.1.0-r38.apk \
  --pattern pmos@local-6a92d930.rsa.pub \
  --pattern SHA256SUMS \
  --pattern RELEASE-NOTES.md
```

The same files can be downloaded from the release page in a browser. Do not
skip verification merely because the files came from GitHub: a release URL
does not replace a checksum and signature check. With all five files in the
same directory, verify the release and the public key:

```sh
sha256sum -c SHA256SUMS
test "$(sha256sum pmos@local-6a92d930.rsa.pub | awk '{print $1}')" = \
  c1f8892b9576ce1807732a985243311d272ab422fc30958a2fb78d5bfc8d36a6
```

The key is public and safe to install. The corresponding private signing key
must never be copied to the phone, committed to Git or published. Install the
verified public key, simulate the exact package transaction, inspect the
output, then apply it:

```sh
sudo install -m 0644 pmos@local-6a92d930.rsa.pub /etc/apk/keys/

sudo apk add --simulate --upgrade --network=no --no-interactive \
  ./advanced-snapshot-0.1.0-r38.apk \
  ./advanced-snapshot-lang-0.1.0-r38.apk

sudo apk add --upgrade --no-interactive \
  ./advanced-snapshot-0.1.0-r38.apk \
  ./advanced-snapshot-lang-0.1.0-r38.apk
```

The simulation should show the two Advanced Snapshot packages being added or
upgraded and no removal of `snapshot`, camera libraries, PipeWire, Waydroid or
the kernel. If the package key, architecture, version or transaction is not
what you expect, stop and keep the old installation. Verify the result with:

```sh
apk info -e advanced-snapshot
apk info -e advanced-snapshot-lang
apk info -a advanced-snapshot | sed -n '1,20p'
command -v advanced-snapshot
```

Close any older camera window before launching `advanced-snapshot`. No reboot
is required. On the OnePlus 6T, the optional rear hardware-flash control also
needs the `oneplus6t-pmos-fixes` package and writable rear LED channels; all
other camera controls remain usable without that optional helper.

### Method 2: use the OnePlus 6T verification wrapper

The companion repository contains a pinned, simulation-first installer for
this exact r38 pair. It downloads the release over HTTPS, verifies
`SHA256SUMS`, verifies the release key fingerprint, checks both APK
signatures, and only changes Advanced Snapshot when `--apply` is supplied.
The wrapper does not touch the native camera generation, Waydroid, kernel or
firmware and does not reboot.

Run it on the phone as the normal graphical login user. It is also suitable
for a checkout copied to the phone over USB networking or SSH:

```sh
git clone --depth 1 https://github.com/lolren/oneplus6t-pmos-fixes.git
cd oneplus6t-pmos-fixes

# Simulation only; downloads and verifies the exact r38 assets.
./scripts/install-advanced-snapshot

# Apply only after reviewing the simulation output.
./scripts/install-advanced-snapshot --apply
```

If the companion runtime package is already installed, the packaged command
is equivalent:

```sh
pmos-install-advanced-snapshot
pmos-install-advanced-snapshot --apply
```

The command retains its verified download directory for inspection. Set
`PMOS_SNAPSHOT_WORK_DIR` or pass `--work-dir` when the evidence and APKs must
remain in a specific location. The helper's default is simulation-only. The
separate `install-camera-generation` wrapper in the companion repository is
for a complete native r35/r36 camera generation, not for this app-only update;
use it only with the lower-layer procedure documented there.

### Method 3: build the postmarketOS APK from the pinned source

This method is for maintainers or users who want a locally reproducible
package. Building does not require root; installing the resulting APK does.
Use a current `pmbootstrap`, a pmaports checkout that can build postmarketOS
edge AArch64 packages, Rust/Cargo support provided by the recipe, and enough
disk space for a clean cross-build. The source recipe is pinned to the full
r38 commit above; do not replace it with a moving branch archive.

```sh
git clone https://github.com/lolren/advanced-snapshot.git
git -C advanced-snapshot checkout \
  5e102b7d4b6bf6b4dcfeabe8f9040ffff8cc1ffd

git clone https://gitlab.com/postmarketOS/pmaports.git
mkdir -p pmaports/temp/advanced-snapshot
cp advanced-snapshot/packaging/postmarketos/APKBUILD \
  advanced-snapshot/packaging/postmarketos/cargo-auditable.patch \
  pmaports/temp/advanced-snapshot/

pmbootstrap -p "$PWD/pmaports" build --arch aarch64 advanced-snapshot
```

The resulting `advanced-snapshot-*.apk` and
`advanced-snapshot-lang-*.apk` are normally under the pmbootstrap package
cache. Locate them without assuming a particular pmbootstrap home directory:

```sh
find "$HOME/.local/var/pmbootstrap/packages" \
  -type f \( -name 'advanced-snapshot-*.apk' -o \
             -name 'advanced-snapshot-lang-*.apk' \) -print
```

Before installing a local build, run the repository validator. It verifies
the APK signature, AArch64 ELF headers, exact payload, independent D-Bus and
GSettings identifiers, mobile GtkBuilder layout, language split and optional
ownership overlap with distro Snapshot:

```sh
APK_VERIFY_TOOL="$HOME/.local/var/pmbootstrap/apk.static" \
APK_KEY_DIR="$HOME/.local/var/pmbootstrap/config_apk_keys" \
  ./advanced-snapshot/packaging/postmarketos/validate-apk.sh \
  /path/to/advanced-snapshot-0.1.0-r38.apk \
  /path/to/snapshot-50.0-r3.apk \
  /path/to/advanced-snapshot-lang-0.1.0-r38.apk
```

The second APK argument is optional when a matching distro Snapshot package
is not available locally. A local pmbootstrap build is normally signed by the
buildroot development key, not by the public release key. Trust only the
matching public key from that buildroot, or rebuild/re-sign through your own
reviewed package repository. Do not use an unverified local APK merely because
`apk add --allow-untrusted` can install it.

Install a validated local pair with the same simulation-first rule as a
release pair. If the package is signed by a key not already trusted by the
phone, install that verified public key first or use `--allow-untrusted` only
after independently checking its signature and checksum:

```sh
sudo apk add --simulate --upgrade --network=no --no-interactive \
  /path/to/advanced-snapshot-0.1.0-r38.apk \
  /path/to/advanced-snapshot-lang-0.1.0-r38.apk
sudo apk add --upgrade --no-interactive \
  /path/to/advanced-snapshot-0.1.0-r38.apk \
  /path/to/advanced-snapshot-lang-0.1.0-r38.apk
```

### Method 4: build the application for development

For UI, Rust or GStreamer development, use the native Meson build. This
creates a staged tree and never overwrites the distro camera application:

```sh
meson setup build --prefix=/usr
meson compile -C build
meson test -C build --print-errorlogs
DESTDIR="$PWD/stage" meson install -C build
find stage -type f -o -type l
```

The stage should contain `/usr/bin/advanced-snapshot`, the focus and HDR
helpers, the independently named D-Bus service, schema, metainfo, resource
bundle and icons. It must not contain or overwrite `org.gnome.Snapshot` paths.
For a complete list of dependencies and the phone-specific capture rationale,
see [docs/INSTALL.md](docs/INSTALL.md).

### Rollback and removal

Keep the previous APK pair and the verified release/build key until physical
testing is complete. Advanced Snapshot is deliberately separate from GNOME
Snapshot, so a failed app update can be undone without touching the lower
camera stack:

```sh
# Verify the retained older pair first, then install it explicitly.
sudo apk add --upgrade /path/to/advanced-snapshot-OLDER.apk \
  /path/to/advanced-snapshot-lang-OLDER.apk

# Or remove only this project and return to distro Snapshot.
sudo apk del advanced-snapshot advanced-snapshot-lang
```

Removing the app does not restore or remove libcamera, PipeWire, the kernel or
Waydroid packages. Do not remove shared lower-layer packages as an app
rollback. Keep a package key in `/etc/apk/keys` while any installed or
archived package relies on it; remove it only after those packages and release
artifacts are no longer needed.

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
| Colour-processing presets | Offers Sensor default, Neutral, Natural and Vivid starting points without changing exposure, white balance, focus or a measured matrix; Image Controls also provides a one-tap green-cast correction and the calibration dialog provides the same starting matrix | Implemented |
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
  position after opening the high-resolution photo stream. With neither set, it
  requests a fresh large centre-weighted autofocus scan on that still stream
  and waits for its result, avoiding a stale terminal state from the preview.
  A failed or unavailable result is logged and capture continues with the last
  stable lens position.
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
- If the selected camera still has a green cast, press **Green-cast correction →
  Apply** in the same drawer. This applies the r35, row-sum-preserving
  OnePlus starting matrix to the live preview and saved captures, and turns off
  automatic white balance because the standard libcamera matrix control is only
  active in manual-WB mode. Press **Reset** to undo it. The lower-layer sensor
  profiles remain the automatic path; use **Camera calibration** and a grey card
  or colour chart when a scene- or sensor-specific correction is needed. The
  preset is a starting point, not factory ISP calibration.
- Select **Camera calibration** from **Image Controls** after placing a grey
  card or colour chart in even light. Start with automatic white balance, then
  disable it and adjust red/blue gains until a neutral target is neutral. If
  the camera advertises `ColourCorrectionMatrix`, enable **Use custom colour
  matrix** and tune the nine row-major camera-RGB-to-sRGB coefficients against
  a colour chart. Keep each row sum near 1 while correcting hue; **Identity**,
  **Green-cast correction** and **Colour boost** are safe starting points, not
  measured values. Green-cast correction uses the same stronger r35
  grey-preserving matrix as the native OnePlus profiles and automatically selects
  manual white balance so the matrix is active. Adjust
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

The installed OnePlus 6T lower-layer baseline is kernel r10, libcamera/IPA r35,
PipeWire libcamera SPA r8 and postmarketOS edge. The current app package is the
source-built r38 development line. The lower layer passes all-sensor stream
tests, correlated rear-focus results, fixed-focus front fallback and the
manual lens-position sweep. The r35 simple-IPA profiles add a bounded,
row-sum-preserving green-cast correction to all three sensors and expose a
reproducible equal-channel test-pattern check. This reduces the measured green
excess in controlled IMX519 and IMX376 rear captures while keeping neutral
frames neutral; it is not a factory CCM, lens-shading table or Android vendor
ISP replacement. The application also passes repeated native still capture on
IMX371 and one full-resolution capture after tap-focus on each rear module.
The r38 application preset now uses the exact same stronger matrix as those
native profiles: `[0.90, 0.10, 0.00; 0.10, 0.80, 0.10; 0.00, 0.10, 0.90]`.
It is applied to preview and saved captures only when the user selects the
visible Green-cast correction action; the action deliberately switches to
manual white balance because that is required by the standard matrix control.
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

## What has been achieved

The project now has a complete, independently installable application layer
for the open OnePlus 6T postmarketOS camera stack. The important distinction is
between an implemented control, a host/package test and a physical photo test:
the first two are complete for the r38 release, while the last one still
requires a normal graphical session and controlled targets on the phone.

### Application and capture path

- A separate `advanced-snapshot` binary, D-Bus name, icon, AppStream metadata,
  GSettings schema and language package can coexist with distro GNOME
  Snapshot. No distro-owned Snapshot file is overwritten.
- **Image Controls** is a labelled toolbar entry directly on the camera page.
  The drawer is bounded and scrollable while the live preview remains visible,
  so exposure, focus, colour and zoom changes do not require navigating to
  Preferences.
- The phone path chooses a practical 1280x720-class live preview and a
  separate 2048x1536 full-resolution 4:3 still stream when the camera advertises
  those modes. It stops and restores the preview around the still stream rather
  than asking the legacy Camerabin source to renegotiate incompatible modes.
- Repeated still capture is serialized, bounded and failure-aware. Missing,
  empty, directory and non-local outputs are rejected before gallery insertion;
  preview recovery and temporary-pipeline teardown are handled on success,
  timeout, cancellation and error.
- Rear tap focus maps through preview crop, letterboxing and orientation into a
  real sensor AF region. The reticle is amber while pending, green only after
  focus metadata confirms success and red on a reported failure.
- Rear manual focus exposes the standard `LensPosition` control as a
  normalized 0–2 slider. A tap replaces the manual lock with one-shot AF and
  **Reset** returns to continuous AF. The fixed-focus front camera does not
  pretend to support a lens actuator.
- Most importantly for r38, a still capture with neither a tap point nor a
  manual lock requests a fresh centre-weighted one-shot autofocus scan on the
  new full-resolution still stream. It no longer trusts a terminal AF result
  left over from the old preview stream.
- Pinch zoom, the visible 1x–4x value and the Image Controls slider share one
  coalesced, latest-value-wins control path. The zoom chip is in the toolbar,
  not over the photo/video/QR selector.
- Photo, video and QR modes, gallery handling, countdown choices and
  composition guidelines remain available from the upstream application.

### Image controls and processing

- Exposure compensation, manual shutter time, analogue gain, saturation,
  contrast, detail and Gamma use standard libcamera controls where advertised.
  Automatic exposure and white balance remain the safe defaults.
- Sensor default, Neutral, Natural, Vivid and Custom profiles provide visible
  starting points without silently changing focus, exposure, white balance or a
  measured matrix.
- The calibration dialog stores versioned profiles per stable physical sensor.
  Profiles can contain automatic/manual white balance, bounded red/blue gains,
  tone/detail values, a 3x3 colour-correction matrix and an optional deliberate
  manual focus position.
- The visible **Green-cast correction** action uses the repeatable,
  row-sum-preserving starting matrix
  `[0.90, 0.10, 0.00; 0.10, 0.80, 0.10; 0.00, 0.10, 0.90]` and switches to
  manual white balance because that is required for the standard matrix
  control. **Reset** restores the automatic path.
- Software HDR captures dark, normal and bright JPEGs, performs bounded
  whole-frame translation alignment and merges in linear light into one
  atomically installed JPEG. Hardware flash is intentionally not used during
  HDR.
- The optional rear-flash switch calls the separate bounded helper only for a
  rear still and restores LED state on completion or interruption.

### Validation and packaging

- The pinned source archive, AArch64 package build, release signature, checksum
  file and release public key are recorded in `docs/VALIDATION.md` and the
  r38 release page.
- The release validator checks ELF architecture, exact manifest, package
  metadata, independent runtime identifiers, desktop/D-Bus/AppStream files,
  schema compilation, mobile calibration layout, language ownership and no
  overlap with distro Snapshot.
- The clean source/container test gates pass 35 tests: 15 application, 10 HDR
  helper and 10 Aperture tests. The package can be upgraded or removed without
  changing the native camera generation.
- The companion OnePlus repository contains the signed-release installer,
  simulation-first package policy, native camera-generation manager and
  rollback documentation. This repository remains the application layer; the
  matching kernel, libcamera/IPA, PipeWire SPA, Waydroid and power work is not
  copied into it.

## Work remaining and acceptance gates

The following items are deliberately not marked complete by r38:

1. **Physical r38 saved-photo acceptance.** Log in to the normal postmarketOS
   graphical session, place repeatable near and far targets in good light, and
   test no-tap autofocus, rear tap focus, manual focus, camera switching and
   saved JPEG sharpness on both rear modules. The source fix and package gates
   pass; the saved-photo result must still be observed on the phone.
2. **Full image-quality calibration.** The green-cast preset is an open,
   scene-level starting matrix. Factory CCMs, lens-shading tables, proprietary
   denoise, vendor HDR, vendor tone mapping and Android computational
   photography are not available in this repository. Measure a grey card and
   colour chart under a known illuminant before claiming parity.
3. **Physical video/HDR/flash checks.** Confirm playable native video, live
   preview latency, HDR ghosting/merge quality, LED pulse/restoration and all
   control changes on all supported sensors. A source implementation or unit
   test is not a substitute for these phone tests.
4. **Android/Waydroid parity.** The companion repository has an open Camera3
   provider and Google-free Vanilla image, but Android camera applications,
   frame rate, vendor image processing, Maps/location and long open/close
   soaks remain separate acceptance work. Rear auxiliary hardware video is
   intentionally disabled after a reproducible Venus teardown fault.
5. **Portability and upstreaming.** Generic Linux cameras safely use inherited
   Snapshot paths, but phone-specific controls need capability detection. The
   next upstreaming work is to split generic lifecycle/UI fixes from the
   OnePlus-specific policy and keep the recipe pinned while each dependency is
   rebased and retested.

Do not interpret “work remaining” as an instruction to flash a boot image. The
r38 application update is intentionally userspace-only. Any lower-layer or
kernel change belongs in the companion repository and must retain an exact
rollback package before it is considered for a phone.

## Current OnePlus 6T acceptance record

The matching phone baseline is kernel r10, libcamera/IPA r35 and PipeWire SPA
r8. Earlier lower-layer tests accepted all three sensor streams, correlated
rear focus results, fixed-focus front fallback, normalized rear manual focus,
automatic/manual white balance and matrix transport. Repeated full-resolution
native stills and preview recovery passed on the lower stack, but the current
r38 application-level no-tap fresh-still AF correction still needs the visual
saved-photo check described above.

The exact r38 release artifacts are:

```text
advanced-snapshot-0.1.0-r38.apk       91d2c1c65d1eecbf7dca7e9f90eb69a78e60a123f9f66b662c48d5ebd81e27d5
advanced-snapshot-lang-0.1.0-r38.apk  6ad6645feb9861c8d2305b19357b30c40d7572c4f957c0aa4985c92dfb568417
source archive SHA-512                fc33c1ad639e67662929e104963593f9dc70974505dc47632ce7e3c40771825325cf0ebe09396c6ccf4b6b973ae540aa54c61f8c5384a9ff00f15c6b241d2a33
release key SHA-256                   c1f8892b9576ce1807732a985243311d272ab422fc30958a2fb78d5bfc8d36a6
```

The release and installation procedure are intentionally separate from
historical r0–r37 checkpoints. Those checkpoints remain available in
`docs/VALIDATION.md` for regression archaeology; this README's current status
and commands refer to r38.

No photograph, raw frame, device identifier, account credential, proprietary
Android library or vendor tuning blob belongs in this repository.

## Historical validation notes (r0-r37)

The current AArch64 package was built from commit
`5e102b7d4b6bf6b4dcfeabe8f9040ffff8cc1ffd`. It includes the labelled
**Image Controls** entry, Gamma, sensor-model tone defaults, per-sensor
automatic/manual white balance, writable colour-matrix calibration, narrow-
screen **Camera calibration** profiles, an unobstructed toolbar zoom chip and
the reliable standalone full-resolution still path plus the visible
**Green-cast correction** action. The exact package pair is recorded in
`docs/VALIDATION.md`. The main APK is `advanced-snapshot-0.1.0-r38.apk`; its
SHA-256 is
`91d2c1c65d1eecbf7dca7e9f90eb69a78e60a123f9f66b662c48d5ebd81e27d5`, and the
release-signed language APK is `advanced-snapshot-lang-0.1.0-r38.apk` with
SHA-256 `6ad6645feb9861c8d2305b19357b30c40d7572c4f957c0aa4985c92dfb568417`.
They are signed by `pmos@local-6a92d930.rsa.pub`.
The r38 still-capture path now requests a fresh centre-weighted one-shot
autofocus scan when there is no tap-focus point or manual lens lock, so it does
not reuse a terminal autofocus result left by the preview stream.
The exact artifact hashes are also recorded in
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

### Historical project status (superseded by the current status above)

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
