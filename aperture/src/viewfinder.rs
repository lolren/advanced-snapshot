// SPDX-License-Identifier: GPL-3.0-or-later
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

use gst::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene};

use crate::VideoFormat;
use crate::ViewfinderState;
use crate::code_detector::QrCodeDetector;

/// Default bitrate
///
/// This is the Gstreamer 1.26 default value for x264enc, chosen as reasonable compromise between
/// quality and file size. Candidate for a preference.
const DEFAULT_BITRATE: u32 = 2048;
const PROVIDER_TIMEOUT: u64 = 2;
const CAMERA_STATE_TIMEOUT: u64 = 10;
const FOCUS_HELPER: &str = match option_env!("ADVANCED_SNAPSHOT_FOCUS_HELPER") {
    Some(path) => path,
    None => "advanced-snapshot-focus-control",
};

#[derive(Debug)]
enum StateChangeState {
    Equal,
    Differ,
    Error,
    NotDone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum FocusIndicatorState {
    #[default]
    Scanning,
    Focused,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusResult {
    Focused,
    Failed,
}

fn parse_focus_result(output: &str) -> Option<FocusResult> {
    match output.trim() {
        "focused" => Some(FocusResult::Focused),
        "failed" => Some(FocusResult::Failed),
        _ => None,
    }
}

mod imp {
    use std::cell::Cell;
    use std::cell::OnceCell;
    use std::cell::RefCell;

    use glib::Properties;

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::Viewfinder)]
    pub struct Viewfinder {
        #[property(get, explicit_notify, default)]
        state: Cell<ViewfinderState>,
        #[property(get = Self::detect_codes, set = Self::set_detect_codes, explicit_notify)]
        detect_codes: Cell<bool>,
        #[property(get, set = Self::set_camera, nullable, explicit_notify)]
        camera: RefCell<Option<crate::Camera>>,
        #[property(get = Self::is_recording, name = "is-recording", type = bool)]
        pub is_recording_video: RefCell<Option<PathBuf>>,
        #[property(get, set = Self::set_disable_audio_recording, explicit_notify)]
        disable_audio_recording: Cell<bool>,
        #[property(get, set = Self::set_video_format, explicit_notify, default)]
        video_format: Cell<VideoFormat>,
        #[property(get, set = Self::set_enable_hw_encoding, explicit_notify)]
        enable_hw_encoding: Cell<bool>,

        pub qrcode_branch: RefCell<Option<gst::Element>>,
        pub devices: OnceCell<crate::DeviceProvider>,
        pub camera_src: RefCell<Option<gst::Element>>,
        pub camerabin: OnceCell<gst::Element>,
        pub camera_element: OnceCell<gst::Element>,
        pub capsfilter: OnceCell<gst::Element>,
        pub sink_paintable: OnceCell<gst::Element>,
        pub tee: OnceCell<crate::PipelineTee>,
        pub bus_watch: OnceCell<gst::bus::BusWatchGuard>,

        pub is_stopping_recording: Cell<bool>,
        pub is_taking_picture: Cell<bool>,
        pub is_front_camera: Cell<bool>,

        // State changes are asynchronous in GStreamer.  Keep a generation for
        // every start/stop request so an old start cannot put camerabin back
        // into PLAYING after a newer stop or camera reconfiguration.
        pub stream_generation: Cell<u64>,
        pub stream_wanted: Cell<bool>,
        pub stream_stop_pending: Cell<bool>,
        pub camera_reconfigure_generation: Cell<u64>,

        pub timeout_handler: RefCell<Option<glib::SourceId>>,
        pub focus_indicator_handler: RefCell<Option<glib::SourceId>>,
        pub focus_reset_handler: RefCell<Option<glib::SourceId>>,
        pub focus_point: Cell<Option<(f64, f64)>>,
        pub(super) focus_indicator_state: Cell<FocusIndicatorState>,
        pub focus_generation: Cell<u64>,
        pub focus_process: RefCell<Option<gio::Subprocess>>,
        pub adjustment_generation: Cell<u64>,
        pub adjustment_process: RefCell<Option<gio::Subprocess>>,
        pub exposure_generation: Cell<u64>,
        pub exposure_process: RefCell<Option<gio::Subprocess>>,

        pub picture: gtk::Picture,
        pub offload: gtk::GraphicsOffload,
        pub overlay: gtk::Overlay,
        pub focus_overlay: gtk::DrawingArea,
    }

    impl Viewfinder {
        pub fn camerabin(&self) -> &gst::Element {
            self.camerabin.get().unwrap()
        }

        pub(super) fn request_stream_start(&self) -> Option<u64> {
            if self.stream_wanted.replace(true) {
                return None;
            }

            let generation = self.stream_generation.get().wrapping_add(1);
            self.stream_generation.set(generation);
            Some(generation)
        }

        pub(super) fn request_stream_stop(&self) -> u64 {
            self.stream_wanted.set(false);
            self.stream_stop_pending.set(true);
            self.stream_generation
                .set(self.stream_generation.get().wrapping_add(1));
            self.stream_generation.get()
        }

        pub(super) fn stream_request_is_current(&self, generation: u64) -> bool {
            self.stream_wanted.get() && self.stream_generation.get() == generation
        }

        pub(super) fn next_camera_reconfigure_generation(&self) -> u64 {
            let generation = self.camera_reconfigure_generation.get().wrapping_add(1);
            self.camera_reconfigure_generation.set(generation);
            generation
        }

        pub(super) fn camera_reconfigure_is_current(&self, generation: u64) -> bool {
            self.camera_reconfigure_generation.get() == generation
        }

        pub(crate) fn set_state(&self, state: ViewfinderState) {
            if state != self.state.replace(state) {
                self.obj().notify_state();
            }
        }

        pub(super) fn clear_focus_state(&self) {
            self.focus_generation
                .set(self.focus_generation.get().wrapping_add(1));
            if let Some(handler) = self.focus_indicator_handler.take() {
                handler.remove();
            }
            if let Some(handler) = self.focus_reset_handler.take() {
                handler.remove();
            }
            if let Some(process) = self.focus_process.take() {
                process.force_exit();
            }
            self.focus_point.set(None);
            self.focus_indicator_state
                .set(FocusIndicatorState::Scanning);
            self.focus_overlay.set_visible(false);
        }

        pub(super) fn clear_image_adjustment_state(&self) {
            self.adjustment_generation
                .set(self.adjustment_generation.get().wrapping_add(1));
            if let Some(process) = self.adjustment_process.take() {
                process.force_exit();
            }
        }

        pub(super) fn clear_exposure_state(&self) {
            self.exposure_generation
                .set(self.exposure_generation.get().wrapping_add(1));
            if let Some(process) = self.exposure_process.take() {
                process.force_exit();
            }
        }

        fn is_recording(&self) -> bool {
            self.is_recording_video.borrow().is_some()
        }

        fn detect_codes(&self) -> bool {
            self.qrcode_branch.borrow().is_some()
        }

        fn set_detect_codes(&self, value: bool) {
            if value == self.detect_codes.replace(value) {
                return;
            }

            let tee = self.tee.get().unwrap();
            if value {
                match create_qrcode_bin() {
                    Ok(qrcode_branch) => {
                        tee.add_branch(&qrcode_branch);
                        self.qrcode_branch.replace(Some(qrcode_branch));
                    }
                    Err(err) => {
                        log::error!("Could not create qrcode element: {err}");
                    }
                }
            } else if let Some(qrcode_branch) = self.qrcode_branch.take() {
                tee.remove_branch(&qrcode_branch);
            }

            self.obj().notify_detect_codes();
        }

