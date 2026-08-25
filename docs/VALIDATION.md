# Validation record

## 0.1.0 packaging checkpoint

- Date: 2026-08-25
- Source commit: `a73a3993263adb33604a1fad7f88d5e53e75f4c0`
- Target: postmarketOS edge, aarch64, musl
- Build tool: pmbootstrap 3.11.1; initial strict buildroot validation followed
  by an exact published-source recipe rebuild in the same aarch64 buildroot
- Lower camera stack present in the buildroot: libcamera/IPA r24 and PipeWire r6
- Source archive SHA-512:
  `c4ec8493cac938e665d8cf9fc50c97503beb0f0a35e700374c84f915b08d0eaaffc83caa417e19db8e361cb8e5ecd668dde87518e9f69ca876f9cdf64b3581ea`

The exact tracked recipe produced these reference local SHA-256 values:

- main APK:
  `f76372802060de0722cddec238da63ec97dfeae7faf6dc29058bd061fed63bad`;
- language APK:
  `13c9078e499a22ea292f9024b443dbd37d9c9181fb4cc18dbb810665cfd1cd43`.

APK signatures, metadata and timestamps can make hashes build-specific; the
pinned source hash and package validator are the reproducibility anchors.
The committed zero-context `cargo-auditable.patch` was checksum-verified and
produced byte-identical patched Meson files to the patch used by the reference
build; only mail-patch framing and whitespace were removed before commit.

### Passed

- locked Cargo dependency vendoring;
- release build with auditable dependency metadata;
- Cargo test build with zero failures (the current workspace contains no Rust
  unit tests, so this is a compile/link check rather than behavioral coverage);
- AArch64 ELF and musl interpreter inspection for the app and focus helper;
- desktop, AppStream and GSettings validation;
- all compiled resources under `/io/github/lolren/AdvancedSnapshot/`;
- exactly nine files in the main APK, all under independent names;
- no `org.gnome.Snapshot`, `/org/gnome/Snapshot/` or old helper identifier;
- zero file ownership overlap with `snapshot-50.0-r3`; and
- no Rust future-compatibility warnings in the final build.

### Still required at this checkpoint

- side-by-side package installation on the OnePlus 6T;
- native launch and all-three-camera preview checks;
- rear-camera tap/reset focus checks and front fixed-focus fallback;
- saved full-frame JPEG decode and framing check;
- bounded video record, stop and playback; and
- rollback to the prior package generation.

The distro `snapshot-50.0-r3` remains the known-good camera application until
those phone checks pass.

## 0.1.0-r1 truthful autofocus package checkpoint

- Date: 2026-08-25
- Source commit: `f163794d0bd4b796b4f8555c9af1a1e51f42ebf7`
- Target: postmarketOS edge, aarch64, musl
- Source: immutable GitHub commit archive
- Source archive SHA-512:
  `5d1c8197cbe368e6e88313d6fd5e997e5a3cf5aeb442e490ae4be6492c75d0f2721b007e6400c072ac6361445f82fbb216bc258e8dbab19c3c62980d92c0b83d`
- Main APK SHA-256:
  `1e19e6d3bfa990d9ae4440fcc0364383e7cfc36de835689d2a2d5d1748368795`
- Language APK SHA-256:
  `7329bc3133cacd288e1f95e9cb93e69f71acc986b0bf1a875e8e4cd0469a47c8`

The exact tracked recipe was copied byte-for-byte into the pinned pmaports
tree and rebuilt from the downloaded archive. The recipe ran all library and
binary unit tests in the Cargo workspace: the app binary had no unit tests and
all six Aperture tests passed. These include focused/failed result parsing,
ambiguous-result rejection, still-mode selection and existing caps helpers.

An earlier `--workspace` check also passed all six unit tests, then exposed a
`crossdirect` rustdoc limitation: the AArch64 doctest process could not resolve
its already-built GStreamer/GTK target crates. The final recipe explicitly
uses `--lib --bins`, retaining every unit test without invoking cross-compiled
doctests. Aperture contains no documentation test examples, so no behavioral
test was hidden or skipped by that correction.

Both APK signatures verified against the published development key. The main
package passed the complete manifest, AArch64 ELF, desktop, D-Bus, AppStream,
GSettings, resource-namespace and stale-identifier validator and had no file
ownership overlap with `snapshot-50.0-r3`.

This package checkpoint preceded phone acceptance. The matching PipeWire r7
state transport and Advanced Snapshot r1 subsequently passed together in the
coherent device transaction recorded below.

## OnePlus 6T side-by-side package acceptance

- Date: 2026-08-25
- Device: OnePlus 6T (`oneplus-fajita`), postmarketOS edge
- Main APK SHA-256:
  `f76372802060de0722cddec238da63ec97dfeae7faf6dc29058bd061fed63bad`
