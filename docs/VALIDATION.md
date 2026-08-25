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