        /// Sets the camera that the `ApertureViewfinder` will use.
        fn set_camera(&self, camera: Option<crate::Camera>) {
            let obj = self.obj();

            if !matches!(obj.state(), ViewfinderState::Ready | ViewfinderState::Error) {
                log::error!("Could not set camera, the viewfinder is not ready");
                return;
            }

            if self.is_taking_picture.get() {
                log::error!("Could not set camera, where are taking a picture");
                return;
            }

            if self.is_recording_video.borrow().is_some() {
                log::error!("Could not set camera, there is a recording in progress");
                return;
            }

            if camera == self.camera.replace(camera.clone()) {
                return;
            }
            let reconfigure_generation = self.next_camera_reconfigure_generation();
            let desired_camera = camera.clone();
            self.clear_focus_state();
            self.clear_image_adjustment_state();
            self.clear_exposure_state();

            // We reset to READY if we landed on the ERROR state on the previous
            // camera.
            if matches!(obj.state(), ViewfinderState::Error) {
                if self
                    .devices
                    .get()
                    .and_then(|devices| devices.camera(0))
                    .is_some()
                {
                    self.set_state(ViewfinderState::Ready);
                } else {
                    self.set_state(ViewfinderState::NoCameras);
                }
            }

            // Camera source changes must happen only after camerabin has fully
            // reached NULL. GStreamer state changes are asynchronous, and
            // changing the wrapper source while the old source is still
            // stopping leaves Android/libcamera with a half-drained stream.
            let stream_active = obj.is_realized()
                && matches!(
                    self.camerabin().current_state(),
                    gst::State::Playing | gst::State::Paused
                );
            if stream_active || self.stream_stop_pending.get() {
                obj.stop_stream();
                obj.reconfigure_camera_after_stop(desired_camera, reconfigure_generation);
            } else {
                if let Some(camera) = desired_camera
                    && let Err(err) = obj.setup_camera_element(&camera)
                {
                    log::error!("Could not reconfigure camera element: {err}");
                    self.set_state(ViewfinderState::Error);
                }

                if obj.is_realized() && matches!(obj.state(), ViewfinderState::Ready) {
                    obj.start_stream();
                }
            }

            obj.notify_camera();
        }

        fn set_disable_audio_recording(&self, value: bool) {
            let obj = self.obj();

            if value != self.disable_audio_recording.replace(value) {
                obj.reset_pipeline();
                obj.notify_disable_audio_recording();
            }
        }

        fn set_video_format(&self, video_format: VideoFormat) {
            let obj = self.obj();

            if video_format != self.video_format.replace(video_format) {
                obj.reset_pipeline();
                obj.notify_video_format();
            }
        }

