# OnePlus 6T device checks

`validate-pipewire-af.sh` is a bounded, non-image camera test for the r24
OnePlus 6T reference stack. Run it as the graphical login user, never as root:

```sh
./tests/device/validate-pipewire-af.sh \
  --output "$HOME/advanced-snapshot-af-smoke" \
  --stability-seconds 60
```

The script discovers current PipeWire serials instead of hard-coding them. It
opens each sensor into a GStreamer fakesink, checks tap/reset and stable
continuous focus on IMX519 and IMX376, and requires IMX371 to return the
fixed-focus unsupported status after 120 frames. No photograph or video is
saved.

PipeWire and WirePlumber are restarted per sensor so libcamera diagnostics can
be isolated. Their prior active state and any `LIBCAMERA_LOG_*` user-service
environment are restored on every exit. The test refuses to run while a camera
application or another `gst-launch-1.0` process is active unless
`--close-camera-apps` is explicitly supplied.

The output directory must be absent or empty. A passing run creates
`summary.psv`, per-sensor autofocus logs, stream logs and the front-helper
status. Logs contain sensor names, ephemeral PipeWire serials, lens positions
and focus metrics; review them before sharing.
