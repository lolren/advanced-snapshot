# Installation

The source builds as an independently named application. The reviewed
postmarketOS aport in `packaging/postmarketos` is the supported installation
target; direct replacement of distro Snapshot files is not.

## Staged source install

```sh
meson setup build --prefix=/usr
meson compile -C build
meson test -C build --print-errorlogs
DESTDIR="$PWD/stage" meson install -C build
```

Inspect the staging tree before packaging. It must contain
`/usr/bin/advanced-snapshot`,
`/usr/libexec/advanced-snapshot-focus-control`,
`/usr/libexec/advanced-snapshot-hdr`, the independent D-Bus service,
GSettings schema, metainfo, resources and icons under the
`io.github.lolren.AdvancedSnapshot` name. It must not contain or overwrite
`org.gnome.Snapshot` paths.

The optional **Hardware flash** switch is supplied by the separate
`oneplus6t-pmos-fixes` package. On a OnePlus 6T installation, install that
package alongside Advanced Snapshot and verify the helper before opening the
camera:

```sh
pmos-camera-flash --status
```

The status command is read-only. A report with no writable `*:flash` channels
keeps the switch disabled; it does not modify LED state.

On a rear autofocus camera, the still-capture path repeats the selected focus
operation after the preview-to-photo hand-off. A tap focus is submitted again
to the new full-resolution stream, a manual lens position is held and given a
short settle period, and automatic mode waits for that stream's published
autofocus state before releasing the JPEG. This matters because the raw still
pipeline has its own lens state; waiting on the old preview stream alone can
still save a blurred photo. The wait is bounded and best-effort, so fixed-focus
cameras and older PipeWire stacks retain the normal capture path.

On cameras advertising a concrete raw still mode, Advanced Snapshot does not
ask `wrappercamerabinsrc` to renegotiate the live PipeWire source from preview
resolution to photo resolution. The application stops the 1280x720 preview,
opens a temporary fixed 2048x1536 raw pipeline, discards one second of warm-up
frames for 3A convergence, reapplies focus on the new stream, encodes exactly
one JPEG, validates the output and then restores preview. Capture has a
15-second timeout and teardown/cancellation
always returns the temporary pipeline to NULL. Cameras without a suitable raw
mode keep the inherited Camerabin path. The rationale and diagnostic probe are
documented in `tests/device/README.md`.

On the OnePlus 6T, **Image Controls → Manual focus position** sends the
standard `LensPosition` control through the installed helper. Use 0 for the far
end and 2 for the near end; the slider is disabled for the fixed-focus front
camera. A preview tap returns to one-shot autofocus, and **Reset** returns to
continuous autofocus. A still capture with no tap or manual lock performs its
own centre-weighted one-shot autofocus on the high-resolution stream before
encoding. If the control is missing, the UI remains usable and logs the
unavailable capability instead of writing raw V4L2 values.

The **Gamma** slider sends the standard `Gamma` property when the selected
camera advertises it. The OnePlus 6T lower layer starts the IMX371, IMX376 and
IMX519 sensors at the conservative 2.0, 2.1 and 2.2 values respectively. The
**Camera calibration** button opens a mobile dialog for tuning Gamma, Colour,
Contrast, Detail, Exposure and focus against a grey card or colour chart. It
also keeps automatic white balance on by default and exposes standard red/blue
`ColourGains` when automatic mode is disabled. Start in automatic mode, then
adjust the manual gains until a neutral target is neutral. With the matching
lower stack, **Use custom colour matrix** additionally submits the standard
3×3 `ColourCorrectionMatrix` while white balance is manual. Tune its row-major
coefficients only against a controlled chart; keep each row sum near 1 so grey
stays neutral. **Identity** and **Colour boost** are starting points and are not
factory calibration. Save the current values after capturing a reference
photo; the profile is versioned
and keyed by the stable libcamera node identity, not the PipeWire serial. Use
**Apply Saved Profile** to restore it, **Restore manual focus** only when a
fixed lens position is intentional, and **Clear Saved Profile** to return to
the built-in defaults. This profile tool can retain repeatable white-balance
gains and a measured CCM for a known illuminant, but it cannot create factory
lens-shading or proprietary denoise data.

For a quick starting point, **Image Controls → Colour profile** offers
**Sensor default**, **Neutral**, **Natural** and **Vivid**. These change only
Gamma, Colour, Contrast and Detail; they do not overwrite exposure, white
balance, focus or a measured matrix. Editing one of those four sliders changes
the selector to **Custom**. Save a tuned result through **Calibrate → Save
Current Profile** when it should survive camera selection and app restarts.

If the selected OnePlus 6T camera still looks green, press **Image Controls →
Green-cast correction → Apply**. The r37 package applies the exact r35
row-sum-preserving matrix used by the native IMX371/IMX376/IMX519 profiles to
the live preview and saved captures, and turns automatic white balance off, as
required for a standard `ColourCorrectionMatrix` request.
Press **Reset** to return to automatic white balance and the sensor defaults.
The action applies to the currently selected camera; for a camera-specific
result, use **Camera calibration** with a grey card or colour chart and save the
profile. The starting matrix is not factory calibration.

The Software HDR switch uses the installed `advanced-snapshot-hdr` helper. It
is off by default and requires automatic exposure. It creates three hidden
temporary JPEGs, merges them, atomically installs the final JPEG and removes
the temporary files. It aligns small whole-frame translations automatically,
but keep the phone and subject still: independently moving subjects, rotation,
parallax and non-rigid motion can still ghost.

## Build the postmarketOS package

Copy `packaging/postmarketos/APKBUILD` and `cargo-auditable.patch` into
`temp/advanced-snapshot` in a current pmaports checkout, then run:

