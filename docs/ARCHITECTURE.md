# Architecture

Advanced Snapshot remains a thin application over standard Linux camera
layers:

```text
touch and controls
        |
Advanced Snapshot / Aperture
        |
PipeWire camera node + control helper
        |
libcamera simple pipeline and software ISP
        |
V4L2 sensor and actuator drivers
```

The application never writes actuator DAC values directly. A tap is first
mapped out of preview letterboxing, through the effective stream crop and
inverse libcamera orientation, then converted into a sensor-coordinate
`AfWindows` rectangle. The helper discovers PipeWire control IDs dynamically
and submits mode, metering, window and trigger atomically.

The current helper is intentionally a separate C process because Snapshot 50.0
does not expose generic PipeWire camera controls through Aperture. Long term,
the preferred design is a typed Aperture control/status API backed by PipeWire
properties and request metadata. That will allow the reticle to represent
Scanning, Focused and Failed rather than only helper acceptance.

Device tuning, algorithms and kernel changes do not belong in this repository.
They are versioned in `oneplus6t-pmos-fixes`; VibeMarketOS will compose signed
versions of both projects and activate an update only after cross-layer health
checks pass.
