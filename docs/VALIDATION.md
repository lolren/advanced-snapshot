# Validation record

## 0.1.0-r16 OnePlus 6T manual-focus checkpoint

- Date: 2026-08-29
- Source commit: `51139b2df475fa34a7e798452fcda0fac184b3a1`
- Target: postmarketOS edge, AArch64, musl
- Lower stack: kernel r10, libcamera/IPA r28 and PipeWire libcamera SPA r7
- Main APK: `advanced-snapshot-0.1.0_p20260829215222-r16.apk`
  (`46cc19ac583d3ba84fcd400b3e1be4506f583eee404cce11dc8312acea85408d`)
- Language APK:
  `advanced-snapshot-lang-0.1.0_p20260829215222-r16.apk`
  (`3da06127a14216a2463b4454ade32c5d239f03c53cd4d501ac0713e3a1084f9e`)

The exact AArch64 pair was built in the isolated pmbootstrap buildroot and
installed on the reference phone without reboot. `cargo fmt --all -- --check`,
the C helper syntax check, shell syntax checks and the package build passed.

The native all-sensor regression returned:

```text
main|serial=103|tap_result=focused|post_reset_metrics=183|restarts=0|lens_requests=0
secondary|serial=105|tap_result=focused|post_reset_metrics=239|restarts=0|lens_requests=0
front|serial=101|frames=120|focus_status=unsupported
RESULT|pass|rear_stability_seconds=60
```

The serials are ephemeral runtime evidence and must not be copied into a
recipe or script. The two rear sensors also accepted direct manual positions
0.0, 1.0 and 2.0; the matching Waydroid Camera2 probe observed result ranges
`[0.000,2.000]` with delta `2.000` for both rear IDs and correctly marked the
front fixed-focus camera unsupported. Its tap-focus profile reported terminal
states `[3,4]` and non-empty AF regions for both rear IDs.

This checkpoint proves control transport, actuator movement and lifecycle
stability. It does not prove factory dioptre calibration, Android-vendor colour
tuning, saved-photo parity or touchscreen acceptance; those require a
well-lit, repeatable scene and remain separate gates. The installed production
tuning is intentionally conservative: gamma 2.0/2.1/2.2, contrast 1.10 and
saturation 1.35 for IMX371/IMX376/IMX519, with identity CCMs because no colour
chart or flat-field calibration was available.

## 0.1.0 capture-output and zoom-safety checkpoint

- Date: 2026-08-26
- Source changes: still completion now validates a local, non-empty regular
  file before emitting `picture-done`; Camerabin zoom limits are normalized so
  a sub-1, non-finite or otherwise unusable `max-zoom` cannot make the clamp
  panic.
- Host verification: the pinned GTK build environment passed `cargo fmt
  --all -- --check` and all 6 application plus 9 Aperture unit tests.
- Device status: not installed or visually accepted; the reference phone's
  userspace transport is currently unavailable.

The still-output check closes a correctness hole in the inherited
`image-done` path: a missing or empty file is now reported as a failed capture
and cannot be added to the gallery. The zoom test covers zero, NaN and finite
camera limits while preserving the existing 1x–4x UI contract.

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

## Preview-mode selection checkpoint

- Date: 2026-08-25
- Change: preview selection prefers a supported mode no taller than 720 pixels
- Still capture: independently remains bounded at the largest supported 4:3
  mode up to 2048x1536

This separation is intentional for phones using libcamera's software ISP:
the viewfinder and GTK compositor process frames continuously, while a still
capture can use a larger sensor mode only when the shutter is pressed. If a
camera advertises no suitable 640x480–1280x720 preview mode, the selector
uses the previous ratio-checked mode up to 1920x1080 rather than failing camera
startup or selecting an odd-sized mode just above the 720p cap.
The unit test covers a camera advertising both 1920x1080 and 1280x720 and
requires the latter for the live viewfinder.

