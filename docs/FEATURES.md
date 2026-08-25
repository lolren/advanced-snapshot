# Feature and acceptance matrix

Advanced Snapshot treats Android camera behavior as a set of independently
testable capabilities. “Present in the interface” and “implemented by the
camera stack” must always agree.

| Capability | Application | PipeWire/libcamera requirement | Acceptance |
| --- | --- | --- | --- |
| Photo capture | Inherited Snapshot flow with full-frame mode selection | A negotiated RGB/YUV still stream | Decodable saved image at selected caps |
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

The installed OnePlus 6T r24/r7 stack and Advanced Snapshot r1 pass the
non-image lower-layer, package-launch and generation-correlated autofocus
checks. The application deliberately rejects focus-result mode on an older
transport rather than treating an accepted request as optical success. Visual
reticle, saved-photo and video acceptance remain separate UI tests.

The r2 source adds synchronized pinch zoom. Four pure application tests cover
gesture scaling, lower/upper clamping, invalid gesture values and the displayed
value format; the complete source passed a clean AArch64 release build and all
six Aperture tests. That proves build and state logic only. Touch arbitration,
visible chip placement, preview/still framing and capture quality still require
acceptance on the reference phone before r2 becomes the installed generation.
