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
`/usr/libexec/advanced-snapshot-focus-control`,
`/usr/libexec/advanced-snapshot-hdr`, the independent D-Bus service,
GSettings schema, metainfo, resources and icons under the
`io.github.lolren.AdvancedSnapshot` name. It must not contain or overwrite
`org.gnome.Snapshot` paths.

The optional **Hardware flash** switch is supplied by the separate
`oneplus6t-pmos-fixes` package. On a OnePlus 6T installation, install that
package alongside Advanced Snapshot and verify the helper before opening the
camera:

```sh
pmos-camera-flash --status
```

The status command is read-only. A report with no writable `*:flash` channels
keeps the switch disabled; it does not modify LED state.

On a rear autofocus camera, the still-capture path waits for the published
libcamera autofocus state before starting the still request. This avoids
saving a frame while the lens is between scan positions. The wait is bounded
and best-effort, so fixed-focus cameras and older PipeWire stacks retain the
normal capture path.

The Software HDR switch uses the installed `advanced-snapshot-hdr` helper. It
is off by default and requires automatic exposure. It creates three hidden
temporary JPEGs, merges them, atomically installs the final JPEG and removes
the temporary files. It aligns small whole-frame translations automatically,
but keep the phone and subject still: independently moving subjects, rotation,
parallax and non-rigid motion can still ghost.

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

The release installer is still in development. The r7/r1 package and focus
transport have passed coherent non-image phone acceptance, but until native
visual photo and video acceptance passes on the reference phone, continue
keeping the known-good `snapshot-50.0-r3` package installed beside it.

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

That r0 procedure records local-package identity constraints in
`/etc/apk/world`. Do not later expect a package-name-only `apk upgrade` to
replace them. The accepted OnePlus 6T r6/r0-to-r7/r1 transaction supplies the
two r1 app APK paths explicitly and resolves PipeWire r7 from a separate
offline repository:

```sh
stage=/absolute/path/to/camera-r7-r1
sudo apk add --simulate --upgrade --allow-untrusted --network=no \
  --interactive=no --repository "$stage/candidate" \
  "$stage/candidate/aarch64/advanced-snapshot-0.1.0-r1.apk" \
  "$stage/candidate/noarch/advanced-snapshot-lang-0.1.0-r1.apk"
```

Require exactly the PipeWire r6-to-r7 and both Advanced Snapshot r0-to-r1
upgrades, with no removal, before running the same command without
`--simulate`. The expected reference world-file change is exactly two updated
Advanced Snapshot identity lines. Build the candidate and rollback repositories
and follow the complete service, hash and rollback procedure in the
[OnePlus 6T packaging guide](https://github.com/lolren/oneplus6t-pmos-fixes/blob/main/packaging/pmaports/README.md).

For an initial r0 side-by-side installation, rollback removes only the
independent packages:

```sh
sudo apk del advanced-snapshot advanced-snapshot-lang
```

The development public key may remain for later signed builds. Remove it only
when no installed or archived package still relies on that trust anchor.