The corrected source was cross-built for AArch64 on 2026-08-25 from commit
`5efd982`. The isolated package build passed all four Advanced Snapshot tests
and all six Aperture tests, then produced:

```text
advanced-snapshot-0.1.0_p20260825194238-r1.apk
c2c6f195185528b51dcbb759f40daa46e7d3136cefba6d5b8fd76329f2aca6da
advanced-snapshot-lang-0.1.0_p20260825194238-r1.apk
bddcb910c716adb93c62afeaad067dbc641ae77b563dcb7862131eaee50daa9b
```

The APKs are build artifacts, not yet installed after the phone's Waydroid
overlay I/O stall; install them only as a matched package generation.

## Latest-frame preview source checkpoint

- Date: 2026-08-25
- Source commit: `fed2784`
- Change: every `PipelineTee` branch now uses a queue with one buffer,
  unlimited byte/time limits and downstream leakage. A slow compositor or
  software conversion stage therefore discards old viewfinder frames instead
  of presenting a growing preview delay.

The source diff and documentation checks pass, and the repository remains
clean after the commit. No AArch64 package from this revision is claimed as
installed: the current pmbootstrap checkout does not provide a matching
OnePlus 6T device buildroot for a fresh package. Physical acceptance still
requires a matching package, launch, live-preview latency observation,
saved-photo, video and rollback checks on the phone.

## Asynchronous live-sink preview candidate

- Date: 2026-08-26
- Change: the live `gtk4paintablesink` is configured with `sync=false` and
  `qos=true`; capture branches are unchanged

The viewfinder already keeps one downstream-leaky buffer per branch. Disabling
clock synchronisation prevents the display sink from waiting for a timestamp
that is already late after software-ISP or compositor work, while QoS exposes
downstream pressure to upstream GStreamer elements. This is deliberately a
preview-latency candidate, not a claim about still-image quality or video
encoding. The source passed `git diff --check`; it still requires a matching
AArch64 package and visual frame-rate/lifecycle acceptance on the recovered
OnePlus 6T before it becomes a runtime baseline.

## Clean native source-validation checkpoint

- Date: 2026-08-25
- Source commit: `7b1e778`
- Environment: disposable Debian sid container, Rust/Cargo 1.95 and the
  current GTK4, libadwaita, Glycin, GStreamer and PipeWire development files

The complete native workspace test command passed after supplying the ignored
Meson-generated `src/config.rs` fallback used for direct Cargo invocations:

```text
cargo test --workspace --all-targets
4 Advanced Snapshot tests passed
6 Aperture tests passed
```

This validates the application and library source on x86-64. It does not
replace AArch64 packaging or the still-open phone-side preview-latency,
photo, video and visual acceptance gates.

## GTK CI-container test checkpoint

- Date: 2026-08-25
- Source commit: `d66f853`
- Container: `ghcr.io/gtk-rs/gtk4-rs/gtk4:latest`, resolved as
  `sha256:89b799ae74f933b3e3ecc50086743c4ef0c684fa7ab6f091de72ddc65df8fd5e`
- Toolchain: Rust/Cargo 1.97.1
- Installed development stack: GTK 4.22.4, libadwaita 1.9.3, Glycin 2.1.5
  and GStreamer 1.28.6

The complete locked workspace command passed in the container, using a
temporary target directory outside the checkout:

```text
CARGO_TARGET_DIR=/tmp/advanced-cargo-target \
  cargo test --locked --all-targets --all-features --workspace
4 Advanced Snapshot tests passed
6 Aperture tests passed
```

This removes the host-only dependency limitation from the previous check. It
still validates source behavior and compilation on x86-64, not the AArch64
package or physical camera preview, photo, video and lifecycle behavior.

## Strict preview-cap negotiation checkpoint

- Date: 2026-08-26
- Source commit: `3ac146f`
- Change: concrete live camera modes are restricted to the selected
  720p-class mode; range-only advertisements retain the original fallback

