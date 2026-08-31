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
schedule a delayed movement that could blur the subsequent still. The
standalone full-resolution still path snapshots the last tap or manual lens
choice before stopping preview and reapplies it after the new raw stream is
ready. Automatic mode instead waits for the new stream's terminal AF result.
Thus the focus barrier covers the stream that actually supplies the saved
JPEG, not merely the preview that preceded it.

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

## Live camera controls

The labelled **Image Controls** button is part of the camera page's direct
toolbar, not Preferences. On first use, the scrollable control panel is moved
into a bounded overlay drawer at the bottom of the camera view. The upper
viewfinder remains visible while sliders, presets and switches submit their
standard controls, so the user can judge the live result immediately. The
drawer is intentionally limited to 360 logical pixels and scrolls internally
on narrow phones.

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

## White balance

Automatic white balance remains the default. When it is disabled, Aperture
debounces the two gain sliders and starts the control helper with the selected
camera serial, red gain and blue gain. The helper discovers `AwbEnable` and
`ColourGains` dynamically, disables AWB and sends the two-element float array
in one PipeWire property update. Green is fixed at 1.0 by the standard
libcamera contract. Re-enabling automatic mode submits `AwbEnable=true` and
lets the software ISP resume statistics-driven updates.

The subprocess is generation-owned just like exposure and focus helpers: a
new value, camera switch or stream teardown cancels stale work. The PipeWire
SPA patch transports the float array without hard-coding libcamera numeric
IDs, and the simple IPA clamps both channels to its advertised 0–4 range.
Advanced Snapshot intentionally presents 0.1–4.0 so a saved profile cannot
black out one colour channel.

## Colour correction matrix

The optional custom matrix is the standard row-major 3×3
`ColourCorrectionMatrix` that converts white-balanced camera RGB into sRGB.
The calibration dialog bounds every coefficient to -4…4. Aperture launches one
generation-owned helper request containing `AwbEnable=false`, both
`ColourGains` and all nine matrix values, so the ISP never observes an
intermediate frame with automatic white balance and a manual matrix mixed
together. The helper discovers every control ID from the live PipeWire node;
unsupported cameras fail as unavailable instead of receiving a guessed numeric
ID.

Automatic white balance remains the normal default. The custom matrix is only
submitted while AWB is manual, as required by libcamera. Turning custom colour
off sends identity once before returning to ordinary manual white balance;
turning AWB back on lets the sensor tuning choose its temperature-dependent
matrix again. Matrix controls and the two convenience presets are calibration
inputs, not claims that a factory profile has been recovered.

The Image Controls drawer exposes a separate **Green-cast correction → Apply**
action. It uses the conservative OnePlus row-sum-preserving starting matrix,
explicitly disables AWB, and marks the selector Custom so the user can see that
the live pipeline is no longer automatic. **Reset** reverses the action. This
is intentionally the same reproducible starting point as the calibration
dialog, not a hidden per-scene or factory calibration.

## Colour-processing presets

The Image Controls panel offers four tone presets implemented entirely in the
application: Sensor default, Neutral, Natural and Vivid. A preset writes only
the standard `Gamma`, `Saturation`, `Contrast` and `Sharpness` values through
the existing adjustment helper. It leaves exposure, white balance, focus,
zoom and the optional colour matrix alone. If the user changes one of those
four sliders afterward, the selector becomes Custom. This keeps a quick
Android-like starting point available without pretending that a generic look
is a measured sensor calibration.

## Sensor calibration profiles

The Camera Calibration dialog is deliberately a userspace profile layer. It
reads the current values from the Image Controls UI, lets the user compare a
grey card or colour chart in even light, and stores a versioned GLib KeyFile
inside the `camera-calibration-profiles` GSettings key. Each group name is an
FNV-1a hash of the stable camera node name, libcamera path or device name; the
ephemeral PipeWire object serial is never persisted. Selecting a different
physical sensor therefore loads a different profile, while a corrupt profile
falls back to bounded defaults.

Profiles contain only controls the application can submit through the standard
interface: automatic/manual exposure, shutter time, analogue gain,
automatic/manual white balance, red/blue gains, the optional 3×3 colour matrix,
Gamma, Saturation, Contrast, Sharpness and normalized rear `LensPosition`. The
optional manual-focus restore flag is off by default, so a saved profile does
not disable continuous autofocus unexpectedly. Clearing a profile restores the
sensor-aware built-in values. Profile format version 3 adds the matrix switch
and nine coefficients while older profiles load with custom colour disabled
and identity values. The matrix is writable on the matching OnePlus lower
stack, but no factory coefficients, lens-shading table or vendor denoise data
are supplied. The tool therefore remains a repeatable control calibration aid
rather than a proprietary ISP replacement.

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
