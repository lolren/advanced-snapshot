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
`/usr/libexec/advanced-snapshot-focus-control`, the independent D-Bus service,
GSettings schema, metainfo, resources and icons under the
`io.github.lolren.AdvancedSnapshot` name. It must not contain or overwrite
`org.gnome.Snapshot` paths.

## Build the postmarketOS package

Copy `packaging/postmarketos/APKBUILD` and `cargo-auditable.patch` into
`temp/advanced-snapshot` in a current pmaports checkout, then run:

```sh
pmbootstrap -p /path/to/pmaports build --arch aarch64 advanced-snapshot
```

Before installation, run `packaging/postmarketos/validate-apk.sh` against the
new APK and the locally built distro Snapshot APK. The validator rejects wrong
architectures, unexpected files, invalid metadata, old runtime identifiers and
any file ownership collision. See `packaging/postmarketos/README.md` and
`docs/VALIDATION.md` for the exact source pin and reference results.

## Installation policy

The APK will be built in a reviewed pmaports overlay, signed with the
VibeMarketOS repository key, installed only after `apk upgrade --simulate`
shows the expected addition, and retained beside its exact previous version.
The installer will run native camera smoke tests before marking the generation
healthy. A failed launch, stream, focus or capture check leaves the prior app
and camera package generation available for rollback.

The release installer is still in development. Until side-by-side native photo
and video acceptance passes on the reference phone, continue using the
known-good `snapshot-50.0-r3` package.