        fn set_enable_hw_encoding(&self, value: bool) {
            let obj = self.obj();

            if value != self.enable_hw_encoding.replace(value) {
                match self.video_format.get() {
                    VideoFormat::Vp8Webm => (),
                    VideoFormat::H264Mp4 => obj.reset_pipeline(),
                }
                obj.notify_enable_hw_encoding();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Viewfinder {
        const NAME: &'static str = "ApertureViewfinder";
        type Type = super::Viewfinder;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for Viewfinder {
        fn constructed(&self) {
            self.parent_constructed();

            crate::ensure_init();

            let obj = self.obj();

            let camerabin = gst::ElementFactory::make("camerabin")
                .property("location", None::<&str>)
                .build()
                .expect("Missing GStreamer Bad Plug-ins");
            self.camerabin.set(camerabin.clone()).unwrap();

            let bus = self.camerabin().bus().unwrap();
            let watch = bus
                .add_watch_local(glib::clone!(
                    #[weak]
                    obj,
                    #[upgrade_or]
                    glib::ControlFlow::Break,
                    move |_, msg| {
                        obj.on_bus_message(msg);
                        glib::ControlFlow::Continue
                    }
                ))
                .unwrap();
            self.bus_watch.set(watch).unwrap();

            let tee = crate::PipelineTee::new();

            let paintablesink = gst::ElementFactory::make("gtk4paintablesink")
                .build()
                .expect("Missing gst-plugin-gtk4");

            // This sink is only the live viewfinder.  Waiting for the sink
            // clock can display an already-old frame after the compositor or
            // software ISP has fallen behind; the preview queue above is
            // explicitly configured to keep only the newest frame.  Disable
            // clock synchronisation so the sink consumes that newest frame as
            // soon as it arrives, while QoS lets upstream elements observe
            // downstream pressure.  Capture branches are independent and
            // retain their normal timing.
            paintablesink.set_property("sync", false);
            paintablesink.set_property("qos", true);

            let paintable = paintablesink.property::<gdk::Paintable>("paintable");

            let is_yuv_natively_supported = {
                let yuv_caps =
                    gst_video::video_make_raw_caps(&[gst_video::VideoFormat::Yuy2]).build();
                !paintablesink
                    .pad_template("sink")
                    .unwrap()
                    .caps()
                    .intersect(&yuv_caps)
                    .is_empty()
            };
            let sink = if is_yuv_natively_supported {
                let bin = gst::Bin::default();

                bin.add(&paintablesink).unwrap();
                bin.add_pad(
                    &gst::GhostPad::with_target(&paintablesink.static_pad("sink").unwrap())
                        .unwrap(),
                )
                .unwrap();

                bin.upcast()
            } else {
                let is_gl_supported = paintable
                    .property::<Option<gdk::GLContext>>("gl-context")
                    .is_some();
                if is_gl_supported {
                    gst::ElementFactory::make("glsinkbin")
                        .property("sink", &paintablesink)
                        .build()
                        .expect("Missing GStreamer Base Plug-ins")
                } else {
                    let bin = gst::Bin::default();
                    let convert = gst::ElementFactory::make("videoconvert")
                        .build()
                        .expect("Missing GStreamer Base Plug-ins");

                    bin.add(&convert).unwrap();
                    bin.add(&paintablesink).unwrap();
                    convert.link(&paintablesink).unwrap();

                    bin.add_pad(
                        &gst::GhostPad::with_target(&convert.static_pad("sink").unwrap()).unwrap(),
                    )
                    .unwrap();

                    bin.upcast()
                }
            };

            tee.add_branch(&sink);
            camerabin.set_property("viewfinder-sink", &tee);

            let videoconvert_video = gst::ElementFactory::make("videoconvert")
                .build()
                .expect("Missing GStreamer Base Plug-ins");
            camerabin.set_property("video-filter", &videoconvert_video);

            let caps_video = gst_video::video_make_raw_caps(&[
                gst_video::VideoFormat::I420,
                gst_video::VideoFormat::Nv12,
            ])
            .build();
            camerabin.set_property("video-capture-caps", caps_video);

            let videoconvert_image = gst::ElementFactory::make("videoconvert")
                .build()
                .expect("Missing GStreamer Base Plug-ins");
            camerabin.set_property("image-filter", &videoconvert_image);

            self.sink_paintable.set(paintablesink).unwrap();

            self.picture
                .set_accessible_role(gtk::AccessibleRole::Presentation);
            self.picture.set_hexpand(true);
            self.picture.set_vexpand(true);
            self.picture.set_content_fit(gtk::ContentFit::Contain);
            self.picture.set_paintable(Some(&paintable));

            self.offload.set_child(Some(&self.picture));
            self.offload.set_black_background(true);

            self.focus_overlay.set_can_target(false);
            self.focus_overlay.set_hexpand(true);
            self.focus_overlay.set_vexpand(true);
            self.focus_overlay.set_visible(false);
            self.focus_overlay.set_draw_func(glib::clone!(
                #[weak]
                obj,
                move |_, context, width, height| {
                    obj.draw_focus_indicator(context, width, height);
                }
            ));

            self.overlay.set_child(Some(&self.offload));
            self.overlay.add_overlay(&self.focus_overlay);
            self.overlay.set_parent(&*obj);

            let focus_gesture = gtk::GestureClick::new();
            focus_gesture.set_button(gdk::BUTTON_PRIMARY);
            focus_gesture.connect_released(glib::clone!(
                #[weak]
                obj,
                move |_, _, x, y| obj.focus_at(x, y)
            ));
            obj.add_controller(focus_gesture);

            self.tee.set(tee).unwrap();

            let devices = crate::DeviceProvider::instance();

            self.devices.set(devices.clone()).unwrap();

            if devices.started() {
                obj.init();
            } else {
                devices.connect_started_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |_| {
                        obj.init();
                    }
                ));
            }

            devices.connect_camera_added(glib::clone!(
                #[weak]
                obj,
                move |_, camera| {
                    if matches!(
                        obj.state(),
                        ViewfinderState::NoCameras
                            | ViewfinderState::Loading
                            | ViewfinderState::Error
                    ) {
                        obj.imp().set_state(ViewfinderState::Ready);
                        obj.set_camera(Some(camera.clone()));
                    }
                }
            ));

            devices.connect_camera_removed(glib::clone!(
                #[weak]
                obj,
                move |devices, camera| {
                    let imp = obj.imp();
                    if Some(camera) == imp.camera.borrow().as_ref() {
                        obj.cancel_current_operation();

                        let next_camera = devices.camera(0);
                        let is_none = next_camera.is_none();
                        obj.set_camera(next_camera);
                        if is_none {
                            obj.imp().set_state(ViewfinderState::NoCameras);
                        }
                    }
                }
            ));

            obj.setup_recording();
        }

        fn dispose(&self) {
            self.clear_focus_state();
            self.clear_image_adjustment_state();
            if self.is_recording_video.borrow().is_some()
                && let Err(err) = self.obj().stop_recording()
            {
                log::error!("Could not stop recording: {err}");
            }
            self.request_stream_stop();
            if let Err(err) = self.camerabin().set_state(gst::State::Null) {
                log::error!("Could not stop camerabin: {err}");
            }

            self.overlay.unparent();
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: LazyLock<Vec<glib::subclass::Signal>> = LazyLock::new(|| {
                vec![
                    // These are emitted whenever the saving process finishes,
                    // successful or not.
                    glib::subclass::Signal::builder("picture-done")
                        .param_types([Option::<gio::File>::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("recording-done")
                        .param_types([Option::<gio::File>::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("code-detected")
                        .param_types([glib::Bytes::static_type()])
                        .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for Viewfinder {
        fn realize(&self) {
            self.parent_realize();

            if matches!(self.obj().state(), ViewfinderState::Ready) {
                log::debug!("Viewfinder realized: starting stream");
                self.obj().start_stream();
            }
        }

        fn unrealize(&self) {
            log::debug!("Viewfinder unrealized: stopping stream");
            self.clear_image_adjustment_state();
            self.obj().stop_stream();

            self.parent_unrealize();
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if self.is_front_camera.get() {
                let w = self.obj().width() as f32;
                snapshot.save();
                snapshot.translate(&graphene::Point::new(w, 0.0));
                snapshot.scale(-1.0, 1.0);
                self.parent_snapshot(snapshot);
                snapshot.restore();
            } else {
                self.parent_snapshot(snapshot);
            }
        }
    }
}

glib::wrapper! {
    /// A GTK widget for displaying a camera feed and taking pictures and videos from it.
    ///
    /// The viewfinder is the main widget in Aperture, and is responsible for displaying a camera
    /// feed in your UI; along with using that camera feed to do useful tasks, like take pictures,
    /// record video, and detect barcodes.
    ///
    /// The viewfinder does not contain any camera controls, these must be implemented yourself.
    ///
    ///
    /// ## Properties
    ///
    ///
    /// #### `state`
    ///  The current viewfinder state.
    /// The state indicates what the viewfinder is currently doing, or sometimes that an error has
    /// occurred. Many operations, such as taking a picture, require that the viewfinder be in the
    /// [`ViewfinderState::Ready`][crate::ViewfinderState::Ready] state.
    ///
    ///  Readable
    ///
    ///
    /// #### `detect-codes`
    ///  Whether the viewfinder should detect codes.
    /// When a code is detected, the [`code-detected`](#code-detected) signal will be emitted.
    ///
    ///  Readable | Writable
    ///
    /// ### `video-format`
    /// The video format for recordings. `[crate::is_h264_encoding_supported]`
    /// can be used to detect whether there is h264 support.
    ///
    ///  Readable | Writable
    ///
    /// ### `enable-hw-encoding`
    /// Whether to enable hardware video encoding.
    /// `[crate::is_hardware_encoding_supported]` can be used to detect whether
    /// the system supports hardware encoding for a given format.
    ///
    ///  Readable | Writable
    ///
    /// #### `camera`
    ///  The camera that is currently being used.
    /// The [`DeviceProvider`][crate::DeviceProvider] handles obtaining new cameras,
    /// do not create cameras yourself.
    ///
    /// To safely switch cameras, the current [`fn@Viewfinder::state`] must be in [`ViewfinderState::Ready`][crate::ViewfinderState::Ready].
    /// This is because switching camera sources would interrupt most active operations, if any are present.
    ///
    ///  Readable | Nullable
    ///
    ///
    /// ## Signals
    ///
    ///
    /// #### `picture-done`
    ///  This signal is emitted after a picture has been taken and saved.
    /// Note that this signal is emitted even if saving the picture failed, and should not be used
    /// to detect if the picture was successfully saved.
    ///
    ///
    /// #### `recording-done`
    ///  This signal is emitted after a recording has finished and been saved.
    /// Note that this signal is emitted even if saving the recording failed, and should not be used
    /// to detect if the recoding was successfully saved.
    ///
    ///
    /// #### `code-detected`
    ///  This signal is emitted when a barcode is detected in the camera feed.
    /// This will only be emitted if [`detect-codes`](#detect-codes) is `true`.
    ///
    /// Barcodes are only detected when they appear on the feed, not on every frame when they are visible.
    ///
    /// # Implements
    ///
    /// [`gtk::prelude::WidgetExt`][trait@gtk::prelude::WidgetExt], [`glib::ObjectExt`][trait@gtk::glib::ObjectExt]
    pub struct Viewfinder(ObjectSubclass<imp::Viewfinder>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Viewfinder {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Viewfinder {
    /// Creates a new [`Viewfinder`][crate::Viewfinder]
    ///
    /// # Returns
    ///
    /// a new Viewfinder
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the aspect ratio of the camera output.
    ///
    /// # Returns
    ///
    /// an aspect ratio calculated with width/height, or 0 for no valid aspect
    /// ratio.
    pub fn aspect_ratio(&self) -> f64 {
        let imp = self.imp();
        if let Some(paintable) = imp.picture.paintable() {
            paintable.intrinsic_aspect_ratio()
        } else {
            0.0
        }
    }

    fn focus_at(&self, widget_x: f64, widget_y: f64) {
        if !matches!(self.state(), ViewfinderState::Ready) {
            return;
        }

        let Some(camera) = self.camera() else {
            return;
        };
        if !matches!(camera.location(), crate::CameraLocation::Back) {
            return;
        }
        let Some(serial) = camera.target_object() else {
            log::debug!("Tap-to-focus unavailable: camera has no PipeWire serial");
            return;
        };
        let Some((focus_x, focus_y)) = self.focus_coordinates(widget_x, widget_y) else {
            return;
        };

        let imp = self.imp();
        imp.clear_focus_state();
        let generation = imp.focus_generation.get();

        let widget_width = self.width().max(1) as f64;
        let widget_height = self.height().max(1) as f64;
        let indicator_point = (widget_x / widget_width, widget_y / widget_height);
        self.show_focus_indicator(generation, serial, indicator_point);

        let serial_arg = serial.to_string();
        let x_arg = format!("{focus_x:.8}");
        let y_arg = format!("{focus_y:.8}");
        let launcher = gio::SubprocessLauncher::new(
            gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE,
        );
        let process = match launcher.spawn(&[
            OsStr::new(FOCUS_HELPER),
            OsStr::new("focus"),
            OsStr::new(&serial_arg),
            OsStr::new(&x_arg),
            OsStr::new(&y_arg),
            OsStr::new("0.18"),
        ]) {
            Ok(process) => process,
            Err(err) => {
                log::warn!("Could not start tap-to-focus helper: {err}");
                self.complete_focus_indicator(generation, serial, FocusIndicatorState::Failed);
                return;
            }
        };
        imp.focus_process.replace(Some(process.clone()));

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = viewfinder)]
            self,
            async move {
                let result = match process.communicate_utf8_future(None).await {
                    Ok((stdout, _stderr)) if process.has_exited() && process.exit_status() == 0 => {
                        let parsed = stdout
                            .as_ref()
                            .and_then(|output| parse_focus_result(output.as_str()));
                        if parsed.is_none() {
                            log::warn!(
                                "Tap-to-focus helper returned an unknown result: {}",
                                stdout.as_deref().unwrap_or_default().trim()
                            );
                        }
                        parsed
                    }
                    Ok((_, stderr)) => {
                        log::debug!(
                            "Tap-to-focus was not applied (status {}): {}",
                            if process.has_exited() {
                                process.exit_status()
                            } else {
                                -1
                            },
                            stderr.as_deref().unwrap_or_default().trim()
                        );
                        None
                    }
                    Err(err) => {
                        log::debug!("Could not read tap-to-focus result: {err}");
                        None
                    }
                };

                let imp = viewfinder.imp();
                if imp.focus_generation.get() == generation {
                    imp.focus_process.take();
                }

                let indicator_state = match result {
                    Some(FocusResult::Focused) => FocusIndicatorState::Focused,
                    Some(FocusResult::Failed) | None => FocusIndicatorState::Failed,
                };
                viewfinder.complete_focus_indicator(generation, serial, indicator_state);
                viewfinder.schedule_focus_reset(generation, serial);
            }
        ));
    }

    fn focus_coordinates(&self, widget_x: f64, widget_y: f64) -> Option<(f64, f64)> {
        let paintable = self.imp().picture.paintable()?;
        let frame_width = paintable.intrinsic_width() as f64;
        let frame_height = paintable.intrinsic_height() as f64;
        let widget_width = self.width() as f64;
        let widget_height = self.height() as f64;
        if frame_width <= 0.0 || frame_height <= 0.0 || widget_width <= 0.0 || widget_height <= 0.0
        {
            return None;
        }

        let scale = (widget_width / frame_width).min(widget_height / frame_height);
        let display_width = frame_width * scale;
        let display_height = frame_height * scale;
        let offset_x = (widget_width - display_width) / 2.0;
        let offset_y = (widget_height - display_height) / 2.0;
        if widget_x < offset_x
            || widget_x > offset_x + display_width
            || widget_y < offset_y
            || widget_y > offset_y + display_height
        {
            return None;
        }

        Some((
            ((widget_x - offset_x) / display_width).clamp(0.0, 1.0),
            ((widget_y - offset_y) / display_height).clamp(0.0, 1.0),
        ))
    }

    fn show_focus_indicator(&self, generation: u64, serial: u64, point: (f64, f64)) {
        let imp = self.imp();
        if imp.focus_generation.get() != generation
            || self.camera().and_then(|camera| camera.target_object()) != Some(serial)
        {
            return;
        }

        imp.focus_point.set(Some(point));
        imp.focus_indicator_state.set(FocusIndicatorState::Scanning);
        imp.focus_overlay.set_visible(true);
        imp.focus_overlay.queue_draw();
    }

    fn complete_focus_indicator(&self, generation: u64, serial: u64, state: FocusIndicatorState) {
        let imp = self.imp();
        if imp.focus_generation.get() != generation
            || self.camera().and_then(|camera| camera.target_object()) != Some(serial)
        {
            return;
        }

        imp.focus_indicator_state.set(state);
        imp.focus_overlay.queue_draw();
        if let Some(handler) = imp.focus_indicator_handler.take() {
            handler.remove();
        }
        let indicator_handler = glib::timeout_add_local_once(
            Duration::from_millis(1600),
            glib::clone!(
                #[weak(rename_to = viewfinder)]
                self,
                move || {
                    let imp = viewfinder.imp();
                    imp.focus_indicator_handler.take();
                    if imp.focus_generation.get() == generation {
                        imp.focus_point.set(None);
                        imp.focus_overlay.set_visible(false);
                    }
                }
            ),
        );
        imp.focus_indicator_handler.replace(Some(indicator_handler));
    }

    fn schedule_focus_reset(&self, generation: u64, serial: u64) {
        let imp = self.imp();
        if imp.focus_generation.get() != generation
            || self.camera().and_then(|camera| camera.target_object()) != Some(serial)
        {
            return;
        }

        let reset_handler = glib::timeout_add_seconds_local_once(
            8,
            glib::clone!(
                #[weak(rename_to = viewfinder)]
                self,
                move || {
                    viewfinder.imp().focus_reset_handler.take();
                    if viewfinder
                        .camera()
                        .and_then(|camera| camera.target_object())
                        == Some(serial)
                    {
                        viewfinder.reset_focus(serial);
                    }
                }
            ),
        );
        imp.focus_reset_handler.replace(Some(reset_handler));
    }

    fn reset_focus(&self, serial: u64) {
        let serial_arg = serial.to_string();
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        let process = match launcher.spawn(&[
            OsStr::new(FOCUS_HELPER),
            OsStr::new("reset"),
            OsStr::new(&serial_arg),
        ]) {
            Ok(process) => process,
            Err(err) => {
                log::debug!("Could not restore continuous autofocus: {err}");
                return;
            }
        };

        glib::spawn_future_local(async move {
            if let Err(err) = process.wait_check_future().await {
                log::debug!("Could not restore continuous autofocus: {err}");
            }
        });
    }

    /// Applies software-ISP image adjustments to the active camera.
    pub fn set_image_adjustments(
        &self,
        exposure: f64,
        saturation: f64,
        contrast: f64,
        sharpness: f64,
    ) {
        if !matches!(self.state(), ViewfinderState::Ready) {
            return;
        }

        let Some(serial) = self.camera().and_then(|camera| camera.target_object()) else {
            log::debug!("Image adjustments unavailable: camera has no PipeWire serial");
            return;
        };

        let imp = self.imp();
        imp.clear_image_adjustment_state();
        let generation = imp.adjustment_generation.get();

        let arguments = [
            serial.to_string(),
            format!("{:.4}", exposure.clamp(-1.0, 1.0)),
            format!("{:.4}", saturation.clamp(0.0, 2.0)),
            format!("{:.4}", contrast.clamp(0.0, 2.0)),
            format!("{:.4}", sharpness.clamp(0.0, 2.0)),
        ];
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        let process = match launcher.spawn(&[
            OsStr::new(FOCUS_HELPER),
            OsStr::new("adjust"),
            OsStr::new(&arguments[0]),
            OsStr::new(&arguments[1]),
            OsStr::new(&arguments[2]),
            OsStr::new(&arguments[3]),
            OsStr::new(&arguments[4]),
        ]) {
            Ok(process) => process,
            Err(err) => {
                log::warn!("Could not start image-adjustment helper: {err}");
                return;
            }
        };
        imp.adjustment_process.replace(Some(process.clone()));

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = viewfinder)]
            self,
            async move {
                let result = process.wait_check_future().await;
                let imp = viewfinder.imp();
                if imp.adjustment_generation.get() != generation {
                    return;
                }
                imp.adjustment_process.take();
                if let Err(err) = result {
                    log::debug!("Image adjustments were not applied: {err}");
                }
            }
        ));
    }

    /// Sets a fixed sensor exposure time and analogue gain for the active
    /// camera. Values are clamped by the libcamera IPA to the sensor's real
    /// limits; the UI deliberately uses ordinary units (microseconds and
    /// linear gain) instead of register codes.
    pub fn set_manual_exposure(&self, exposure_time_us: i32, analogue_gain: f64) {
        if !matches!(self.state(), ViewfinderState::Ready) {
            return;
        }

        let Some(serial) = self.camera().and_then(|camera| camera.target_object()) else {
            log::debug!("Manual exposure unavailable: camera has no PipeWire serial");
            return;
        };

        let imp = self.imp();
        imp.clear_exposure_state();
        let generation = imp.exposure_generation.get();
        let arguments = [
            serial.to_string(),
            exposure_time_us.max(1).to_string(),
            format!("{:.4}", analogue_gain.clamp(0.1, 256.0)),
        ];
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        let process = match launcher.spawn(&[
            OsStr::new(FOCUS_HELPER),
            OsStr::new("manual"),
            OsStr::new(&arguments[0]),
            OsStr::new(&arguments[1]),
            OsStr::new(&arguments[2]),
        ]) {
            Ok(process) => process,
            Err(err) => {
                log::warn!("Could not start manual-exposure helper: {err}");
                return;
            }
        };
        imp.exposure_process.replace(Some(process.clone()));

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = viewfinder)]
            self,
            async move {
                let result = process.wait_check_future().await;
                let imp = viewfinder.imp();
                if imp.exposure_generation.get() != generation {
                    return;
                }
                imp.exposure_process.take();
                if let Err(err) = result {
                    log::debug!("Manual exposure was not applied: {err}");
                }
            }
        ));
    }

    /// Restores automatic exposure and analogue gain on the active camera.
    pub fn set_auto_exposure(&self) {
        if !matches!(self.state(), ViewfinderState::Ready) {
            return;
        }

        let Some(serial) = self.camera().and_then(|camera| camera.target_object()) else {
            log::debug!("Automatic exposure unavailable: camera has no PipeWire serial");
            return;
        };

        let imp = self.imp();
        imp.clear_exposure_state();
        let generation = imp.exposure_generation.get();
        let serial_arg = serial.to_string();
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        let process = match launcher.spawn(&[
            OsStr::new(FOCUS_HELPER),
            OsStr::new("auto"),
            OsStr::new(&serial_arg),
        ]) {
            Ok(process) => process,
            Err(err) => {
                log::warn!("Could not start automatic-exposure helper: {err}");
                return;
            }
        };
        imp.exposure_process.replace(Some(process.clone()));

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = viewfinder)]
            self,
            async move {
                let result = process.wait_check_future().await;
                let imp = viewfinder.imp();
                if imp.exposure_generation.get() != generation {
                    return;
                }
                imp.exposure_process.take();
                if let Err(err) = result {
                    log::debug!("Automatic exposure was not restored: {err}");
                }
            }
        ));
    }

    /// Sets Camerabin's capture-wide digital zoom factor.
    pub fn set_zoom(&self, zoom: f64) {
        let camerabin = self.imp().camerabin();
        let max_zoom = camerabin.property::<f32>("max-zoom") as f64;
        camerabin.set_property("zoom", clamp_zoom(zoom, max_zoom) as f32);
    }

    fn draw_focus_indicator(&self, context: &gtk::cairo::Context, width: i32, height: i32) {
        let Some((x, y)) = self.imp().focus_point.get() else {
            return;
        };
        let center_x = x * width as f64;
        let center_y = y * height as f64;
        let radius = (width.min(height) as f64 * 0.075).clamp(28.0, 48.0);
        let left = center_x - radius;
        let top = center_y - radius;
        let size = radius * 2.0;

        // A dark keyline keeps the reticle legible against bright scenes.
        context.set_source_rgba(0.0, 0.0, 0.0, 0.72);
        context.set_line_width(5.0);
        context.rectangle(left, top, size, size);
        let _ = context.stroke();

        let (red, green, blue) = match self.imp().focus_indicator_state.get() {
            FocusIndicatorState::Scanning => (1.0, 0.84, 0.16),
            FocusIndicatorState::Focused => (0.22, 0.88, 0.38),
            FocusIndicatorState::Failed => (0.96, 0.22, 0.20),
        };
        context.set_source_rgba(red, green, blue, 1.0);
        context.set_line_width(2.5);
        context.rectangle(left, top, size, size);
        context.move_to(center_x - 5.0, center_y);
        context.line_to(center_x + 5.0, center_y);
        context.move_to(center_x, center_y - 5.0);
        context.line_to(center_x, center_y + 5.0);
        let _ = context.stroke();
    }

    /// Takes a picture.
    ///
    /// The recording will be saved to `location`. This method throws an error
    /// if:
    ///  - we are already recording or taking a picture
    ///  - the [`fn@Viewfinder::state`] of the camera is not
    ///    [`ViewfinderState::Ready`][crate::ViewfinderState::Ready].
    ///
    /// This operation may take a while. The resolution might be changed
    /// temporarily, autofocusing might take place, etc. Basically
    /// everything you'd expect to happen when you click the photo button in
    /// a camera app.
    ///
    /// The [`picture-done`](#picture-done) signal will be emitted when this
    /// operation ends.
    pub fn take_picture<P: AsRef<Path>>(&self, location: P) -> Result<(), crate::CaptureError> {
        let imp = self.imp();

        if !matches!(self.state(), ViewfinderState::Ready) {
            return Err(crate::CaptureError::NotReady);
        }

        if imp.is_taking_picture.get() {
            return Err(crate::CaptureError::SnapshotInProgress);
        }

        if imp.is_recording_video.borrow().is_some() {
            return Err(crate::CaptureError::RecordingInProgress);
        }

        // Set after we cannot fail anymore.
        imp.is_taking_picture.set(true);

        self.set_tags();

        let camerabin = imp.camerabin();
        camerabin.set_property_from_str("mode", "mode-image");
        camerabin.set_property("location", location.as_ref().display().to_string());
        camerabin.emit_by_name::<()>("start-capture", &[]);

        Ok(())
    }

    /// Starts recording a video.
    ///
    /// The recording will be saved to `location`. This method throws an error
    /// if:
    ///  - we are already recording or taking a picture
    ///  - the [`fn@Viewfinder::state`] of the camera is not
    ///    [`ViewfinderState::Ready`][crate::ViewfinderState::Ready].
    pub fn start_recording<P: AsRef<Path>>(&self, location: P) -> Result<(), crate::CaptureError> {
        let imp = self.imp();

        if !matches!(self.state(), ViewfinderState::Ready) {
            return Err(crate::CaptureError::NotReady);
        }

        if imp.is_taking_picture.get() {
            return Err(crate::CaptureError::SnapshotInProgress);
        }

        if imp.is_recording_video.borrow().is_some() {
            return Err(crate::CaptureError::RecordingInProgress);
        }

        // Set after we cannot fail anymore.
        if imp
            .is_recording_video
            .replace(Some(location.as_ref().to_owned()))
            .is_none_or(|old| old != location.as_ref())
        {
            self.notify_is_recording();
        };

        let camerabin = imp.camerabin();
        camerabin.set_property_from_str("mode", "mode-video");
        camerabin.set_property("location", location.as_ref().display().to_string());

        self.set_tags();

        camerabin.emit_by_name::<()>("start-capture", &[]);

        Ok(())
    }

    /// Stop recording video.
    ///
    /// This method throws an error if:
    /// - [`fn@Viewfinder::start_recording`] hasn't been called
    /// - There is another [`fn@Viewfinder::stop_recording`] operation in
    ///   progress.
    ///
    /// The [`recording-done`](#recording-done) signal will be emitted when this
    /// operation ends.
    pub fn stop_recording(&self) -> Result<(), crate::CaptureError> {
        let imp = self.imp();

        if !imp.is_recording_video.borrow().is_some() {
            return Err(crate::CaptureError::NoRecordingToStop);
        }

        if imp.is_stopping_recording.get() {
            return Err(crate::CaptureError::StopRecordingInProgress);
        }

        imp.is_stopping_recording.set(true);

        imp.camerabin().emit_by_name::<()>("stop-capture", &[]);

        Ok(())
    }

    pub fn connect_picture_done<F: Fn(&Self, Option<&gio::File>) + 'static>(&self, f: F) {
        self.connect_closure(
            "picture-done",
            false,
            glib::closure_local!(|obj, file| {
                f(obj, file);
            }),
        );
    }

    pub fn connect_recording_done<F: Fn(&Self, Option<&gio::File>) + 'static>(&self, f: F) {
        self.connect_closure(
            "recording-done",
            false,
            glib::closure_local!(|obj, file| {
                f(obj, file);
            }),
        );
    }

    pub fn connect_code_detected<F: Fn(&Self, glib::Bytes) + 'static>(&self, f: F) {
        self.connect_closure(
            "code-detected",
            false,
            glib::closure_local!(|obj, data| {
                f(obj, data);
            }),
        );
    }

    /// Starts the viewfinder.
    pub fn start_stream(&self) {
        let imp = self.imp();
        let Some(generation) = imp.request_stream_start() else {
            return;
        };

        if imp.stream_stop_pending.get() {
            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    if !obj.wait_for_camerabin_state(gst::State::Null).await {
                        let imp = obj.imp();
                        if imp.stream_generation.get() == generation {
                            imp.stream_stop_pending.set(false);
                            imp.set_state(ViewfinderState::Error);
                        }
                        log::error!("Camerabin did not reach NULL before restart");
                        return;
                    }

                    let imp = obj.imp();
                    if !imp.stream_request_is_current(generation) {
                        return;
                    }
                    imp.stream_stop_pending.set(false);
                    obj.start_stream_now(generation);
                }
            ));
            return;
        }

        self.start_stream_now(generation);
    }

    fn start_stream_now(&self, generation: u64) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                obj.change_state_inner(gst::State::Playing, generation)
                    .await;
            }
        ));
    }

    async fn wait_for_camerabin_state(&self, expected: gst::State) -> bool {
        let (sender, receiver) = futures_channel::oneshot::channel();

        let camerabin = self.imp().camerabin();
        std::thread::spawn(glib::clone!(
            #[weak]
            camerabin,
            move || {
                let timeout = gst::format::ClockTime::from_seconds(CAMERA_STATE_TIMEOUT);
                let state = camerabin
                    .state(Some(timeout))
                    .ok()
                    .map(|(change_done, current_state, _pending_state)| {
                        change_done != gst::StateChangeSuccess::Async && current_state == expected
                    })
                    .unwrap_or(false);
                let _ = sender.send(state);
            }
        ));

        receiver.await.unwrap_or(false)
    }

    fn reconfigure_camera_after_stop(&self, camera: Option<crate::Camera>, generation: u64) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                if !obj.wait_for_camerabin_state(gst::State::Null).await {
                    let imp = obj.imp();
                    if imp.camera_reconfigure_is_current(generation) {
                        imp.stream_stop_pending.set(false);
                        imp.set_state(ViewfinderState::Error);
                    }
                    log::error!("Camerabin did not reach NULL before camera switch");
                    return;
                }

                let imp = obj.imp();
                if !imp.camera_reconfigure_is_current(generation) {
                    return;
                }
                imp.stream_stop_pending.set(false);

                if let Some(camera) = camera
                    && let Err(err) = obj.setup_camera_element(&camera)
                {
                    log::error!("Could not reconfigure camera element: {err}");
                    imp.set_state(ViewfinderState::Error);
                    return;
                }

                if obj.is_realized() && matches!(obj.state(), ViewfinderState::Ready) {
                    obj.start_stream();
                }
            }
        ));
    }

    // It is not needed to call this for gst::State::Null.
    async fn change_state_inner(&self, state: gst::State, generation: u64) {
        if !self.imp().stream_request_is_current(generation) {
            return;
        }

        let (sender, receiver) = futures_channel::oneshot::channel();

        let camerabin = self.imp().camerabin();
        std::thread::spawn(glib::clone!(#[weak] camerabin, move || {
            let timeout = gst::format::ClockTime::from_seconds(2);
            let (res, current_state, pending_state) = camerabin.state(Some(timeout));
            let new_state_is = match res {
                Ok(change_done) => {
                    if change_done == gst::StateChangeSuccess::Async {
                        camerabin.set_locked_state(true);
                        log::debug!("Camerabin could not change its state from {current_state:?} to {pending_state:?}");
                        StateChangeState::NotDone
                    } else if current_state == state {
                        StateChangeState::Equal
                    } else {
                        StateChangeState::Differ
                    }
                }
                Err(err) => {
                    log::error!("Previous camerabin state changed failed: {err}");
                    StateChangeState::Error
                }
            };
            sender.send(new_state_is).unwrap();
        }))
            .join()
            .unwrap();

        let change_state = receiver.await.unwrap();
        if !self.imp().stream_request_is_current(generation) {
            log::debug!("Discarding stale camerabin state request {state:?}");
            return;
        }

        match change_state {
            StateChangeState::Equal => {
                // Nothing to do, the new state matches the current one.
            }
            StateChangeState::NotDone => {
                log::debug!("Aborting camerabin state change {state:?}");
                camerabin.abort_state();
                camerabin.set_locked_state(false);
                self.set_camerabin_state(state);
            }
            // If the previous state change failed, we might as well try to set it now.
            StateChangeState::Error | StateChangeState::Differ => self.set_camerabin_state(state),
        }
    }

    fn set_camerabin_state(&self, state: gst::State) {
        match self.imp().camerabin().set_state(state) {
            Err(err) => {
                log::error!("Could not start camerabin: {err}");
                self.imp().set_state(ViewfinderState::Error);
            }
            Ok(gst::StateChangeSuccess::Async) => {
                log::debug!("Trying to set camerabin state to {state:?}");
            }
            Ok(_) => log::debug!("Camerabin successfully state set to {state:?}"),
        }
    }

    /// Stops the viewfinder.
    ///
    /// A black frame will be shown after this methods has been called.
    pub fn stop_stream(&self) {
        let imp = self.imp();
        // Invalidate any in-flight asynchronous PLAYING request before
        // changing the GStreamer state.  This ordering is the important part:
        // the pending future may finish at any time after this call returns.
        let generation = imp.request_stream_stop();
        imp.clear_image_adjustment_state();
        imp.clear_exposure_state();
        if let Err(err) = imp.camerabin().set_state(gst::State::Null) {
            log::error!("Could not pause camerabin: {err}");
            imp.set_state(ViewfinderState::Error);
        } else {
            log::debug!("Camerabin state successfully set to NULL");

            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    if !obj.wait_for_camerabin_state(gst::State::Null).await {
                        let imp = obj.imp();
                        if imp.stream_generation.get() == generation {
                            imp.stream_stop_pending.set(false);
                            imp.set_state(ViewfinderState::Error);
                        }
                        log::error!("Camerabin did not reach NULL while stopping");
                        return;
                    }

                    let imp = obj.imp();
                    if imp.stream_generation.get() == generation {
                        imp.stream_stop_pending.set(false);
                    }
                }
            ));
        }
    }

    /// Bus message handler for the pipeline
    fn on_bus_message(&self, msg: &gst::Message) {
        match msg.view() {
            gst::MessageView::Error(msg) => self.on_pipeline_error(msg),
            gst::MessageView::Element(msg) => match msg.structure() {
                Some(s) if s.has_name("image-done") => {
                    let path = s.get::<PathBuf>("filename").unwrap();
                    let file = gio::File::for_path(path);
                    self.on_image_done(&file);
                }
                Some(s) if s.has_name("video-done") => {
                    self.on_video_done();
                }
                Some(s) if s.has_name("qrcode") => {
                    let data = s.get::<glib::Bytes>("payload").unwrap();

                    self.emit_code_detected(data);
                }
                _ => (),
            },
            _ => (),
        }
    }

    fn on_image_done(&self, file: &gio::File) {
        self.imp().is_taking_picture.set(false);

        let Some(path) = file.path() else {
            log::error!("Still capture returned a non-local output file");
            self.emit_picture_done(None);
            return;
        };

        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                self.emit_picture_done(Some(file));
            }
            Ok(metadata) => {
                log::error!(
                    "Still capture produced an invalid output: file={} size={}",
                    metadata.is_file(),
                    metadata.len()
                );
                self.emit_picture_done(None);
            }
            Err(err) => {
                log::error!("Still capture output is unavailable: {err}");
                self.emit_picture_done(None);
            }
        }
    }

    fn on_video_done(&self) {
        self.imp().is_stopping_recording.set(false);

        if let Some(path) = self.imp().is_recording_video.take() {
            self.notify_is_recording();
            match std::fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                    let file = gio::File::for_path(path);
                    self.emit_recording_done(Some(&file));
                }
                Ok(metadata) => {
                    log::error!(
                        "Video pipeline produced an invalid output: file={} size={}",
                        metadata.is_file(),
                        metadata.len()
                    );
                    self.emit_recording_done(None);
                }
                Err(err) => {
                    log::error!("Video pipeline output is unavailable: {err}");
                    self.emit_recording_done(None);
                }
            }
        }
    }

    fn on_pipeline_error(&self, err: &gst::message::Error) {
        log::error!(
            "Bus Error from {:?}\n{}\n{:?}",
            err.src().map(|s| s.path_string()),
            err.error(),
            err.debug()
        );

        let imp = self.imp();
        // An error can arrive while a state transition is still pending.  A
        // clean NULL transition releases libcamera/PipeWire resources and the
        // generation bump prevents the older transition from reviving them.
        self.cancel_current_operation();
        self.stop_stream();
        imp.set_state(ViewfinderState::Error);
    }

    fn cancel_current_operation(&self) {
        let imp = self.imp();

        if imp.is_taking_picture.replace(false) {
            self.emit_picture_done(None);
        }
        if imp.is_recording_video.replace(None).is_some() {
            self.notify_is_recording();
            self.emit_recording_done(None);
        }
        imp.is_stopping_recording.set(false);
    }

    fn emit_picture_done(&self, file: Option<&gio::File>) {
        self.emit_by_name::<()>("picture-done", &[&file]);
    }

    fn emit_recording_done(&self, file: Option<&gio::File>) {
        self.emit_by_name::<()>("recording-done", &[&file]);
    }

    fn emit_code_detected(&self, data: glib::Bytes) {
        log::info!("Code detected: {}", String::from_utf8_lossy(&data));
        self.emit_by_name::<()>("code-detected", &[&data]);
    }

    fn set_tags(&self) {
        let imp = self.imp();

        let tagsetter = imp
            .camerabin()
            .dynamic_cast_ref::<gst::TagSetter>()
            .unwrap();
        tagsetter.add_tag::<gst::tags::ApplicationName>(
            crate::APP_ID.get().unwrap(),
            gst::TagMergeMode::Replace,
        );

        if let Some(datetime) = gst::DateTime::new_now_local_time() {
            tagsetter.add_tag::<gst::tags::DateTime>(&datetime, gst::TagMergeMode::Replace);
        }
        if let Some(camera) = self.camera() {
            let device_model = camera.display_name();
            tagsetter.add_tag::<gst::tags::DeviceModel>(
                &device_model.as_str(),
                gst::TagMergeMode::Replace,
            );
        }
    }

    fn setup_recording(&self) {
        use gst_pbutils::encoding_profile::EncodingProfileBuilder;
        use gst_pbutils::{ElementProperties, ElementPropertiesMapItem};

        // Video encoder properties
        let video_properties_map = ElementProperties::builder_map()
            .item(
                ElementPropertiesMapItem::builder("x264enc")
                    .field("bitrate", DEFAULT_BITRATE)
                    // tune "zerolatency": Suitable for live-sources like cameras. Crucial to avoid
                    //                     draining the buffer pool.
                    .field("tune", 4)
                    // speed-preset "faster": Lower CPU usage compared to the default "medium" with
                    //                        minimal reduction of quality, see
                    //                        https://streaminglearningcenter.com/wp-content/uploads/2019/10/Choosing-an-x264-Preset_1.pdf
                    .field("speed-preset", 4)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("openh264enc")
                    .field("bitrate", DEFAULT_BITRATE * 1024)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vah264lpenc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vah264enc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vulkanh264enc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vp8enc")
                    .field("target-bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vavp8lpenc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .item(
                ElementPropertiesMapItem::builder("vavp8enc")
                    .field("bitrate", DEFAULT_BITRATE)
                    .build(),
            )
            .build();

        let image_properties_map = ElementProperties::builder_map()
            .item(
                ElementPropertiesMapItem::builder("jpegenc")
                    .field("quality", 95)
                    // idct-method "float": Slowest, most accurate method.
                    .field("idct-method", 2)
                    .build(),
            )
            .build();

        let video_profile = match self.video_format() {
            VideoFormat::H264Mp4 => {
                let mut hw_encoder_found = false;
                let registry = gst::Registry::get();
                if let Some(encoder) = registry.lookup_feature("vah264lpenc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 2);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                if let Some(encoder) = registry.lookup_feature("vah264enc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 1);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                if let Some(encoder) = registry.lookup_feature("v4l2h264enc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 1);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                if let Some(encoder) = registry.lookup_feature("openh264enc") {
                    encoder.set_rank(gst::Rank::PRIMARY);
                }
                if let Some(encoder) = registry.lookup_feature("x264enc") {
                    encoder.set_rank(gst::Rank::PRIMARY - 1);
                }
                log::debug!(
                    "Setting up recording with h264/mp4 profile {} hw acceleration",
                    if self.enable_hw_encoding() && hw_encoder_found {
                        "with"
                    } else {
                        "without"
                    }
                );

                let caps = gst::Caps::builder("video/quicktime").build();
                let mut container_profile = gst_pbutils::EncodingContainerProfile::builder(&caps)
                    .name("MP4 audio/video")
                    .description("Standard MP4/H264/MP3");

                let video_profile = gst_pbutils::EncodingVideoProfile::builder(
                    &gst::Caps::builder("video/x-h264").build(),
                )
                .variable_framerate(true)
                .element_properties(video_properties_map)
                .build();
                container_profile = container_profile.add_profile(video_profile);

                if !self.disable_audio_recording() {
                    let audio_profile = gst_pbutils::EncodingAudioProfile::builder(
                        &gst::Caps::builder("audio/mpeg").build(),
                    )
                    .build();
                    container_profile = container_profile.add_profile(audio_profile);
                }

                container_profile.build()
            }
            VideoFormat::Vp8Webm => {
                let mut hw_encoder_found = false;
                let registry = gst::Registry::get();
                if let Some(encoder) = registry.lookup_feature("vavp8lpenc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 2);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                if let Some(encoder) = registry.lookup_feature("vavp8enc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 1);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                if let Some(encoder) = registry.lookup_feature("v4l2vp8enc") {
                    if self.enable_hw_encoding() {
                        encoder.set_rank(gst::Rank::PRIMARY + 1);
                    } else {
                        encoder.set_rank(gst::Rank::NONE);
                    }
                    hw_encoder_found = true;
                }
                log::debug!(
                    "Setting up recording with vp8/webm profile {} hw acceleration",
                    if self.enable_hw_encoding() && hw_encoder_found {
                        "with"
                    } else {
                        "without"
                    }
                );

                let caps = gst::Caps::builder("video/webm").build();
                let mut container_profile = gst_pbutils::EncodingContainerProfile::builder(&caps)
                    .name("WebM audio/video")
                    .description("Standard WebM/VP8/Vorbis");

                let video_profile = gst_pbutils::EncodingVideoProfile::builder(
                    &gst::Caps::builder("video/x-vp8").build(),
                )
                .preset("Profile Realtime")
                .variable_framerate(true)
                .element_properties(video_properties_map)
                .build();
                container_profile = container_profile.add_profile(video_profile);

                if !self.disable_audio_recording() {
                    let audio_profile = gst_pbutils::EncodingAudioProfile::builder(
                        &gst::Caps::builder("audio/x-vorbis").build(),
                    )
                    .build();
                    container_profile = container_profile.add_profile(audio_profile);
                }

                container_profile.build()
            }
        };

        let image_profile =
            gst_pbutils::EncodingVideoProfile::builder(&gst::Caps::builder("image/jpeg").build())
                .variable_framerate(true)
                .element_properties(image_properties_map)
                .build();

        let camerabin = self.imp().camerabin();
        camerabin.set_property("video-profile", video_profile);
        camerabin.set_property("image-profile", image_profile);
    }

    fn init(&self) {
        let imp = self.imp();
        let devices = imp.devices.get().unwrap();

        if let Some(camera) = devices.default_camera().or_else(|| devices.camera(0))
            && matches!(
                self.state(),
                ViewfinderState::NoCameras | ViewfinderState::Loading | ViewfinderState::Error
            )
        {
            imp.set_state(ViewfinderState::Ready);
            self.set_camera(Some(camera));
        }

        glib::timeout_add_local_once(
            std::time::Duration::from_secs(PROVIDER_TIMEOUT),
            glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move || {
                    if matches!(obj.state(), ViewfinderState::Loading) {
                        obj.imp().set_state(ViewfinderState::NoCameras);
                    }
                }
            ),
        );
    }

    fn create_camera_element(
        &self,
        device_src: &gst::Element,
    ) -> Result<gst::Element, glib::BoolError> {
        use gst::prelude::*;

        let bin = gst::Bin::new();

        let capsfilter = gst::ElementFactory::make("capsfilter").build()?;
        let decodebin3 = gst::ElementFactory::make("decodebin3").build()?;
        let capsfilter_post_decode = gst::ElementFactory::make("capsfilter").build()?;
        let caps_post_decode = gst::Caps::builder("video/x-raw").build();
        capsfilter_post_decode.set_property("caps", &caps_post_decode);

        bin.add_many([
            device_src,
            &capsfilter,
            &decodebin3,
            &capsfilter_post_decode,
        ])?;
        gst::Element::link_many([device_src, &capsfilter, &decodebin3])?;

        self.imp().capsfilter.set(capsfilter).unwrap();

        let (sender, receiver) = futures_channel::oneshot::channel::<bool>();
        let sender = std::sync::Arc::new(std::sync::Mutex::new(Some(sender)));
        decodebin3.connect_pad_added(glib::clone!(#[weak] capsfilter_post_decode, move |_, pad| {
            if pad.stream().is_some_and(|stream| matches!(stream.stream_type(), gst::StreamType::VIDEO)) {
                let has_succeeded = pad.link(&capsfilter_post_decode.static_pad("sink").unwrap())
                                       .inspect_err(|err| {
                                           log::error!("Failed to link decodebin3:video_%u pad with capsfilter_post_decode:sink pad: {err}");
                                       })
                                       .is_ok();
                let mut guard = sender.lock().unwrap();
                if let Some(sender) = guard.take() {
                    let _ = sender.send(has_succeeded);
                }
            }
        }));

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = viewfinder)]
            self,
            async move {
                let has_succeeded = receiver.await.unwrap_or_default();
                if !has_succeeded {
                    viewfinder.imp().set_state(ViewfinderState::Error);
                }
            }
        ));

        let pad = capsfilter_post_decode.static_pad("src").unwrap();
        let ghost_pad = gst::GhostPad::with_target(&pad)?;
        ghost_pad.set_active(true)?;

        bin.add_pad(&ghost_pad)?;

        let wrappercamerabinsrc = gst::ElementFactory::make("wrappercamerabinsrc")
            .property("video-source", &bin)
            .build()
            .expect("Missing GStreamer Bad Plug-ins");

        Ok(wrappercamerabinsrc)
    }

    fn setup_camera_element(&self, camera: &crate::Camera) -> Result<(), glib::BoolError> {
        let imp = self.imp();

        if let Some(element) = imp.camera_element.get() {
            camera.reconfigure(element)?;
        } else {
            let element = camera.create_element()?;

            let wrapper = self.create_camera_element(&element)?;
            imp.camerabin().set_property("camera-source", &wrapper);

            imp.camera_element.set(element).unwrap();
        }

        if let Some(capsfilter) = imp.capsfilter.get() {
            let caps = camera.best_caps();
            capsfilter.set_property("caps", &caps);
        }
        if let Some(caps) = camera.best_image_caps() {
            imp.camerabin().set_property("image-capture-caps", &caps);
        }

        let is_front_camera = !matches!(camera.location(), crate::CameraLocation::Back);
        imp.is_front_camera.set(is_front_camera);

        Ok(())
    }

    fn reset_pipeline(&self) {
        let imp = self.imp();
        if matches!(
            imp.camerabin().current_state(),
            gst::State::Playing | gst::State::Paused
        ) || imp.stream_stop_pending.get()
        {
            self.stop_stream();
            let generation = imp.stream_generation.get();
            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    if !obj.wait_for_camerabin_state(gst::State::Null).await {
                        let imp = obj.imp();
                        if imp.stream_generation.get() == generation {
                            imp.stream_stop_pending.set(false);
                            imp.set_state(ViewfinderState::Error);
                        }
                        log::error!("Camerabin did not reach NULL before pipeline reset");
                        return;
                    }

                    let imp = obj.imp();
                    if imp.stream_generation.get() != generation {
                        return;
                    }
                    imp.stream_stop_pending.set(false);
                    obj.setup_recording();
                    if obj.is_realized() && matches!(obj.state(), ViewfinderState::Ready) {
                        obj.start_stream();
                    }
                }
            ));
        } else {
            self.setup_recording();
        }
    }
}

