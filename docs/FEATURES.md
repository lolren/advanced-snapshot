# Feature and acceptance matrix

Advanced Snapshot treats Android camera behavior as a set of independently
testable capabilities. “Present in the interface” and “implemented by the
camera stack” must always agree.

| Capability | Application | PipeWire/libcamera requirement | Acceptance |
| --- | --- | --- | --- |
| Photo capture | Inherited Snapshot flow with full-frame mode selection | A negotiated RGB/YUV still stream | Decodable saved image at selected caps |
| Preview quality and latency | Chooses a supported 720p-class live mode before using a ratio-checked 1080p fallback; still capture remains independently bounded at 2048x1536. Each preview branch keeps one buffer and drops old buffers downstream when the consumer falls behind | Preview caps containing a suitable 640x480–1280x720 mode; GStreamer `queue` | The negotiated preview is no taller than 720 pixels when the camera advertises one, and the viewfinder remains near the newest camera timestamp under load |
| Video capture | Inherited recording flow, timer and duration indicator | Working encoder/muxer and stable preview | Playable file, monotonic duration, clean stop |
| Tap-to-focus | Preview gesture, oriented crop mapping and an amber/green/red result reticle | `AfMode`, `AfMetering`, `AfWindows`, `AfTrigger` plus generation-correlated `AfState` transport | Rear lens moves; green is shown only for `Focused`, red for `Failed`/transport error |
| Continuous focus | Return to whole-frame monitoring after a tap | Stable continuous AF implementation | No lens sweep on Reset and no stable-scene hunting |
| Fixed focus | No focus affordance | Camera advertises no AF controls | Front stream works; focus request reports unsupported |
| Exposure | -1..+1 EV UI | Standard `ExposureValue` | Metadata echoes request and pixels move unless sensor-limited |
| Colour | Saturation UI | Standard `Saturation` | Zero is monochrome; supported maximum raises chroma |
| Contrast | Contrast UI | Standard `Contrast` | Preview and saved output change in the same direction |
| Detail | Sharpness UI | Standard `Sharpness` | Ordered edge/detail metric at 0, default and maximum |
| Zoom | One shared 1x–4x value controlled by the image-control slider, two-finger preview pinch and a tappable reset chip | Camerabin zoom/crop | Pinch, slider and chip remain synchronized; two-finger zoom does not submit a tap-focus request; preview and still framing agree |
| Timer/grid | Persisted app setting | None beyond capture support | Survives restart and affects only requested capture/UI |
| HDR | Hidden/unavailable | Multi-exposure capture, alignment, merge and tone map | Not implemented |
| Manual shutter/ISO | Hidden/unavailable | Advertised controls with valid units and metadata | Not implemented |
| Flash | Hidden until real hardware policy exists | Torch/flash controls plus timing and thermal safety | Not implemented |

The installed OnePlus 6T r24/r7 stack and Advanced Snapshot r4 pass the
non-image lower-layer, package-launch, generation-correlated autofocus and live
pinch-zoom checks. The application deliberately rejects focus-result mode on
an older transport rather than treating an accepted request as optical
success. Visual reticle, saved-photo, preview-latency and video acceptance remain separate UI
tests.

Four pure application tests cover gesture scaling, lower/upper clamping,
invalid gesture values and the displayed value format; the complete r4 source
passed a clean AArch64 release build and all six Aperture tests. Physical r2
testing exposed a touch-arbitration defect: the full-size controls overlay was
picked above the viewfinder, so a gesture controller on the viewfinder never
received those sequences. r3 attaches the gesture to their common Camera
ancestor in GTK capture phase. Physical r3 testing then showed sparse crop
updates during a sustained pinch. r4 applies UI state immediately, schedules
at most one latest camera update every 33 ms and flushes the exact endpoint.
The bounded device trace advanced through 1.0x, 1.5x, 1.9x and 2.7x before its
3.0x endpoint. This is automated device acceptance; physical visual and
preview-latency acceptance of r4 remains open.
