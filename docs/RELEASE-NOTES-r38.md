# Advanced Snapshot r38

This OnePlus 6T application update fixes the preview-to-photo autofocus gap.
When the user has not selected a tap-focus point or manual lens position, a
still capture now performs a fresh centre-weighted one-shot autofocus scan on
the standalone full-resolution still stream. A stale terminal result from the
preview stream can no longer be mistaken for the new still stream's focus
result.

Tap-focus and manual-focus selections remain authoritative. The fixed-focus
front camera is unchanged. This is an application-only update: it does not
change the kernel, firmware, boot slots, partitions, libcamera, PipeWire or
Waydroid.

Source commit: `5e102b7d4b6bf6b4dcfeabe8f9040ffff8cc1ffd`

## Install

Verify the public key and both APK hashes from `SHA256SUMS`, then install the
pair with `apk add`. No reboot is required:

```sh
sudo install -m 0644 pmos@local-6a92d930.rsa.pub /etc/apk/keys/
sudo apk add --upgrade ./advanced-snapshot-0.1.0-r38.apk \
  ./advanced-snapshot-lang-0.1.0-r38.apk
```

The package remains separate from distro GNOME Snapshot. Keep the previous
pair until preview, focus and saved-photo checks pass; removing these two
packages rolls back to the distro camera application.
