# Feature and acceptance matrix

Advanced Snapshot treats Android camera behavior as a set of independently
testable capabilities. “Present in the interface” and “implemented by the
camera stack” must always agree.

| Capability | Application | PipeWire/libcamera requirement | Acceptance |
| --- | --- | --- | --- |
| Photo capture | Inherited Snapshot flow with full-frame mode selection | A negotiated RGB/YUV still stream | Decodable saved image at selected caps |
| Capture output validation | Treats a still as complete only when camerabin returns a local, non-empty regular file; failed or empty saves reach the error path instead of the gallery | A local filesystem output from the capture pipeline | Empty, missing and non-local still outputs are rejected; valid files are emitted once |
| Preview quality and latency | Restricts the live pipeline to a selected supported 720p-class mode before using a ratio-checked 1080p fallback; still capture remains independently bounded at 2048x1536. Each preview branch keeps one buffer, drops old buffers downstream when the consumer falls behind, and the live sink does not wait for an already-late clock timestamp | Preview caps containing a suitable 640x480–1280x720 mode; GStreamer `queue` and sink QoS | When concrete modes are advertised, the negotiated preview contains only the selected mode and is no taller than 720 pixels; the viewfinder remains near the newest camera timestamp under load |
| Video capture | Inherited recording flow, timer and duration indicator; shutter and page lifecycle remain gated until `video-done`, and invalid empty outputs are rejected before gallery insertion | Working encoder/muxer and stable preview | Playable file, monotonic duration, clean stop, no duplicate stop request during finalization or page navigation |
| Tap-to-focus | Preview gesture, oriented crop mapping and an amber/green/red result reticle | `AfMode`, `AfMetering`, `AfWindows`, `AfTrigger` plus generation-correlated `AfState` transport | Rear lens moves; green is shown only for `Focused`, red for `Failed`/transport error |
| Manual rear focus | Debounced 0–2 `LensPosition` slider; selected actuator position is held until the next tap, Reset or camera switch | `AfModeManual` plus `LensPosition` | Rear lens metadata follows the requested position; fixed-focus cameras disable the control |
| Continuous focus | Return to whole-frame monitoring after Reset | Stable continuous AF implementation | Reset restores continuous mode without a forced scan; no stable-scene hunting |
| Fixed focus | No focus affordance | Camera advertises no AF controls | Front stream works; focus request reports unsupported |
| Exposure | -1..+1 EV UI | Standard `ExposureValue` | Metadata echoes request and pixels move unless sensor-limited |
| Manual shutter and gain | Automatic-exposure switch plus shutter-time and analogue-gain controls | `ExposureTimeMode`, `ExposureTime`, `AnalogueGainMode` and `AnalogueGain` with valid sensor ranges | Manual requests persist for subsequent frames; switching automatic mode restores statistics-driven regulation |
| Sensor-aware startup defaults | Applies the selected sensor's tuned colour/contrast/Gamma defaults even when Aperture selects the first camera during startup | Stable camera model properties (`device.product.name`, `node.nick` or `device.name`) | IMX371, IMX376 and IMX519 defaults are unit-tested; unknown cameras use a conservative fallback |
| Colour | Saturation UI | Standard `Saturation` | Zero is monochrome; supported maximum raises chroma |
| Contrast | Contrast UI | Standard `Contrast` | Preview and saved output change in the same direction |
| Detail | Sharpness UI | Standard `Sharpness` | Ordered edge/detail metric at 0, default and maximum |
| Gamma | Gamma UI with a 0.1–10 range and sensor-aware OnePlus startup defaults | Standard `Gamma` property when advertised by the node | The requested value is transported to preview/capture; unsupported third-party nodes leave the rest of the controls usable |
| Camera calibration | Mobile dialog for grey-card/colour-chart tuning; saves, applies and clears a versioned profile per physical sensor | The same standard image controls plus stable node identity | A profile survives app restart and applies only to its sensor; no ephemeral PipeWire serial is persisted |
| Zoom | One shared 1x–4x value controlled by the image-control slider, two-finger preview pinch and a tappable reset chip; non-finite or sub-1 camera limits safely fall back to 1x | Camerabin zoom/crop | Pinch, slider and chip remain synchronized; two-finger zoom does not submit a tap-focus request; preview and still framing agree without an invalid clamp |
| Timer/grid | Persisted app setting | None beyond capture support | Survives restart and affects only requested capture/UI |
| HDR | Opt-in Software HDR switch; captures dark, normal and bright JPEGs and adds one merged output to the gallery | Three still requests, standard ExposureValue control, GdkPixbuf JPEG decode/encode and the bounded `advanced-snapshot-hdr` helper | Three same-sized frames are registered to the middle exposure with a confidence-gated global translation, merged in linear light, and atomically installed; clipped samples are down-weighted and temporary frames are cleaned up; local/non-rigid motion and vendor-ISP parity remain explicitly out of scope |
| Vendor ISO calibration | No vendor-specific ISO label; manual analogue gain is exposed in linear units | Sensor gain model and a client-side ISO mapping would be required | Not implemented; the gain control remains honest and bounded |
| Hardware flash pulse | Opt-in rear-camera switch; starts a bounded `pmos-camera-flash` pulse and stops it safely on capture failure, camera switch or page teardown | Executable helper with writable `*:flash` LED channels; helper saves/restores values, caps duration at 5 seconds and handles interruption | Verify rear LEDs illuminate for the requested capture and return to their prior values; front camera must never pulse |

