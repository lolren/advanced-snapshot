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
| Zoom | 1x–4x UI | Camerabin zoom/crop | Preview and still framing agree |
| Timer/grid | Persisted app setting | None beyond capture support | Survives restart and affects only requested capture/UI |
| HDR | Hidden/unavailable | Multi-exposure capture, alignment, merge and tone map | Not implemented |
| Manual shutter/ISO | Hidden/unavailable | Advertised controls with valid units and metadata | Not implemented |
| Flash | Hidden until real hardware policy exists | Torch/flash controls plus timing and thermal safety | Not implemented |

The installed OnePlus 6T r24/r6 stack passes the earlier lower-layer controls.
The generation-correlated focus transport and UI are implemented in source and
must be installed and accepted together as the next PipeWire/Application
package revisions. The application deliberately rejects focus-result mode on
an older transport rather than treating an accepted request as optical success.