fn clamp_zoom(zoom: f64, max_zoom: f64) -> f64 {
    let upper = if max_zoom.is_finite() {
        if max_zoom < 1.0 {
            1.0
        } else {
            max_zoom.min(4.0)
        }
    } else {
        1.0
    };

    if zoom.is_finite() {
        zoom.clamp(1.0, upper)
    } else {
        1.0
    }
}

fn create_qrcode_bin() -> Result<gst::Element, glib::BoolError> {
    let bin = gst::Bin::new();

    let videorate = gst::ElementFactory::make("videorate").build()?;
    videorate.set_property("max-rate", 5);
    videorate.set_property("drop-only", true);
    let videoconvert = gst::ElementFactory::make("videoconvert").build()?;

    // Ensure a copy is made
    let capsfilter = gst::ElementFactory::make("capsfilter").build()?;
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", gst_video::VideoFormat::Gray8.to_str())
        .build();
    capsfilter.set_property("caps", &caps);

    let queue = gst::ElementFactory::make("queue").build()?;
    let qrcode = QrCodeDetector::new().upcast::<gst::Element>();
    let fakesink = gst::ElementFactory::make("fakesink").build()?;

    bin.add_many([
        &videorate,
        &videoconvert,
        &capsfilter,
        &queue,
        &qrcode,
        &fakesink,
    ])
    .unwrap();
    gst::Element::link_many([
        &videorate,
        &videoconvert,
        &capsfilter,
        &queue,
        &qrcode,
        &fakesink,
    ])
    .unwrap();

    let pad = videorate.static_pad("sink").unwrap();
    let ghost_pad = gst::GhostPad::with_target(&pad).unwrap();
    ghost_pad.set_active(true).unwrap();
    bin.add_pad(&ghost_pad).unwrap();

    Ok(bin.upcast())
}

#[cfg(test)]
mod tests {
    use super::{FocusResult, clamp_zoom, parse_focus_result};

    #[test]
    fn parses_truthful_focus_results() {
        assert_eq!(parse_focus_result("focused\n"), Some(FocusResult::Focused));
        assert_eq!(parse_focus_result("failed\n"), Some(FocusResult::Failed));
    }

    #[test]
    fn rejects_missing_or_ambiguous_focus_results() {
        assert_eq!(parse_focus_result(""), None);
        assert_eq!(parse_focus_result("scanning"), None);
        assert_eq!(parse_focus_result("focused\nfailed"), None);
    }

    #[test]
    fn keeps_zoom_clamp_valid_for_unusable_camera_limits() {
        assert_eq!(clamp_zoom(2.0, 0.0), 1.0);
        assert_eq!(clamp_zoom(2.0, f64::NAN), 1.0);
        assert_eq!(clamp_zoom(f64::NAN, 4.0), 1.0);
        assert_eq!(clamp_zoom(5.0, 3.0), 3.0);
    }
}
