# postmarketOS packaging

This aport builds Advanced Snapshot as a separate package; it never replaces
the distro `snapshot` package. The source is pinned to commit
`35aea283224d706b76b32df33d2fdd66407533c0`, and Cargo dependencies are
resolved from `Cargo.lock` into a local vendor tree before compilation.

## Build

From an up-to-date pmaports checkout:

```sh
mkdir -p temp/advanced-snapshot
cp /path/to/advanced-snapshot/packaging/postmarketos/APKBUILD \
  /path/to/advanced-snapshot/packaging/postmarketos/cargo-auditable.patch \
  temp/advanced-snapshot/
pmbootstrap -p "$PWD" build --arch aarch64 advanced-snapshot
```

Run the artifact validator against the resulting package and the distro
Snapshot package when it is available locally:

```sh
APK_VERIFY_TOOL="$HOME/.local/var/pmbootstrap/apk.static" \
  APK_KEY_DIR="$HOME/.local/var/pmbootstrap/config_apk_keys" \
  ./packaging/postmarketos/validate-apk.sh \
  ~/.local/var/pmbootstrap/packages/edge/aarch64/advanced-snapshot-0.1.0-r23.apk \
  ~/.local/var/pmbootstrap/packages/edge/aarch64/snapshot-50.0-r3.apk \
  ~/.local/var/pmbootstrap/packages/edge/aarch64/advanced-snapshot-lang-0.1.0-r23.apk
```

Package revision r23 contains the manual shutter/analogue-gain UI, bounded
rear-flash work and opt-in Software HDR on top of the serialized image-
adjustment transport. HDR captures three exposure-bracketed JPEGs and merges
them with the installed `advanced-snapshot-hdr` helper. The helper aligns a
confidence-gated global translation against the middle exposure before fusion;
moving subjects, rotation, parallax and vendor-ISP parity remain explicitly
out of scope. Automatic exposure is enabled by default; disabling it submits
standard libcamera controls in microseconds and linear gain units. These are
userspace features and still require the matching libcamera r26 candidate plus
physical phone acceptance. The helper is included in the main package and is
covered by the staged install check. The r23 UI also keeps the labelled Image
Controls entry in a direct toolbar above the preview and renders the control
panel in a bounded, scrollable in-layout revealer, so the controls are
visible on the OnePlus 6T's small display without depending on bottom-sheet
natural-size negotiation. Rear sensors are returned to continuous autofocus
after the preview starts; the panel also exposes an explicit Auto button.
Tapping the preview requests one-shot autofocus at that location, while the
manual-focus slider remains an intentional lock. The fixed-focus front sensor
keeps those rear-only controls disabled. Gamma is exposed as the standard
0.1–10 tone-curve control. The Camera Calibration panel stores a versioned
per-sensor profile in GSettings, keyed by the stable libcamera node identity;
it can restore image controls and optionally a deliberate manual focus
position while keeping continuous autofocus as the default. The current
GTK/glib compile fix is included in source commit
`35aea283224d706b76b32df33d2fdd66407533c0`.

`APK_KEY_DIR` and `APK_KEY_FILE` are optional and are useful for validating a
local pmbootstrap build signed by that buildroot's development key. The key
file must be inside the selected key directory because apk-tools scans that
directory. A release build must use the repository key in `packaging/keys`
instead. The recipe runs all library and
binary unit tests in the Cargo workspace,
including the Aperture focus-result parser. Cross-compiled Rust doctests are
excluded because `crossdirect` cannot resolve their target crates. The
validator checks both package signatures, architectures, complete file
manifests, language split, desktop entry, D-Bus service, AppStream metadata,
GSettings schema, resource namespace, stale upstream identifiers and file
ownership overlap. A host `apk` command can be used without setting
`APK_VERIFY_TOOL`.

## Updating the source pin

1. Commit and push the source that will be packaged.
2. Replace `_commit` in `APKBUILD` with the full commit ID.
3. Download `https://github.com/lolren/advanced-snapshot/archive/COMMIT.tar.gz`.
4. Replace the source SHA-512 with `sha512sum` output.
5. Build in a clean aarch64 buildroot and run `validate-apk.sh`.
6. Test installation, launch, preview, focus, photo and video on the reference
   phone before changing a known-good manifest or release tag.

Do not use a moving branch archive in a release recipe.
