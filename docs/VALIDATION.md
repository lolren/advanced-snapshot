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

### Still required before a release tag

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

This is package acceptance, not phone acceptance. The matching PipeWire r7
state transport, r1 helper result, all-three-camera stream and rear autofocus
checks must pass as one rollback-safe device transaction before the installed
r0/r6 baseline is superseded.

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
`snapshot-50.0-r3`. PipeWire and WirePlumber stayed active, Waydroid stayed
stopped, and no stale camera process remained.

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
