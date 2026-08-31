# postmarketOS packaging

This aport builds Advanced Snapshot as a separate package; it never replaces
the distro `snapshot` package. The source is pinned to commit
`5e102b7d4b6bf6b4dcfeabe8f9040ffff8cc1ffd`, and Cargo dependencies are
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
	~/.local/var/pmbootstrap/packages/edge/aarch64/advanced-snapshot-0.1.0-r38.apk \
  ~/.local/var/pmbootstrap/packages/edge/aarch64/snapshot-50.0-r3.apk \
	~/.local/var/pmbootstrap/packages/edge/aarch64/advanced-snapshot-lang-0.1.0-r38.apk
```

Package revision r38 contains the manual shutter/analogue-gain UI, bounded
rear-flash work and opt-in Software HDR on top of the serialized image-
adjustment transport. HDR captures three exposure-bracketed JPEGs and merges
them with the installed `advanced-snapshot-hdr` helper. The helper aligns a
confidence-gated global translation against the middle exposure before fusion;
moving subjects, rotation, parallax and vendor-ISP parity remain explicitly
out of scope. Automatic exposure is enabled by default; disabling it submits
standard libcamera controls in microseconds and linear gain units. These are
userspace features and require the matching OnePlus libcamera/IPA r35 and
PipeWire SPA r8 camera stack on the current reference phone. Older r33 stacks
remain useful for historical source/package reproduction, but r35 is the
documented current lower-layer baseline. The helper
is included in the main package and is covered by the staged install check.
The r38 UI keeps the labelled Image Controls entry in a direct toolbar above
the preview and opens the controls as a bounded, scrollable camera-page
overlay drawer. The upper preview remains visible while values change, so
camera tuning is not buried in Preferences or dependent on bottom-sheet
natural-size negotiation. The drawer provides Sensor default, Neutral,
Natural, Vivid and Custom colour-processing presets; presets change only the
live tone controls and preserve exposure, white balance, focus and matrix
settings. Rear sensors are returned to continuous autofocus
after the preview starts; the panel also exposes an explicit Auto button.
Tapping the preview requests one-shot autofocus at that location, while a
still capture with no tap or manual lock requests a fresh centre-weighted
one-shot scan on the high-resolution stream. The manual-focus slider remains
an intentional lock. The fixed-focus front sensor keeps those rear-only
controls disabled. Gamma is exposed as the standard
0.1–10 tone-curve control. The Camera Calibration panel stores a versioned
per-sensor profile in GSettings, keyed by the stable libcamera node identity;
it can restore automatic/manual white balance, bounded red/blue ISP gains,
image controls and optionally a deliberate manual focus
position while keeping continuous autofocus as the default. It can additionally
store a bounded 3×3 colour matrix when the lower stack advertises that standard
control. The zoom-value chip lives in the toolbar instead of over the capture-
mode selector, and the calibration dialog presents one matrix coefficient per
row so it fits a 360-logical-pixel phone. r38 keeps the visible Green-cast
correction action beside the live white-balance controls and aligns it with the
native r35 profiles' stronger row-sum-preserving matrix
`[0.90, 0.10, 0.00; 0.10, 0.80, 0.10; 0.00, 0.10, 0.90]`. Applying it turns
automatic white balance off because the libcamera contract applies a custom
matrix only in manual-WB mode; Reset reverses it. These changes are included
in source commit
`5e102b7d4b6bf6b4dcfeabe8f9040ffff8cc1ffd`. When the preview is handed
off to the standalone full-resolution still stream, r38 reapplies the last tap
focus window, holds the selected manual lens position, or starts a fresh
centre-weighted one-shot autofocus scan on the new stream before releasing a
JPEG. This avoids accepting a stale terminal state from the preview stream and
closes the old preview-to-photo focus gap that could produce a blurred saved
image.

For phone cameras with a concrete raw still mode, r34 also bypasses the legacy
Camerabin source-retarget operation that produced `not-negotiated` failures on
the OnePlus 6T. It stops preview, runs one bounded full-resolution JPEG stream
and restores preview. Generic cameras without such a mode retain the inherited
path. `tests/device/probe-camerabin-capture.py` preserves the three wrapper
strategies used to reproduce the lower-level failure.

The release-signed r38 artifacts produced main APK SHA-256
`91d2c1c65d1eecbf7dca7e9f90eb69a78e60a123f9f66b662c48d5ebd81e27d5` and
language APK SHA-256
`6ad6645feb9861c8d2305b19357b30c40d7572c4f957c0aa4985c92dfb568417`.
They use `pmos@local-6a92d930.rsa.pub`, whose SHA-256 is
`c1f8892b9576ce1807732a985243311d272ab422fc30958a2fb78d5bfc8d36a6`.
The r38 local pmbootstrap build outputs, signed by its development key, were
`2ffac097848b369dfc06a38c158d60200e1518f1ab498ab4f02151603f764410` and
`66f6a43dbbfdb65041b3275b471a68c5a0e2e52411f14c08a496e4712b9bd0c1` before
the release signature was added.
The previous release-signed r37 artifacts produced main APK SHA-256
`8cdd69242116036009c89b51c43a17f99498221b902146a7641d386d067dfc0d` and
language APK SHA-256
`4b6a45b36e7429a4ab453515469e5cb760f167140985da073002f8e196a1b874`.
The unsigned local r37 pmbootstrap build outputs were
`7b6105a956c19d9777c5f70db41ed790d875444823e7b09627e1e4c2b600a130` and
`8ddde05a5b056de7111dc1d3cd726d5ca3970ffafe6827e8626e5c81ba9101a4` before
the release signature was added.
The previous r36 build produced main APK SHA-256
`8a0f08defead7406823b269f92a161963e754770e26e698ed508a1e3c631d37c` and
language APK SHA-256
`32a2893a5e2fa2a68c6a17a2f4581e9a5fa1c78b75cccbb4efd2f04dc5888a5e`.
The previous reference r34 build produced main APK SHA-256
`7f94c88bbc5d7ec300a7f2f1481dff7f882bd43480506fef18f79fdffa390c74` and
language APK SHA-256
`6326708ca21e1dacd4e4264cf48358ccc59d8a99dc2f88be5d36cc46f19ef5de`.
The reference r33 build produced main APK SHA-256
`a16a5e4c0d72316de7d10228f02fd7d966861edd09160ab4cdf85e41fe400c72` and
language APK SHA-256
`47fa8c3d68d6ce638de9abdfb2980d703275fb519f339dd2b3dfbb2177e34545`.
The reference r32 build produced main APK SHA-256
`269f68cb9d2fc7061a7277f21f70c87641d2a20a7206a090bbbbbd279a09ce5b` and
language APK SHA-256
`8bc79a14ed890dd429188cb7b173cc9d13c61572c92faea4b4f24de66501e377`.

`APK_KEY_DIR` and `APK_KEY_FILE` are optional and are useful for validating a
local pmbootstrap build signed by that buildroot's development key. The key
file must be inside the selected key directory because apk-tools scans that
directory. A release artifact must use the repository key
`packaging/keys/pmos@local-6a92d930.rsa.pub` instead. The recipe runs all library and
binary unit tests in the Cargo workspace,
including the Aperture focus-result parser. Cross-compiled Rust doctests are
excluded because `crossdirect` cannot resolve their target crates. The
validator checks both package signatures, architectures, complete file
manifests, language split, desktop entry, D-Bus service, AppStream metadata,
GSettings schema, resource namespace, stale upstream identifiers and file
ownership overlap. It also parses the packaged GtkBuilder XML to require one
toolbar-contained zoom chip and the nine phone-width calibration rows. A host
`apk` command can be used without setting
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