The earlier selector placed the preferred mode first and then appended every
advertised mode. That was only a negotiation preference: a source or
downstream element could still select a full-resolution preview and overload a
software ISP. The new path returns the selected fixed mode when it can prove
that the source advertises concrete width, height and framerate combinations.
It preserves the original caps when no concrete selection can be established,
so generic cameras with ranges do not lose their negotiation fallback.

The pinned GTK CI container passed formatting, Meson configuration and the
locked full workspace suite:

```text
cargo fmt --all -- --check
CARGO_TARGET_DIR=/tmp/advanced-cargo-target \
  cargo test --locked --all-targets --all-features --workspace
4 Advanced Snapshot tests passed
8 Aperture tests passed
```

This is a source-level performance fix. A matching AArch64 package still needs
to be built and installed, followed by physical preview-frame-rate, photo,
video and rollback checks on the recovered OnePlus 6T.

## Strict preview-cap AArch64 package checkpoint

- Date: 2026-08-26
- Source commit: `3ac146f3768ec30fbf82f1a172725fefda5da733`
- Target: postmarketOS edge, aarch64, musl
- Source archive SHA-512:
  `c1bc644cb84cd100162ba404248526a6ec6b7f59acb61e24f0c9966f36673aab02eabe3d19c97814acb87efac550f5bfb56b32ef22a7d0986ddb8604e71ba866`
- Build tool: pmbootstrap 3.11.1 with the pinned GTK/GStreamer aarch64
  buildroot

The strict recipe build completed its `check()` phase with four Advanced
Snapshot tests and eight Aperture tests passing. It then produced the matched
release pair:

```text
advanced-snapshot-0.1.0-r6.apk
advanced-snapshot-lang-0.1.0-r6.apk
```

The isolated build initially used pmbootstrap's temporary development key.
For the validator fixture, the signature/control gzip member was separated
from the unchanged data member and the copies were re-signed with the
persistent development key `pmos@local-6a8b0868.rsa`; no payload or package
metadata was changed. The complete repository validator then passed both
signatures, the AArch64 ELF checks, the exact manifest, desktop/D-Bus,
AppStream, GSettings, resource namespace, stale-identifier and Snapshot file
ownership checks.

The validated local fixture hashes are:

- main APK SHA-256:
  `d11286090ce5354a60303b701d45f7fd958b011292831a830a99ecb689cb3ad9`;
- language APK SHA-256:
  `f1596dd13c12c5daebfaa454defe7cda0afc5241195b217cb476791053de606d`;
- public-key SHA-256:
  `31d5d6663ebe400a93fd3d5a107da2ea4dd96e8f6835ba1cdfecf89389ec16f6`.

These are local signed-artifact hashes; package signatures and build
timestamps are expected to vary. The source commit, source SHA-512, recipe
and validator command are the reproducibility anchors. The pair is not yet
installed on the reference phone because its userspace remains wedged in the
previously documented recovery state. Physical preview-frame-rate, photo,
video, display and rollback acceptance remain open.

## Video finalization guard checkpoint

- Date: 2026-08-26
- Source commit: `7177e8683c51d8b7caeb83dc47d262da1242f9cc`
- Change: serialize the stop transition through `video-done` and reject invalid
  recording outputs before gallery insertion

The shutter remains disabled after the user requests stop until camerabin has
finished finalization and emitted `video-done`. This prevents a second shutter
press from racing the muxer's EOS/finalization path. The completion handler
re-enables the shutter for both successful and failed recordings, adds only a
regular non-empty file to the gallery, and shows a failure toast otherwise.

The pinned GTK CI container passed formatting and the locked full workspace
suite after this change:

```text
cargo fmt --all -- --check
CARGO_TARGET_DIR=/tmp/advanced-cargo-target \
  cargo test --locked --all-targets --all-features --workspace
4 Advanced Snapshot tests passed
8 Aperture tests passed
```