```sh
pmbootstrap -p /path/to/pmaports build --arch aarch64 advanced-snapshot
```

Before installation, run `packaging/postmarketos/validate-apk.sh` against the
new APK and the locally built distro Snapshot APK. The validator rejects wrong
architectures, unexpected files, invalid metadata, old runtime identifiers and
any file ownership collision. It also verifies the APK signature against the
development public key in `packaging/keys`; on a pmbootstrap workstation, set
`APK_VERIFY_TOOL="$HOME/.local/var/pmbootstrap/apk.static"`. See
`packaging/postmarketos/README.md` and `docs/VALIDATION.md` for the exact source
pin and reference results.

## OnePlus 6T r37 package

The current reproducible package is source commit
`71e3378aacf59c87696af8acd2086418dfa0ea64`, package revision r37. Build it
from the pinned recipe as described above, validate both APKs, and copy them
to a booted phone. It contains the visible Green-cast correction action and
uses the same stronger `[0.90, 0.10, 0.00; 0.10, 0.80, 0.10; 0.00, 0.10,
0.90]` matrix as the native OnePlus sensor profiles. It automatically disables
AWB when that matrix is selected. The
package is independent of distro Snapshot, so it can be upgraded or removed
without replacing `/usr/bin/snapshot`:

```sh
scp advanced-snapshot-0.1.0-r37.apk \
  advanced-snapshot-lang-0.1.0-r37.apk \
  packaging/keys/pmos@local-6a92d930.rsa.pub user@PHONE:/tmp/
ssh user@PHONE 'sudo install -m 0644 /tmp/pmos@local-6a92d930.rsa.pub \
  /etc/apk/keys/ && sudo apk add \
  /tmp/advanced-snapshot-0.1.0-r37.apk \
  /tmp/advanced-snapshot-lang-0.1.0-r37.apk'
```

Verify the downloaded APKs before copying them; the release hashes are in
`docs/VALIDATION.md`. The public key is safe to install, but never copy or
publish the corresponding private signing key.

Stop any running Advanced Snapshot window before replacing the files, then
launch `advanced-snapshot` again. No phone reboot is required for an
application-only update. Keep the previous APK pair until the new preview,
focus, calibration, still and video checks pass; remove only the independent
package to roll back to distro Snapshot.

## Installation policy

The APK will be built in a reviewed pmaports overlay, signed with the
VibeMarketOS repository key, installed only after `apk upgrade --simulate`
shows the expected addition, and retained beside its exact previous version.
The installer will run native camera smoke tests before marking the generation
healthy. A failed launch, stream, focus or capture check leaves the prior app
and camera package generation available for rollback.

The release installer is still in development. The r7/r1 package and focus
transport have passed coherent non-image phone acceptance, but until native
visual photo and video acceptance passes on the reference phone, continue
keeping the known-good `snapshot-50.0-r3` package installed beside it.

## Development package installation

Verify the public key before trusting it:

```sh
sha256sum packaging/keys/pmos@local-6a8b0868.rsa.pub
# expected: 31d5d6663ebe400a93fd3d5a107da2ea4dd96e8f6835ba1cdfecf89389ec16f6
sudo install -m 0644 packaging/keys/pmos@local-6a8b0868.rsa.pub /etc/apk/keys/
```

Copy the main and language APKs to the phone, preserve `/etc/apk/world`, and
simulate the exact local-file transaction:

```sh
cp /etc/apk/world "$HOME/world.before-advanced-snapshot"
sudo apk add --simulate ./advanced-snapshot-0.1.0-r0.apk \
  ./advanced-snapshot-lang-0.1.0-r0.apk
```

apk-tools 3 prints a realistic “Installing” transaction during simulation;
confirm `apk info -e advanced-snapshot` is still false and the saved world hash
is unchanged. Proceed only when the simulation contains two additions and no
upgrade, downgrade or removal:

```sh
sudo apk add ./advanced-snapshot-0.1.0-r0.apk \
  ./advanced-snapshot-lang-0.1.0-r0.apk
apk info -W /usr/bin/advanced-snapshot /usr/bin/snapshot
```

With both camera applications closed, validate the installed helper without
retaining scene data:

```sh
./tests/device/validate-pipewire-af.sh \
  --output "$HOME/advanced-snapshot-af-smoke" \
  --stability-seconds 60
```

That r0 procedure records local-package identity constraints in
`/etc/apk/world`. Do not later expect a package-name-only `apk upgrade` to
replace them. The accepted OnePlus 6T r6/r0-to-r7/r1 transaction supplies the
two r1 app APK paths explicitly and resolves PipeWire r7 from a separate
offline repository:

```sh
stage=/absolute/path/to/camera-r7-r1
sudo apk add --simulate --upgrade --allow-untrusted --network=no \
  --interactive=no --repository "$stage/candidate" \
  "$stage/candidate/aarch64/advanced-snapshot-0.1.0-r1.apk" \
  "$stage/candidate/noarch/advanced-snapshot-lang-0.1.0-r1.apk"
```

Require exactly the PipeWire r6-to-r7 and both Advanced Snapshot r0-to-r1
upgrades, with no removal, before running the same command without
`--simulate`. The expected reference world-file change is exactly two updated
Advanced Snapshot identity lines. Build the candidate and rollback repositories
and follow the complete service, hash and rollback procedure in the
[OnePlus 6T packaging guide](https://github.com/lolren/oneplus6t-pmos-fixes/blob/main/packaging/pmaports/README.md).

For an initial r0 side-by-side installation, rollback removes only the
independent packages:

```sh
sudo apk del advanced-snapshot advanced-snapshot-lang
```

The development public key may remain for later signed builds. Remove it only
when no installed or archived package still relies on that trust anchor.
