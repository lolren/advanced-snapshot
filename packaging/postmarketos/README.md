# postmarketOS packaging

This aport builds Advanced Snapshot as a separate package; it never replaces
the distro `snapshot` package. The source is pinned to commit
`eef98bbb16a5af6cdb21150811a4ea33d6543daf`, and Cargo dependencies are
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
  ./packaging/postmarketos/validate-apk.sh \
  ~/.local/var/pmbootstrap/packages/edge/aarch64/advanced-snapshot-0.1.0-r13.apk \
  ~/.local/var/pmbootstrap/packages/edge/aarch64/snapshot-50.0-r3.apk \
  ~/.local/var/pmbootstrap/packages/edge/aarch64/advanced-snapshot-lang-0.1.0-r13.apk
```

Package revision r13 contains the manual shutter/analogue-gain UI and helper
transport on top of the serialized image-adjustment and bounded rear-flash
work. Automatic exposure is enabled by default; disabling it submits standard
libcamera controls in microseconds and linear gain units. This is a userspace
feature and still requires the matching libcamera r26 candidate plus physical
phone acceptance. It also includes the compile fix for the initial exposure
control sensitivity update.

The recipe runs all library and binary unit tests in the Cargo workspace,
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