This closes the source-level duplicate-stop and empty-output hazards. A
physical test must still confirm a playable file, monotonic duration, stable
preview during recording, clean stop and recovery from an encoder failure on
the OnePlus 6T.

## Video page-lifecycle checkpoint

- Date: 2026-08-26
- Source commit: `abb390ae3b9b95b2f39b88c9d40a4f13f6df2703`
- Change: defer camera-stream shutdown when the camera page hides during a
  recording, and defer stream restart when the page returns before
  `video-done`

The gallery/navigation lifecycle now uses the same completion boundary as the
shutter. Hiding the camera page requests recording stop but leaves camerabin
running through muxer finalization. If the user returns early, stream startup
waits for `video-done`; if the page remains hidden, the stream is stopped only
after the completion signal. This avoids interrupting a recording or reopening
the camera while its stop transition is still in progress.

The pinned GTK CI container's formatting and locked full workspace test suite
passed after this change. Physical navigation, playable-file and encoder-error
acceptance remain pending on the recovered OnePlus 6T.

## Sensor-aware startup-default checkpoint

- Date: 2026-08-26
- Change: apply the sensor-specific colour and contrast defaults when Aperture
  selects the first camera during startup, as well as when the user changes
  cameras later

Aperture can finish selecting its initial camera before the application-level
camera selector runs. The camera-property notification now applies the same
IMX371, IMX376 and IMX519 defaults used for manual camera changes. The default
selection is kept in a pure helper so it can be tested independently; unknown
camera names use the conservative colour-sensor fallback.

The unit tests cover all three OnePlus 6T sensors and the fallback. The pinned
GTK container's formatting and locked full workspace test suite must pass before
this change is released. Saved-photo colour-chart and physical preview
acceptance remain device-gated because the reference phone is currently not
reachable through a usable control interface.

## PostmarketOS recipe-sync checkpoint

- Date: 2026-08-26
- Recipe revision: `advanced-snapshot-0.1.0-r7`
- Source commit: `0df3acc7626a5d5db195c58536ab649e16b83cd3`
- GitHub source archive SHA-512:
  `194a5e16bf66852edcc34de31d9c94d01eeb191f453e8576edfcc10525a34ab904a61e5b637072f2f5d1f25326e72c16db0305e187309b2ae1072b6ade37a9c3`

The postmarketOS APKBUILD now consumes the exact pushed source revision that
VibeMarketOS pins. The package release was incremented from r6 to r7 and the
archive checksum was obtained from the immutable GitHub commit archive. A clean
AArch64 pmbootstrap build and artifact validation are still required before
this recipe is installed on the phone.

## Advanced Snapshot r10 adjustment-serialization package checkpoint

- Date: 2026-08-26
- Source commit: `2a9763b8f42c1bb755a507de1cc49ed3c8f09a77`
- Recipe revision: `advanced-snapshot-0.1.0-r10`
- Source archive SHA-512:
  `ebb1e7818dd9777a5b794ba0667cf449957949a0f3d6e4cb014f12f85538b2d9b9dfad9fe5ec700e2c3accbd6e555cfc457f7cde78c22a03ef93b060bfc1a5b5`

The image-adjustment helper is now serialized and cancellable. A newer slider
value, camera switch, page teardown or stream stop invalidates the previous
generation, and stale completion callbacks are ignored. This prevents rapid
slider movement from applying old values after a newer request or keeping a
dead helper attached to the camera page.

The clean postmarketOS edge AArch64/musl build completed its package test phase
with all 15 cross-compiled tests passing (6 application and 9 Aperture) and
produced the matched release pair. Both APKs passed the independent signature,
architecture and package-content validator:

```text
advanced-snapshot-0.1.0-r10.apk: f832c5b3ae4e96969fccba8c8f563e7ff8a7372e3fef7d9b32dc7d5fb9828eb9
advanced-snapshot-lang-0.1.0-r10.apk: 2756823e3cb3ad68575bbe96d88a20cc99ecdc7440c405ba143baf43fdf99fb9
public-key: 31d5d6663ebe400a93fd3d5a107da2ea4dd96e8f6835ba1cdfecf89389ec16f6
```

