#!/usr/bin/env python3
"""Exercise Camerabin still-mode negotiation without starting the GTK UI."""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

import gi

gi.require_version("Gst", "1.0")
gi.require_version("GLib", "2.0")
from gi.repository import GLib, Gst  # noqa: E402


FORMATS = "{ BGRx, RGBx, RGBA, BGRA }"


def raw_caps(width: int, height: int) -> Gst.Caps:
    return Gst.Caps.from_string(
        "video/x-raw(memory:SystemMemory),"
        f"format=(string){FORMATS},width=(int){width},height=(int){height},"
        "framerate=(fraction)30/1"
    )


def camera_location(device: Gst.Device) -> str:
    properties = device.get_properties()
    if properties is None or not properties.has_field("api.libcamera.location"):
        return "unknown"
    return properties.get_string("api.libcamera.location") or "unknown"


def find_camera(location: str) -> Gst.Device:
    monitor = Gst.DeviceMonitor.new()
    monitor.add_filter("Video/Source", Gst.Caps.from_string("video/x-raw"))
    if not monitor.start():
        raise RuntimeError("GStreamer camera device monitor did not start")

    try:
        cameras = [
            device
            for device in monitor.get_devices()
            if camera_location(device) == location
        ]
        if not cameras:
            available = ", ".join(
                f"{device.get_display_name()} ({camera_location(device)})"
                for device in monitor.get_devices()
            )
            raise RuntimeError(
                f"No {location} camera found; discovered: {available or 'none'}"
            )
        return cameras[0]
    finally:
        monitor.stop()


def make_source_bin(device: Gst.Device, source_caps: Gst.Caps) -> Gst.Bin:
    source = device.create_element(None)
    if source is None:
        raise RuntimeError("Camera device did not create a source element")
    if source.find_property("client-name") is not None:
        source.set_property("client-name", "advanced-snapshot-capture-probe")

    source_filter = Gst.ElementFactory.make("capsfilter", "probe-source-filter")
    decoder = Gst.ElementFactory.make("decodebin3", "probe-decoder")
    raw_filter = Gst.ElementFactory.make("capsfilter", "probe-raw-filter")
    if source_filter is None or decoder is None or raw_filter is None:
        raise RuntimeError("Missing capsfilter or decodebin3")

    source_filter.set_property("caps", source_caps)
    raw_filter.set_property("caps", Gst.Caps.from_string("video/x-raw"))

    source_bin = Gst.Bin.new("probe-source-bin")
    source_bin.add(source)
    source_bin.add(source_filter)
    source_bin.add(decoder)
    source_bin.add(raw_filter)
    if not source.link(source_filter) or not source_filter.link(decoder):
        raise RuntimeError("Could not link camera source to decoder")

    def link_video_pad(_decoder: Gst.Element, pad: Gst.Pad) -> None:
        sink = raw_filter.get_static_pad("sink")
        if sink.is_linked():
            return
        caps = pad.query_caps(None)
        if caps.can_intersect(Gst.Caps.from_string("video/x-raw")):
            result = pad.link(sink)
            print(f"decoder-pad-link={result.value_nick}", flush=True)

    decoder.connect("pad-added", link_video_pad)
    ghost = Gst.GhostPad.new("src", raw_filter.get_static_pad("src"))
    ghost.set_active(True)
    source_bin.add_pad(ghost)
    return source_bin


