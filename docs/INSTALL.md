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
any file ownership collision. It also verifies the APK signature against the
development public key in `packaging/keys`; on a pmbootstrap workstation, set
`APK_VERIFY_TOOL="$HOME/.local/var/pmbootstrap/apk.static"`. See
`packaging/postmarketos/README.md` and `docs/VALIDATION.md` for the exact source
pin and reference results.

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

## Development package installation

Verify the public key before trusting it:

```sh
sha256sum packaging/keys/pmos@local-6a8b0868.rsa.pub
# expected: 31d5d6663ebe400a93fd3d5a107da2ea4dd96e8f6835ba1cdfecf89389ec16f6
sudo install -m 0644 packaging/keys/pmos@local-6a8b0868.rsa.pub /etc/apk/keys/
```

Copy the main and language APKs to the phone, preserve `/etc/apk/world`, and
simulate the exact local-file transaction:

```sh
cp /etc/apk/world "$HOME/world.before-advanced-snapshot"
sudo apk add --simulate ./advanced-snapshot-0.1.0-r0.apk \
  ./advanced-snapshot-lang-0.1.0-r0.apk
```

apk-tools 3 prints a realistic “Installing” transaction during simulation;
confirm `apk info -e advanced-snapshot` is still false and the saved world hash
is unchanged. Proceed only when the simulation contains two additions and no
upgrade, downgrade or removal:

```sh
sudo apk add ./advanced-snapshot-0.1.0-r0.apk \
  ./advanced-snapshot-lang-0.1.0-r0.apk
apk info -W /usr/bin/advanced-snapshot /usr/bin/snapshot
```

With both camera applications closed, validate the installed helper without
retaining scene data:

```sh
./tests/device/validate-pipewire-af.sh \
  --output "$HOME/advanced-snapshot-af-smoke" \
  --stability-seconds 60
```

Rollback removes only the independent packages:

```sh
sudo apk del advanced-snapshot advanced-snapshot-lang
```

The development public key may remain for later signed builds. Remove it only
when no installed or archived package still relies on that trust anchor.