The r10 pair is source/package validated but not installed or hardware-
accepted. The reference OnePlus 6T still exposes CDC-NCM without an SSH
banner, so preview, saved-photo, video, display and rollback acceptance remain
open.

## Advanced Snapshot r11 bounded hardware-flash package checkpoint

- Date: 2026-08-26
- Source commit: `0512a75b1419db5621e4e65c7c4ea5b3446aeeac`
- Recipe revision: `advanced-snapshot-0.1.0-r11`
- Source archive SHA-512:
  `84b4849ebd8b46e8473a1cea2c8197cb54a9fed54435cac44528c4575b285b3c1f8341b52e9639d7da1eb5b928eee1568efb525a48ec36462feab96e4e79bb37`

The application now exposes an opt-in rear **Hardware flash** switch. It
launches `pmos-camera-flash` only for a rear camera, uses a 2.5-second level-32
pulse, and interrupts the helper through its restoration path on capture error,
camera switch or stream teardown. The switch remains unavailable when the
helper is absent, and HDR, manual ISO/shutter and automatic flash metering are
still intentionally not advertised.

The pinned GTK build passed Meson compilation (including the focus helper's
explicit libm link), formatting, all 6 application tests, all 9 Aperture tests,
desktop/schema/AppStream validation and the Meson cargo test. The pMOS helper's
fixture suite passed normal pulse, interruption restoration, off and
no-hardware cases. A clean postmarketOS edge AArch64/musl package build then
passed all 15 cross-compiled tests and produced the signed pair below. The
independent package validator accepted both APKs for signatures, AArch64 ELF,
content, schema, AppStream and overlap:

```text
advanced-snapshot-0.1.0-r11.apk: f4dafe29a4682df10b4649fee3110dac419c8179098e0a9762f48a2251cf7c1b
advanced-snapshot-lang-0.1.0-r11.apk: 40a9a822421d5640ce14f1046006bbb5b92b022862d977de0d7d14cf30f2c95a
public-key: 31d5d6663ebe400a93fd3d5a107da2ea4dd96e8f6835ba1cdfecf89389ec16f6
```

The signed pair is source/package validated but not installed or hardware-
accepted. The reference phone still exposes CDC-NCM without an SSH banner, so
live LED, preview, capture and rollback acceptance remain open.

## Advanced Snapshot r12 manual-exposure source checkpoint

- Date: 2026-08-26
- Source commit: `0a14b55983493ba04bfb2a046df8b167158af53c`
- Recipe revision: `advanced-snapshot-0.1.0-r12`
- GitHub source archive SHA-512:
  `e53da5e0975bd1cc57f47c560ea6509eee3c813ee832695d6fb0574368a4dc801cd4c15a9fbcc11c78c703103a2cca39e48c7b73baf7374a2a3692438acdbf2d`

The application now exposes an automatic-exposure switch, manual shutter time
and linear analogue gain in Image Controls. Slider changes are debounced and
sent through dynamically discovered PipeWire controls; camera changes and
stream teardown cancel stale helper processes. Automatic mode remains the
default. The UI and helper are source-validated, but the controls require the
matching libcamera r26 simple-IPA candidate to exist at runtime.

`cargo fmt --all -- --check`, the C helper's strict syntax check and Cargo
metadata validation pass. The full host Cargo build is currently unavailable
because this workstation lacks the GTK/GLib/libadwaita development `.pc`
files; the pinned GTK CI container and clean AArch64 package build remain the
authoritative compile gates. No r12 APK has been built, installed or
hardware-accepted, and the reference phone still has no usable SSH banner.

## Advanced Snapshot r13 compile-fixed AArch64 package checkpoint