def run(args: argparse.Namespace) -> int:
    Gst.init(None)
    output = Path(args.output)

    def output_for(capture_number: int) -> Path:
        if args.captures == 1:
            return output
        return output.with_name(f"{output.stem}-{capture_number}{output.suffix}")

    for capture_number in range(1, args.captures + 1):
        candidate = output_for(capture_number)
        if candidate.exists():
            candidate.unlink()

    device = find_camera(args.location)
    print(
        f"camera={device.get_display_name()} location={camera_location(device)}",
        flush=True,
    )

    preview_caps = raw_caps(1280, 720)
    still_caps = raw_caps(2048, 1536)
    if args.strategy == "ordered":
        source_caps = preview_caps.copy()
        source_caps.append(still_caps.copy())
    else:
        source_caps = still_caps.copy()

    source_bin = make_source_bin(device, source_caps)
    wrapper = Gst.ElementFactory.make("wrappercamerabinsrc", "probe-wrapper")
    camerabin = Gst.ElementFactory.make("camerabin", "probe-camerabin")
    sink = Gst.ElementFactory.make("fakesink", "probe-viewfinder")
    image_filter = Gst.ElementFactory.make("videoconvert", "probe-image-filter")
    if wrapper is None or camerabin is None or sink is None or image_filter is None:
        raise RuntimeError("Missing Camerabin test elements")

    wrapper.set_property("video-source", source_bin)
    if args.strategy in ("full-resolution-source", "fixed-resolution"):
        scaler = Gst.ElementFactory.make("videoscale", "probe-source-scaler")
        if scaler is None:
            raise RuntimeError("Missing videoscale")
        wrapper.set_property("video-source-filter", scaler)

    sink.set_property("sync", False)
    sink.set_property("async", False)
    camerabin.set_property("camera-source", wrapper)
    camerabin.set_property("viewfinder-sink", sink)
    viewfinder_caps = (
        still_caps if args.strategy == "fixed-resolution" else preview_caps
    )
    camerabin.set_property("viewfinder-caps", viewfinder_caps)
    camerabin.set_property("image-filter", image_filter)
    camerabin.set_property("image-capture-caps", still_caps)
    camerabin.set_property("mode", 1)
    camerabin.set_property("location", os.fspath(output_for(1)))

    loop = GLib.MainLoop()
    result = {"status": 1, "capture_started": False, "completed": 0}

    def fail(message: str) -> None:
        print(f"FAIL: {message}", file=sys.stderr, flush=True)
        result["status"] = 1
        loop.quit()

    def on_message(_bus: Gst.Bus, message: Gst.Message) -> None:
        if message.type == Gst.MessageType.ERROR:
            error, debug = message.parse_error()
            fail(f"{message.src.get_path_string()}: {error}; {debug}")
            return
        if message.type != Gst.MessageType.ELEMENT:
            return
        structure = message.get_structure()
        if structure is not None and structure.get_name() == "image-done":
            filename = structure.get_string("filename")
            expected = output_for(result["completed"] + 1)
            saved = Path(filename) if filename else expected
            if saved == expected and saved.is_file() and saved.stat().st_size > 0:
                result["completed"] += 1
                print(
                    f"image-done={result['completed']} image={saved} "
                    f"bytes={saved.stat().st_size}",
                    flush=True,
                )
            else:
                fail(
                    f"image-done mismatch: expected={expected} received={saved} "
                    f"exists={saved.is_file()}"
                )
                return

            if result["completed"] < args.captures:
                next_output = output_for(result["completed"] + 1)
                camerabin.set_property("location", os.fspath(next_output))
                GLib.timeout_add(1000, capture)
            else:
                def finish_after_stable_preview() -> bool:
                    result["status"] = 0
                    print(
                        f"PASS: captures={result['completed']} preview-stable=2s",
                        flush=True,
                    )
                    loop.quit()
                    return GLib.SOURCE_REMOVE

                GLib.timeout_add_seconds(2, finish_after_stable_preview)

    bus = camerabin.get_bus()
    bus.add_signal_watch()
    bus.connect("message", on_message)

    def capture() -> bool:
        result["capture_started"] = True
        if result["completed"] == 0:
            image_pad = wrapper.get_static_pad("imgsrc")
            image_capsfilter = camerabin.get_by_name("imagebin-capsfilter")
            print(f"imgsrc-query={image_pad.query_caps(None).to_string()}", flush=True)
            allowed = image_pad.get_allowed_caps()
            print(
                f"imgsrc-allowed={allowed.to_string() if allowed is not None else 'NULL'}",
                flush=True,
            )
            if image_capsfilter is not None:
                sink_pad = image_capsfilter.get_static_pad("sink")
                print(
                    f"imagebin-filter={image_capsfilter.get_property('caps').to_string()}",
                    flush=True,
                )
                print(
                    f"imagebin-sink-query={sink_pad.query_caps(None).to_string()}",
                    flush=True,
                )
        print(f"capture=start number={result['completed'] + 1}", flush=True)
        camerabin.emit("start-capture")
        return GLib.SOURCE_REMOVE

    def timeout() -> bool:
        fail(
            "timed out "
            + ("after start-capture" if result["capture_started"] else "starting preview")
        )
        return GLib.SOURCE_REMOVE

    state_result = camerabin.set_state(Gst.State.PLAYING)
    if state_result == Gst.StateChangeReturn.FAILURE:
        fail("Camerabin rejected PLAYING")
    else:
        GLib.timeout_add_seconds(3, capture)
        GLib.timeout_add_seconds(10 + args.captures * 6, timeout)
        loop.run()

    camerabin.set_state(Gst.State.NULL)
    camerabin.get_state(10 * Gst.SECOND)
    bus.remove_signal_watch()
    return result["status"]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--location", choices=("front", "back"), default="front")
    parser.add_argument(
        "--strategy",
        choices=("ordered", "full-resolution-source", "fixed-resolution"),
        default="ordered",
    )
    parser.add_argument("--output", required=True)
    parser.add_argument("--captures", type=int, choices=range(1, 11), default=1)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