Software HDR is deliberately narrower than Android's vendor HDR mode: it uses
three rapid exposure-bracketed JPEGs, bounded global-translation alignment and
a global tone map. Alignment is limited to 96 pixels, rejects ambiguous matches
and cannot compensate independently moving subjects or non-rigid scene motion.
The sequence disables manual exposure and hardware flash. It is an opt-in
feature and does not claim calibrated vendor image quality. The manual
shutter/gain control is deliberately narrower
than Android's vendor-specific exposure UI: it exposes sensor time and linear
gain, not a calibrated ISO number. The hardware-flash control is deliberately narrower than
Android's automatic flash mode. It does not meter the scene, merge HDR frames,
or promise vendor camera image quality. The flash switch is off by default and is
enabled only when the current camera is a rear camera and the helper executable
is installed. The OnePlus 6T package uses a 2.5-second pulse at level 32; the
helper halves the requested level for the yellow LED channel and restores both
channels after a normal or interrupted pulse.

The installed OnePlus 6T r28/r7 stack and current Advanced Snapshot source pass the
non-image lower-layer, package-launch, generation-correlated autofocus and live
pinch-zoom checks. The application deliberately rejects focus-result mode on
an older transport rather than treating an accepted request as optical
success. Visual reticle, saved-photo, preview-latency and video acceptance remain separate UI
tests.

The calibration dialog is a repeatable control-profile tool, not a factory ISP
calibrator. It can tune the controls that the standard node advertises—Gamma,
Saturation, Contrast, Sharpness, Exposure and, on rear sensors, focus—and
stores them under a stable sensor identity in GSettings. The current OnePlus
nodes do not expose a writable white-balance matrix, colour-correction matrix,
lens-shading table or vendor denoise controls, so the dialog does not invent
those values or claim Android-vendor colour parity. Use a grey card or colour
chart in even light and compare saved reference photos when choosing values.

Four pure application tests cover gesture scaling, lower/upper clamping,
invalid gesture values and the displayed value format; Aperture additionally
covers unusable camera zoom limits. The complete r4 source
passed a clean AArch64 release build and all six Aperture tests. Physical r2
testing exposed a touch-arbitration defect: the full-size controls overlay was
picked above the viewfinder, so a gesture controller on the viewfinder never
received those sequences. r3 attaches the gesture to their common Camera
ancestor in GTK capture phase. Physical r3 testing then showed sparse crop
updates during a sustained pinch. r4 applies UI state immediately, schedules
at most one latest camera update every 33 ms and flushes the exact endpoint.
The bounded device trace advanced through 1.0x, 1.5x, 1.9x and 2.7x before its
3.0x endpoint. This is automated device acceptance; physical visual and
preview-latency acceptance of r4 remains open. Commit `fed2784` additionally
keeps each preview branch at one downstream-leaky buffer. That r4-era source
passed four application and six Aperture unit tests in a clean native
GTK/GStreamer environment; the current strict-cap revision and its eight
Aperture tests are recorded in [docs/VALIDATION.md](VALIDATION.md). The
corresponding AArch64 package is now signed and source/package validated; phone
acceptance remains open.