- Date: 2026-08-26
- Source commit: `eef98bbb16a5af6cdb21150811a4ea33d6543daf`
- Recipe revision: `advanced-snapshot-0.1.0-r13`
- GitHub source archive SHA-512:
  `66a74459d9277e3cf9759c94a7e76c31f9466977b84f0ed20b949da5bc7888963c9149507646ed827a0830a062da6998f0a3aff27de5c9c626c6720ecfaca17f`

The release fixes the initial exposure-control sensitivity callback so the
full application compiles in the pMOS AArch64 environment. The build used
pmbootstrap 3.11.1, the pinned pmaports base and the package recipe's
immutable source archive. The cross-compiled Cargo check passed all 6
application tests and 9 Aperture tests.

The independent validator accepted signatures, AArch64 ELF files, expected
file ownership, resource namespace, desktop/D-Bus metadata, AppStream data,
GSettings schema, language split and non-overlap with distro Snapshot:

```text
advanced-snapshot-0.1.0-r13.apk: 0c12ce8685afcadd1794e4a530f231d461647e41066965b307b2a43d5f121c81
advanced-snapshot-lang-0.1.0-r13.apk: a03b0a561e4355a4da506e29f0d8b7f16173da694155391e465a3dbfeaab1bd3
public-key: 31d5d6663ebe400a93fd3d5a107da2ea4dd96e8f6835ba1cdfecf89389ec16f6
```

This is source/package validation only. The pair has not been installed or
hardware-accepted; the matching libcamera r26 candidate, phone camera tests,
preview latency, saved-photo/video checks and rollback test remain open while
the reference phone exposes no usable SSH banner. Direct host `cargo test` is
also blocked by the workstation's rustc 1.91.1 versus the locked dependency
minimum of rustc 1.92; the pMOS AArch64 package test phase is the authoritative
compile and test result for this recipe.

## Advanced Snapshot Software HDR source checkpoint

- Date: 2026-08-26
- Source commit: `af69a7151b8fcba1d0650fd911f42e340279e8d0`
- Recipe revision: `advanced-snapshot-0.1.0-r14`
- GitHub source archive SHA-512:
  `e5973d2b5e72d154e6243ded37d26b88328c50be66f5e83b7038794f41df6e0c6cfe4d6e890485b2cd2e6c83d1ecffe1343b09bcd9f815087c5fd99799d64e0a`

This checkpoint adds an opt-in Software HDR path to Advanced Snapshot. The
application captures dark, middle and bright JPEGs sequentially, then invokes
the separately installed `advanced-snapshot-hdr` helper. The helper decodes
the three images, merges non-clipped samples in linear light with a bounded
global tone map, writes through a temporary file and atomically renames the
result. All intermediate files are removed on success, capture failure,
helper failure or stream teardown. Manual exposure and hardware flash are
disabled for the sequence; moving subjects can ghost because no frame
alignment is claimed.

The pinned GTK build passed Meson compilation for both the application and
helper, `cargo fmt --all -- --check`, 8 application tests, 5 HDR helper tests,
9 Aperture tests and clippy with `--deny warnings`. A staged install verified
the app, focus helper, HDR helper, schema and independent resources. The clean
signed AArch64 r14 pair passed the package validator:

```text
advanced-snapshot-0.1.0-r14.apk: 0df78733ec2fc3469dd11a4be274a0fb1bbbb9921dbf18601f99e6b0fa58b0ec
advanced-snapshot-lang-0.1.0-r14.apk: 25d01d10d69099c6c6d837a0cdd30c8724b3e831bf8fbbdf0730e36d75b4d98f
```

The pair is included in the opt-in `camera-r26-r14` generation. Phone
installation and hardware image-quality acceptance remain open; do not
replace the retained r13/r11 artifact until those gates pass.

## Advanced Snapshot r15 handheld-HDR-alignment package checkpoint

