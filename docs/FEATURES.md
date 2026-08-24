# Feature and acceptance matrix

Advanced Snapshot treats Android camera behavior as a set of independently
testable capabilities. “Present in the interface” and “implemented by the
camera stack” must always agree.

| Capability | Application | PipeWire/libcamera requirement | Acceptance |
| --- | --- | --- | --- |
| Photo capture | Inherited Snapshot flow with full-frame mode selection | A negotiated RGB/YUV still stream | Decodable saved image at selected caps |
| Video capture | Inherited recording flow, timer and duration indicator | Working encoder/muxer and stable preview | Playable file, monotonic duration, clean stop |
| Tap-to-focus | Preview gesture, oriented crop mapping and focus reticle | `AfMode`, `AfMetering`, `AfWindows`, `AfTrigger` | Rear lens moves and AF state reaches focused/failed |
| Continuous focus | Return to whole-frame monitoring after a tap | Stable continuous AF implementation | No lens sweep on Reset and no stable-scene hunting |
| Fixed focus | No focus affordance | Camera advertises no AF controls | Front stream works; focus request reports unsupported |
| Exposure | -1..+1 EV UI | Standard `ExposureValue` | Metadata echoes request and pixels move unless sensor-limited |
| Colour | Saturation UI | Standard `Saturation` | Zero is monochrome; supported maximum raises chroma |
| Contrast | Contrast UI | Standard `Contrast` | Preview and saved output change in the same direction |
| Detail | Sharpness UI | Standard `Sharpness` | Ordered edge/detail metric at 0, default and maximum |
| Zoom | 1x–4x UI | Camerabin zoom/crop | Preview and still framing agree |
| Timer/grid | Persisted app setting | None beyond capture support | Survives restart and affects only requested capture/UI |
| HDR | Hidden/unavailable | Multi-exposure capture, alignment, merge and tone map | Not implemented |
| Manual shutter/ISO | Hidden/unavailable | Advertised controls with valid units and metadata | Not implemented |
| Flash | Hidden until real hardware policy exists | Torch/flash controls plus timing and thermal safety | Not implemented |

The OnePlus 6T reference stack currently passes the lower-layer requirements
for all implemented rows above. Focus-result-driven reticle colour remains an
application/metadata-transport task; the current reticle confirms an accepted
request, not optical success.
