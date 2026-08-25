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
does not expose generic PipeWire camera controls through Aperture. The patched
PipeWire libcamera SPA publishes three namespaced, read-only node properties:

- `api.libcamera.af-trigger-generation` increments when an `AfTriggerStart`
  control is accepted;
- `api.libcamera.af-state-trigger-generation` identifies which trigger owns
  the reported request metadata; and
- `api.libcamera.af-state` carries `idle`, `scanning`, `focused` or `failed`.

The helper snapshots the generation before submitting a tap and exits only
after a terminal state for the newly accepted generation. Advanced Snapshot
keeps the reticle amber while waiting, changes it to green only for `focused`,
and uses red for `failed` or an infrastructure error. A new tap terminates the
previous helper and generation checks suppress every stale callback. The
fixed-focus front camera publishes no AF state and receives no focus gesture.

Long term, the preferred design is a typed Aperture control/status API backed
directly by PipeWire properties and request metadata. The helper keeps this
first implementation reviewable and independently rollback-safe.

Device tuning, algorithms and kernel changes do not belong in this repository.
They are versioned in `oneplus6t-pmos-fixes`; VibeMarketOS will compose signed
versions of both projects and activate an update only after cross-layer health
checks pass.