- Date: 2026-08-28
- Source commit: `6813a64b499177d3d0ef5272b019c6da53400fba`
- Recipe revision: `advanced-snapshot-0.1.0-r15`
- Reviewed pmaports base: `875bddba6538818f2c3c9849e184f40688ad5140`
- GitHub source archive SHA-512:
  `49252237523317fdd3e27aa4edb60c6ed932a0108757a32208f385d6698c07401bfd9f172045c57ad2af6b7109003dc061527e428d5663931b8685ec3a2771ad`

The HDR helper now estimates bounded global translation for the dark and
bright brackets against the middle exposure. It compares exposure-resistant
log-luminance gradients on a bounded thumbnail, refines the result against
sparse full-resolution samples and accepts at most 96 pixels of translation.
An ambiguous match or one that improves the unshifted score by less than six
percent safely remains unshifted. The merge omits translated samples outside
the image while retaining the middle exposure as a deterministic fallback.
This compensates small whole-frame handheld motion; it does not claim local
motion, rotation, scale, parallax, moving-subject or vendor-ISP correction.

The native pMOS/Alpine environment passed the locked full workspace suite (8
application, 10 HDR/helper and 9 Aperture tests), including an end-to-end merge
that reproduces the stationary result after independently translating both
outer brackets. The same source passed `cargo fmt`, strict Clippy with
`-D warnings`, all five Meson release gates and a staged-install manifest
check. The clean AArch64/musl package build repeated all 27 tests under QEMU.

The independently validated signed artifacts are:

```text
advanced-snapshot-0.1.0-r15.apk: 16581bcf5c96aa74c522c4f51bbd5cb03711a3e41abd02f00a6d9eec7cf61705
advanced-snapshot-lang-0.1.0-r15.apk: dec0ec0c229848a0e157e2eba49ab9e74d30423e69ae76bbd73773eea97b61d2
public-key: 31d5d6663ebe400a93fd3d5a107da2ea4dd96e8f6835ba1cdfecf89389ec16f6
```

The validator accepted signatures, exact package versions, AArch64 binaries,
the expected file manifest, independent resource identifiers, desktop/D-Bus/
AppStream/schema metadata, language splitting and zero file ownership overlap
with GNOME Snapshot. r15 is source- and package-validated but remains an
opt-in candidate: it has not been installed or image-quality accepted on the
reference phone while that phone exposes no usable SSH session. Keep r14 and
the retained device-accepted application package available for rollback.

## Advanced Snapshot r16 lifecycle package checkpoint

- Date: 2026-08-29
- Source commit: `2c93c2fd094ad3011b9466ab5fc0779fda566cce`
- Recipe revision: `advanced-snapshot-0.1.0-r16`
- GitHub source archive SHA-512:
  `6d6086b5709cf4dc7df5c7ceeaa0bd09b76dfc4c91c0091d624c74519acce92dd83159dd3578fbf454294f62a229d151383994dec6fed45127b4c28d0c9a2145`

The r16 source includes the serialized viewfinder lifecycle, the camerabin
NULL teardown barrier and the GStreamer Rust state-tuple compatibility fix.
The clean postmarketOS edge AArch64 build completed in an offline pmbootstrap
work directory. The package pair was installed on the connected OnePlus 6T
without reboot; it is a local development build signed by the pmbootstrap
workstation key, not a public release signed by the repository key.

```text
advanced-snapshot-0.1.0-r16.apk: 4e2926bdaf40fc7f600095b52591c0be08edddbdb9a86527533d11e2f2a45904
advanced-snapshot-lang-0.1.0-r16.apk: d6153444970592d041c6ee6d81048dc84d6d323906a1facc879d0603ccc4c1b0
pmbootstrap signing key: pmos@local-6a92d930
```

The package contains the independent Advanced Snapshot binary, focus helper,
HDR helper and resource namespace. Touchscreen preview latency, saved-photo,
video and image-quality acceptance remain open; keep the distro Snapshot
package available as the rollback path.