- Language APK SHA-256:
  `13c9078e499a22ea292f9024b443dbd37d9c9181fb4cc18dbb810665cfd1cd43`
- Development public-key SHA-256:
  `31d5d6663ebe400a93fd3d5a107da2ea4dd96e8f6835ba1cdfecf89389ec16f6`
- `/etc/apk/world` before:
  `960e4755fdf654d63e069c567063943d5a4609dda53c27289dd920f1bdb8a842`
- `/etc/apk/world` after:
  `e91dd5dc4a85594da5e28d11c014f6fefaf3b16adc6329f7e1000685de84b32e`

The trusted-signature simulation proposed exactly
`advanced-snapshot-0.1.0-r0` and `advanced-snapshot-lang-0.1.0-r0`: two
additions, 2725 KiB installed size, and no upgrade, downgrade or removal. The
simulation changed neither installed-package state nor the world hash. The
real transaction then installed those same two packages.

`/usr/bin/advanced-snapshot` and its focus helper are owned only by
`advanced-snapshot-0.1.0-r0`; `/usr/bin/snapshot` remains owned by
`snapshot-50.0-r3`. PipeWire and WirePlumber stayed active, the Waydroid
Android session stayed stopped, and no stale camera process remained.

The app launched through the existing Phosh graphical user session as
`io.github.lolren.AdvancedSnapshot`, reported version 0.1.0 and loaded its
independent `/usr/share/advanced-snapshot` data. It produced no warning-or-higher
journal entries and stopped cleanly. With the unattended phone screen
suspended, the window correctly stopped its stream; therefore this launch does
not count as visual preview, photo or video acceptance.

The installed `advanced-snapshot-focus-control` helper passed the repository's
automated [all-sensor smoke test](../tests/device/README.md):

```text
main|post_reset_metrics=41|restarts=0|lens_requests=0
secondary|post_reset_metrics=54|restarts=0|lens_requests=0
front|frames=120|focus_status=unsupported
RESULT|pass|rear_stability_seconds=10
```

The result file SHA-256 is
`1b57f72e9bc6aa22c2e746fc3b450eb254217a34c3118495cab0b95798ff89bd`.
A separate main-camera fakesink test applied safe exposure, saturation,
contrast and sharpness offsets through the installed helper, then restored all
four controls to neutral successfully. No image was retained by either test.

The exact committed test script was rerun from its default helper path and
produced the same summary and hash. The pre/post manifests, world files, helper
logs and summaries are retained on the reference phone under
`/home/user/advanced-snapshot-install-0.1.0`.

## OnePlus 6T r7/r1 coherent acceptance

- Date: 2026-08-25
- Installed stack: kernel r8, libcamera/IPA r24,
  `pipewire-spa-libcamera-1.6.8-r7`, Snapshot r3 and Advanced Snapshot r1
- Staging directory:
  `/home/user/camera-focus-state-r7-r1-20260825`
- Pre-transaction `/etc/apk/world` SHA-256:
  `e91dd5dc4a85594da5e28d11c014f6fefaf3b16adc6329f7e1000685de84b32e`
- Post-transaction `/etc/apk/world` SHA-256:
  `d032cb41e42bda904382159b10198e5c2dd9b73cda58d3f0060993756388e276`

All six candidate and rollback APK signatures were verified before the
transaction. Because r0 had originally been installed from local files,
apk-tools 3 retained identity constraints for the app packages. The accepted
simulation therefore supplied the two r1 app APK paths explicitly and used the
isolated candidate repository to resolve PipeWire r7. It proposed exactly
three upgrades and no removal. The real transaction performed those same
three upgrades. The world-file diff changed only the two Advanced Snapshot
identity constraints; PipeWire remained dependency-owned and gained no world
entry.

The installed package owners are r1 for `/usr/bin/advanced-snapshot` and its
focus helper, r7 for the PipeWire libcamera plugin, and the unchanged distro r3
for `/usr/bin/snapshot`. PipeWire, WirePlumber and the camera portal were active
afterward with no failed user units. The packaged app launched through its
desktop D-Bus service, reported version 0.1.0 and the independent datadir, and
stayed alive until the unattended test terminated it cleanly. Its stdout and
stderr were empty; the runtime evidence SHA-256 is
`c72f813b583e15bf70616d0f9369727fec91e95332fad1101a78210abb5129ae`.
This proves package activation, not visual preview or capture acceptance.

The final non-image all-sensor test used a central staged target and returned:

```text
main|serial=59|tap_result=focused|post_reset_metrics=183|restarts=0|lens_requests=0
secondary|serial=63|tap_result=focused|post_reset_metrics=239|restarts=0|lens_requests=0
front|serial=61|frames=120|focus_status=unsupported
RESULT|pass|rear_stability_seconds=60
```

