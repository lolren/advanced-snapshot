# OnePlus 6T device checks

`validate-pipewire-af.sh` is a bounded, non-image camera test for the OnePlus 6T
reference stack (libcamera/IPA r28 and PipeWire libcamera SPA r7). Run it as the
graphical login user, never as root:

```sh
./tests/device/validate-pipewire-af.sh \
  --output "$HOME/advanced-snapshot-af-smoke" \
  --stability-seconds 60
```

The script discovers current PipeWire serials instead of hard-coding them. It
opens each sensor into a GStreamer fakesink and requires a generation-correlated
`focused` result from a staged central target on both IMX519 and IMX376 before
checking reset and stable continuous focus. A low-detail tap may truthfully
return `failed` and is not suitable as an acceptance target. The script
requires IMX371 to return the fixed-focus unsupported status after 120 frames.
No photograph or video is saved.

PipeWire and WirePlumber are restarted per sensor so libcamera diagnostics can
be isolated. If the desktop portal was active, it and its wlroots backend are
stopped before each PipeWire cycle and restored afterward so neither can retain
a dead camera connection. The prior service state and any `LIBCAMERA_LOG_*`
user-service environment are restored on every exit. The test refuses to run
while a camera application or another `gst-launch-1.0` process is active unless
`--close-camera-apps` is explicitly supplied.

The output directory must be absent or empty. A passing run creates
`summary.psv`, per-sensor autofocus logs, helper result/error files, stream
logs and the front-helper status. Logs contain sensor names, ephemeral PipeWire
serials, lens positions and focus metrics; review them before sharing.

## Still-capture negotiation probe

`probe-camerabin-capture.py` isolates the legacy GStreamer Camerabin still
path from the GTK application. It discovers the current PipeWire camera rather
than embedding a node serial, uses a fake viewfinder sink and validates every
`image-done` path before reporting success. Stop camera applications first and
run it as the graphical login user:

```sh
./tests/device/probe-camerabin-capture.py \
  --location front \
  --strategy ordered \
  --captures 1 \
  --output /tmp/camerabin-ordered.jpg

./tests/device/probe-camerabin-capture.py \
  --location front \
  --strategy fixed-resolution \
  --captures 5 \
  --output /tmp/camerabin-fixed.jpg
```

The strategies are diagnostic, not interchangeable production settings:

- `ordered` offers the 1280x720 preview mode before the 2048x1536 still mode;
- `full-resolution-source` keeps the source at 2048x1536 and scales only the
  viewfinder request; and
- `fixed-resolution` keeps both source and viewfinder at 2048x1536.

On the affected OnePlus 6T PipeWire/libcamera stack, the wrapper can collapse
its image pad to empty caps during preview-to-still retargeting, or fail while
resetting preview caps after several otherwise valid captures. A non-zero probe
exit therefore records the wrapper regression; it does not by itself indict
the sensor or cable. Advanced Snapshot's phone still path avoids that retarget:
it releases the low-power preview, opens one fixed full-resolution raw stream,
drops a bounded one-second warm-up, encodes one JPEG and restores preview.
Generic cameras without a concrete raw still mode retain the inherited
Camerabin path.
