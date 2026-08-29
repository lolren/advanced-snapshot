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

Rear manual focus follows the same boundary. The UI exposes a normalized
`LensPosition` value from 0 to 2, the helper discovers the control ID at
runtime, and the simple IPA maps that public range onto the measured safe
400–800 actuator span. The IPA publishes the normalized position in result
metadata so a caller can verify that the request reached the lens. This range
is intentionally documented as a device tuning contract, not a factory
calibrated object-distance measurement. Manual mode cancels a running scan and
holds the lens; Reset submits the existing scan-free continuous-focus reset.
The fixed-focus front camera exposes neither the control nor the UI affordance.

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
A tap leaves one-shot autofocus locked at the chosen position; it does not
schedule a delayed movement that could blur the subsequent still.

## Software HDR

When the opt-in HDR setting is enabled and automatic exposure is active, the
GTK process reserves three hidden JPEG paths and requests them sequentially at
the current EV preference plus `-1`, `0` and `+1` stops (clamped to the
supported `-1..+1` control range). Controls and the shutter are disabled during
the sequence. The completed paths are passed to the separate
`advanced-snapshot-hdr` helper so JPEG decoding and the bounded merge do not
block the GTK main loop. The helper linearizes sRGB samples, weights
well-exposed/non-clipped samples, applies a global Reinhard tone map and writes
the result to a temporary file before an atomic rename. Before fusion, it
registers the dark and bright brackets against the middle frame. The matcher
uses log-luminance gradients, a bounded 512-pixel thumbnail search and sparse
full-resolution refinement. It accepts at most 96 pixels of whole-frame
translation and returns zero shift when the best score does not improve the
unshifted score by at least six percent. Samples that move outside the source
frame are omitted; the middle exposure remains the deterministic fallback.
The app restores the user's normal image controls and deletes all intermediate
files on success, capture failure, helper failure or stream teardown.

This is an open exposure-fusion baseline, not a proprietary Android ISP:
global camera translation is aligned, but independently moving subjects,
rotation, scale change, parallax and non-rigid motion can still ghost. Output
metadata/EXIF is not preserved by the GdkPixbuf JPEG re-encode. The explicit
limits and honest UI text keep those constraints visible until richer motion
and tone-mapping implementations are independently validated.

## Manual exposure

The Image Controls sheet keeps automatic exposure enabled by default. When the
user disables it, Aperture debounces changes to the shutter-time and analogue-
gain sliders and invokes the same helper with the selected camera serial. The
helper discovers `ExposureTime`, `ExposureTimeMode`, `AnalogueGain` and
`AnalogueGainMode` IDs at runtime, then submits both manual values in one
PipeWire property update. The simple IPA converts microseconds and linear gain
to the sensor's V4L2 units, clamps them to the advertised limits and reports
the applied modes in frame metadata. Re-enabling automatic exposure submits
both automatic modes together. This keeps the app independent of libcamera
numeric IDs and makes unsupported cameras fail as an unavailable control,
rather than silently changing a different property.

## Bounded rear flash

The optional **Hardware flash** switch does not write LED sysfs files from the
GTK process. On a rear-camera still request, Advanced Snapshot launches the
`pmos-camera-flash --pulse` helper, waits briefly for the helper to arm the
channels, and then submits the still request. The helper discovers top-level
`*:flash` LED directories, records their current brightness, applies a capped
pulse and restores the saved values on normal completion or `SIGINT`. A
generation counter prevents an old helper completion from clearing a newer
capture's process handle; camera changes, stream stop and capture errors
interrupt the helper through its restoration path.

This is an explicit illumination primitive, not automatic flash metering. The
helper is kept in `oneplus6t-pmos-fixes` so the sysfs policy, fixture tests and
postmarketOS installation can be reviewed and updated independently of the
camera UI. If the helper or writable LED channels are absent, the switch is
disabled and no capture behavior changes.

Long term, the preferred design is a typed Aperture control/status API backed
directly by PipeWire properties and request metadata. The helper keeps this
first implementation reviewable and independently rollback-safe.

Device tuning, algorithms and kernel changes do not belong in this repository.
They are versioned in `oneplus6t-pmos-fixes`; VibeMarketOS will compose signed
versions of both projects and activate an update only after cross-layer health
checks pass.