Its summary SHA-256 is
`e5663d4a894169c097396f7f825199d4bcd211efa398fbb2c274e3fd76acb98c`.
Both rear result files contained only `focused`, each with SHA-256
`6c6a45ac86c5a830cda6b4f9552c0d6e782ca4b0ceff9ce261296168bb67699e`.
The fixed-focus front helper rejected the request as unsupported; that log has
SHA-256
`fcad6f02190e3fb1d5af2c671becbdcbdfed290aa54d079462dd182166a8bad4`.
An earlier off-centre tap on a low-detail area truthfully returned `failed`;
moving the test target to the centre returned `focused`. That is expected
optical behavior and demonstrates that acceptance is no longer mistaken for a
focus result.

Rollback was not executed on the live phone. A simulation proposed exactly the
r7/r1-to-r6/r0 three-package downgrade, and a copied apk database model
performed that downgrade with scripts disabled. Supplying the local PipeWire
APK temporarily added an identity constraint; modeled
`apk del pipewire-spa-libcamera` removed only that world entry because Snapshot,
Advanced Snapshot and the Phosh base retain the plugin as a dependency. The
plugin remained installed at r6 and the modeled world file returned exactly to
the pre-update SHA-256 above. See the matching repository's pmaports packaging
guide for the guarded commands.

The Waydroid container service remained continuously active from before this
transaction, while the Android session remained stopped. No camera process,
autofocus test environment or failed user service remained after cleanup.

The acceptance run exposed that restarting PipeWire underneath an active
desktop portal can leave the main portal failed, while leaving the wlroots
backend connected causes a transient backend failure. The committed runner now
stops and restores both portal units around each PipeWire cycle. A final
hardware regression passed both rear correlated focus results, the fixed-focus
front and 10-second rear stability windows. Its summary SHA-256 is
`aa5d5dedf5834e90ac15bd121a3711b4a7c004df0b5f41a59f155e6013fb9260`;
the bounded portal-journal SHA-256 is
`9447840432b47360053b37dd960f988994808428223dcd2a25127773a595b201`.
That journal contains only orderly stop/start events. Both portal units,
PipeWire and WirePlumber ended active with zero failed user units and no stale
test state.

## Pinch-to-zoom source gate

- Date: 2026-08-25
- Source state: local r2 candidate based on `bb1ff4a`
- Build target: postmarketOS edge, AArch64, isolated strict pmbootstrap work
  directory
- Release compilation: passed
- Application tests: 4 passed, 0 failed
- Aperture tests: 6 passed, 0 failed

The candidate adds a two-point `GtkGestureZoom` controller to the preview. It
records the existing slider value when recognition begins, multiplies that
value by the gesture delta, clamps the result to the same 1x–4x adjustment and
writes the result back through the slider. The slider remains the single state
source, so it updates Camerabin and the on-preview one-decimal value chip in the
same callback. Tapping the chip sets the slider to 1x. Claiming the recognized
two-point sequence prevents its release events from being interpreted as a
tap-to-focus request.

The exact source-tree gate was:

```sh
pmbootstrap -w /path/to/isolated-work -p /path/to/pmaports \
  build --arch aarch64 --force --src /path/to/advanced-snapshot \
  advanced-snapshot
```

The `--src` artifact carries a development timestamp suffix and an isolated
throwaway signing key. It is deliberately not an install candidate. The next
gate is a committed Git source pin, production-key signature and complete APK
manifest validation, followed by touch/UI, all-sensor, photo and video checks
on the reference OnePlus 6T.

## OnePlus 6T r4 live-pinch checkpoint

- Date: 2026-08-25
- Live scheduler source commit: `fe2e6b3`
- Pinned packaging commit: `ca705eb`
- Main APK SHA-256:
  `a94494a28128481674e3665d14ef820b145f32431315369d05410cd15b92f6e9`
- Language APK SHA-256:
  `f24eec67dfe9099c294c6a099d65fbcc9e906c6b422d111b3e7dc091c055a75b`
- Installed pair: `advanced-snapshot-0.1.0-r4` and
  `pipewire-spa-libcamera-1.6.8-r7`

r3 proved that capture-phase arbitration delivered two-finger gestures, but a
physical sustained-pinch test exposed sparse preview-crop changes. r4 separates
the immediate UI value from the comparatively expensive camera property write.
It retains only the latest pending value, dispatches no faster than once every
33 ms, and flushes the exact endpoint when a gesture ends or is cancelled and
before capture. This keeps work bounded instead of queuing stale intermediate
zooms.

The signed r4 packages passed the guarded generation simulation and install.
An automated compositor trace over the live phone preview recorded visible
values of approximately 1.0x, 1.5x, 1.9x and 2.7x during one five-second pinch,
then the exact 3.0x endpoint. The five private trace-frame SHA-256 values are
stored only in the device evidence directory; no scene image is committed.
This proves progressive application state and camera-crop dispatch on hardware.
A user-observed physical smoothness check remains required because compositor
screenshots temporarily pause rendering and cannot establish perceived frame
cadence.
