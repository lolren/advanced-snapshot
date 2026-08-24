# Installation

The source currently builds as an independently named application. A public
postmarketOS package is the supported installation target; direct replacement
of distro Snapshot files is not.

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

## postmarketOS package policy

The APK will be built in a reviewed pmaports overlay, signed with the
VibeMarketOS repository key, installed only after `apk upgrade --simulate`
shows the expected addition, and retained beside its exact previous version.
The installer will run native camera smoke tests before marking the generation
healthy. A failed launch, stream, focus or capture check leaves the prior app
and camera package generation available for rollback.

The recipe and release installer are still in development. Until they pass on
the reference phone, continue using the known-good `snapshot-50.0-r3` package.
