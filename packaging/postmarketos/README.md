# postmarketOS packaging

This aport builds Advanced Snapshot as a separate package; it never replaces
the distro `snapshot` package. The source is pinned to commit
`a73a3993263adb33604a1fad7f88d5e53e75f4c0`, and Cargo dependencies are
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
  ~/.local/var/pmbootstrap/packages/edge/aarch64/advanced-snapshot-0.1.0-r0.apk \
  ~/.local/var/pmbootstrap/packages/edge/aarch64/snapshot-50.0-r3.apk
```

The validator checks the architecture, complete file manifest, desktop entry,
D-Bus service, AppStream metadata, GSettings schema, resource namespace, stale
upstream identifiers, package signature and file ownership overlap. A host
`apk` command can be used without setting `APK_VERIFY_TOOL`.

## Updating the source pin

1. Commit and push the source that will be packaged.
2. Replace `_commit` in `APKBUILD` with the full commit ID.
3. Download `https://github.com/lolren/advanced-snapshot/archive/COMMIT.tar.gz`.
4. Replace the source SHA-512 with `sha512sum` output.
5. Build in a clean aarch64 buildroot and run `validate-apk.sh`.
6. Test installation, launch, preview, focus, photo and video on the reference
   phone before changing a known-good manifest or release tag.

Do not use a moving branch archive in a release recipe.
