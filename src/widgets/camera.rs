// SPDX-License-Identifier: GPL-3.0-or-later
use std::ffi::OsStr;
use std::os::unix::io::OwnedFd;
use std::path::Path;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use anyhow::Context;
use ashpd::desktop::camera;
use gettextrs::gettext;
use gtk::CompositeTemplate;
use gtk::{gio, glib};

use super::CameraControls;
use crate::enums::ControlsLayout;
use crate::{config, utils};

// Camerabin implements digital zoom by updating a videocrop/videoscale chain.
// Feeding that chain every raw touch event can build a visible backlog, so
// pinch updates are coalesced to the latest value at roughly preview-frame
// cadence. The exact final value is still applied when the gesture ends.
const PINCH_ZOOM_UPDATE_INTERVAL: Duration = Duration::from_millis(33);
const FLASH_HELPER: &str = match option_env!("ADVANCED_SNAPSHOT_FLASH_HELPER") {
    Some(path) => path,
    None => "pmos-camera-flash",
};
const FLASH_DURATION_MS: &str = "2500";
const FLASH_LEVEL: &str = "32";

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use gtk::{CallbackAction, Shortcut, ShortcutController, ShortcutTrigger};

    use crate::CaptureMode;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/lolren/AdvancedSnapshot/ui/camera.ui")]
    #[properties(wrapper_type = super::Camera)]
    pub struct Camera {
        pub selection: gtk::SingleSelection,
        pub provider: OnceCell<aperture::DeviceProvider>,
        pub players: RefCell<Option<gtk::MediaFile>>,
        settings: OnceCell<gio::Settings>,
        pub permission_denied: Cell<bool>,

        pub recording_duration: Cell<u32>,
        pub recording_source: RefCell<Option<glib::source::SourceId>>,
        pub adjustment_handler: RefCell<Option<glib::source::SourceId>>,
        pub manual_exposure_handler: RefCell<Option<glib::source::SourceId>>,
        pub flash_generation: Cell<u64>,
        pub flash_process: RefCell<Option<gio::Subprocess>>,
        pub pinch_zoom_start: Cell<f64>,
        pub pinch_zoom_active: Cell<bool>,
        pub pending_pinch_zoom: Cell<Option<f64>>,
        pub pinch_zoom_handler: RefCell<Option<glib::source::SourceId>>,

        #[property(get, set = Self::set_capture_mode, explicit_notify, default)]
        capture_mode: Cell<crate::CaptureMode>,

        #[template_child]
        pub single_landscape_bp: TemplateChild<adw::Breakpoint>,
        #[template_child]
        pub dual_landscape_bp: TemplateChild<adw::Breakpoint>,
        #[template_child]
        pub dual_portrait_bp: TemplateChild<adw::Breakpoint>,

        #[template_child]
        pub recording_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub recording_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub viewfinder: TemplateChild<aperture::Viewfinder>,
        #[template_child]
        pub flash_bin: TemplateChild<crate::FlashBin>,
        #[template_child]
        pub qr_screen_bin: TemplateChild<crate::QrScreenBin>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,

        #[template_child]
        pub guidelines: TemplateChild<crate::GuidelinesBin>,

        #[template_child]
        pub camera_controls: TemplateChild<crate::CameraControls>,

        #[template_child]
        pub bottom_sheet: TemplateChild<adw::BottomSheet>,
        #[template_child]
        pub sheet_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub qr_bottom_sheet: TemplateChild<crate::QrBottomSheet>,
        #[template_child]
        pub exposure_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub auto_exposure_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub shutter_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub gain_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub saturation_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub contrast_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub sharpness_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub zoom_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub zoom_reset_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub reset_image_controls: TemplateChild<gtk::Button>,
        #[template_child]
        pub flash_switch: TemplateChild<gtk::Switch>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Camera {
        const NAME: &'static str = "Camera";
        type Type = super::Camera;
        type ParentType = adw::BreakpointBin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
            klass.set_css_name("camera");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk::template_callbacks]
    impl Camera {
        fn set_capture_mode(&self, capture_mode: crate::CaptureMode) {
            if capture_mode != self.capture_mode.replace(capture_mode) {
                match capture_mode {
                    CaptureMode::Picture => {
                        self.obj().set_shutter_mode(crate::ShutterMode::Picture);
                    }
                    CaptureMode::Video => {
                        self.obj().set_shutter_mode(crate::ShutterMode::Video);
                    }
                    CaptureMode::QrDetection => (),
                };
                self.obj()
                    .set_detect_codes(matches!(capture_mode, CaptureMode::QrDetection));

                self.obj().notify_capture_mode();
            }
        }

        pub fn settings(&self) -> &gio::Settings {
            self.settings
                .get_or_init(|| gio::Settings::new(config::APP_ID))
        }

        #[template_callback]
        fn change_breakpoint(&self, breakpoint: adw::Breakpoint) {
            let obj = self.obj();

            if breakpoint.eq(&self.dual_landscape_bp.get())
                || breakpoint.eq(&self.dual_portrait_bp.get())
            {
                obj.add_css_class("mobile");
            } else {
                obj.remove_css_class("mobile");
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for Camera {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            let provider = aperture::DeviceProvider::instance();
            self.provider.set(provider.clone()).unwrap();

            let create_shortcut = |shortcut, value: CaptureMode| {
                Shortcut::new(
                    ShortcutTrigger::parse_string(shortcut),
                    Some(CallbackAction::new(glib::clone!(
                        #[weak]
                        obj,
                        #[upgrade_or]
                        glib::Propagation::Proceed,
                        move |_, _| {
                            obj.set_capture_mode(value);
                            glib::Propagation::Proceed
                        }
                    ))),
                )
            };

            let controller = ShortcutController::new();
            controller.set_scope(gtk::ShortcutScope::Managed);
            controller.add_shortcut(create_shortcut("p", CaptureMode::Picture));
            controller.add_shortcut(create_shortcut("r", CaptureMode::Video));

            obj.add_controller(controller);

            provider.connect_camera_added(glib::clone!(
                #[weak]
                obj,
                move |provider, _| {
                    obj.update_cameras_button(provider);
                }
            ));
            provider.connect_camera_removed(glib::clone!(
                #[weak]
                obj,
                move |provider, _| {
                    obj.update_cameras_button(provider);
                }
            ));
            obj.update_cameras_button(provider);

            self.viewfinder.connect_state_notify(glib::clone!(
                #[weak]
                obj,
                move |viewfinder| {
                    obj.update_state();
                    if matches!(viewfinder.state(), aperture::ViewfinderState::Ready) {
                        obj.queue_image_adjustments();
                        viewfinder.set_zoom(obj.imp().zoom_scale.value());
                    }
                }
            ));

            // Aperture can select the initial camera while it brings the
            // provider up, before the application-level selector sees it.
            // Apply the same sensor-aware defaults used when switching
            // cameras so the first preview is not left at generic values.
            self.viewfinder.connect_camera_notify(glib::clone!(
                #[weak]
                obj,
                move |viewfinder| {
                    if let Some(camera) = viewfinder.camera() {
                        obj.set_image_control_defaults(&camera);
                    }
                    obj.update_flash_availability();
                }
            ));

            self.viewfinder.connect_code_detected(glib::clone!(
                #[weak]
                obj,
                move |_, code| {
                    match std::str::from_utf8(&code) {
                        Ok(code) => {
                            log::debug!("Detected QR code: {code}");
                            obj.imp().sheet_stack.set_visible_child_name("qr");
                            obj.imp().bottom_sheet.set_open(true);
                            obj.imp().qr_bottom_sheet.set_contents(code);
                        }
                        Err(err) => {
                            log::error!("Could not decode QR code into utf8: {err}");
                        }
                    }
                }
            ));

            self.qr_screen_bin.set_viewfinder(self.viewfinder.clone());

            obj.update_state();

            self.viewfinder.connect_is_recording_notify(glib::clone!(
                #[weak]
                obj,
                move |viewfinder| {
                    let window = viewfinder.root().and_downcast::<crate::Window>().unwrap();

                    if viewfinder.is_recording() {
                        obj.set_shutter_mode(crate::ShutterMode::Recording);
                        window.inhibit("Recording Video");
                        obj.show_recording_label();
                    } else {
                        obj.hide_recording_label();
                        window.uninhibit();
                        if matches!(obj.shutter_mode(), crate::ShutterMode::Recording) {
                            obj.set_shutter_mode(crate::ShutterMode::Video);
                        }
                    }
                }
            ));

            self.selection.set_model(Some(provider));
            self.selection.connect_selected_item_notify(glib::clone!(
                #[weak]
                obj,
                move |selection| {
                    if let Some(selected_item) = selection.selected_item() {
                        let camera = selected_item.downcast::<aperture::Camera>().ok();

                        if matches!(
                            obj.imp().viewfinder.state(),
                            aperture::ViewfinderState::Ready | aperture::ViewfinderState::Error
                        ) {
                            obj.set_camera_inner(camera);
                        }
                    }
                }
            ));

            self.camera_controls.set_selection(self.selection.clone());
            self.camera_controls.connect_camera_switched(glib::clone!(
                #[weak]
                obj,
                move |_: &CameraControls| {
                    obj.camera_switched();
                }
            ));

            for scale in [
                &*self.exposure_scale,
                &*self.saturation_scale,
                &*self.contrast_scale,
                &*self.sharpness_scale,
            ] {
                scale.connect_value_changed(glib::clone!(
                    #[weak]
                    obj,
                    move |_| obj.queue_image_adjustments()
                ));
            }
            self.auto_exposure_switch
                .connect_active_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |switch| {
                        obj.update_manual_exposure_controls();
                        if switch.is_active() {
                            obj.queue_auto_exposure();
                        } else {
                            obj.queue_manual_exposure();
                        }
                    }
                ));
            for scale in [&*self.shutter_scale, &*self.gain_scale] {
                scale.connect_value_changed(glib::clone!(
                    #[weak]
                    obj,
                    move |_| {
                        if !obj.imp().auto_exposure_switch.is_active() {
                            obj.queue_manual_exposure();
                        }
                    }
                ));
            }
            obj.update_manual_exposure_controls();
            self.zoom_scale.connect_value_changed(glib::clone!(
                #[weak]
                obj,
                move |scale| {
                    let zoom = scale.value();
                    obj.imp()
                        .zoom_reset_button
                        .set_label(&format_zoom_label(zoom));
                    obj.request_zoom(zoom);
                }
            ));
            self.zoom_reset_button.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.imp().zoom_scale.set_value(1.0)
            ));

            let zoom_gesture = gtk::GestureZoom::new();
            // The full-size controls layout is a GtkOverlay sibling above the
            // viewfinder. A gesture attached directly to the viewfinder never
            // sees touch sequences whose picked target belongs to that overlay.
            // Capture on their common Camera ancestor so a recognized
            // two-finger sequence can claim both touches before child controls.
            zoom_gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
            zoom_gesture.connect_begin(glib::clone!(
                #[weak]
                obj,
                move |gesture, _| {
                    obj.begin_pinch_zoom();
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            ));
            zoom_gesture.connect_scale_changed(glib::clone!(
                #[weak]
                obj,
                move |gesture, scale_delta| {
                    let imp = obj.imp();
                    let adjustment = imp.zoom_scale.adjustment();
                    let zoom = pinch_zoom_value(
                        imp.pinch_zoom_start.get(),
                        scale_delta,
                        adjustment.lower(),
                        adjustment.upper(),
                    );
                    imp.zoom_scale.set_value(zoom);
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                }
            ));
            zoom_gesture.connect_end(glib::clone!(
                #[weak]
                obj,
                move |_, _| obj.finish_pinch_zoom()
            ));
            zoom_gesture.connect_cancel(glib::clone!(
                #[weak]
                obj,
                move |_, _| obj.finish_pinch_zoom()
            ));
            obj.add_controller(zoom_gesture);

            self.reset_image_controls.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.reset_image_controls()
            ));

            self.settings()
                .bind(
                    "show-composition-guidelines",
                    &*self.guidelines,
                    "draw-guidelines",
                )
                .build();

            self.settings()
                .bind(
                    "enable-audio-recording",
                    &*self.viewfinder,
                    "disable-audio-recording",
                )
                .invert_boolean()
                .build();

            self.settings()
                .bind("hardware-flash", &*self.flash_switch, "active")
                .build();

            obj.update_flash_availability();

            self.settings()
                .bind("capture-mode", &*obj, "capture-mode")
                .build();

            let format = if aperture::is_h264_encoding_supported() {
                log::debug!("Found openh264enc feature, using the h264/mp4 profile");
                aperture::VideoFormat::H264Mp4
            } else {
                log::debug!("Did not find openh264enc feature, using the vp8/webm profile");
                aperture::VideoFormat::Vp8Webm
            };
            self.viewfinder.set_video_format(format);

            self.settings()
                .bind(
                    "enable-hardware-encoding",
                    &*self.viewfinder,
                    "enable-hw-encoding",
                )
                .get_only()
                .build();

            obj.connect_current_breakpoint_notify(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                move |imp| {
                    if imp.current_breakpoint().is_none()
                        || imp
                            .current_breakpoint()
                            .is_some_and(|breakpoint| breakpoint.eq(&obj.dual_portrait_bp.get()))
                    {
                        imp.add_css_class("portrait");
                    } else {
                        imp.remove_css_class("portrait");
                    }
                }
            ));
        }
    }

    impl WidgetImpl for Camera {}
    impl BreakpointBinImpl for Camera {}
}

glib::wrapper! {
    pub struct Camera(ObjectSubclass<imp::Camera>)
        @extends gtk::Widget, adw::BreakpointBin,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible;
}

impl Default for Camera {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Camera {
    pub fn new() -> Self {
        Self::default()
    }

    fn on_portal_not_allowed(&self) {
        // We don't start the device provider if we are not
        // allowed to use cameras.
        self.imp().permission_denied.set(true);
        self.update_state();
    }

    pub async fn start(&self) {
        let provider = self.imp().provider.get().unwrap();

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            #[strong]
            provider,
            async move {
                if let Err(err) = ashpd::register_host_app(config::APP_ID.try_into().unwrap()).await
                {
                    log::error!(
                        "Failed to run org.freedesktop.host.portal.Registry.Register: {err}"
                    );
                }
                match stream().await {
                    Ok(fd) => {
                        if let Err(err) = provider.set_fd(fd) {
                            log::error!("Could not use the camera portal: {err}");
                        };
                    }
                    Err(err) => match err.downcast_ref::<ashpd::Error>() {
                        Some(ashpd::Error::Portal(ashpd::PortalError::NotAllowed(err))) => {
                            log::warn!("Permission to use the camera portal denied: {err:#?}");
                            obj.on_portal_not_allowed();
                            return;
                        }
                        Some(ashpd::Error::Zbus(ashpd::zbus::Error::MethodError(
                            name,
                            _,
                            message,
                        ))) if *name == "org.freedesktop.portal.Error.NotAllowed" => {
                            log::warn!("Permission to use the camera portal denied: {message}");
                            obj.on_portal_not_allowed();
                            return;
                        }
                        _ => (),
                    },
                }

                if let Err(err) = provider.start_with_default(glib::clone!(
                    #[weak]
                    obj,
                    #[upgrade_or]
                    false,
                    move |camera| {
                        let stored_id = obj.imp().settings().string("last-camera-id");
                        !stored_id.is_empty() && id_from_pw(camera) == stored_id
                    }
                )) {
                    log::error!("Could not start the device provider: {err}");
                } else {
                    log::debug!("Device provider started");
                    obj.update_cameras_button(&provider);
                };
            }
        ));
    }

    pub async fn start_recording(&self) -> anyhow::Result<()> {
        let imp = self.imp();
        self.flush_pending_zoom();
        let format = imp.viewfinder.video_format();
        let filename = utils::video_file_name(format);
        let path = utils::videos_dir()?.join(filename);

        imp.viewfinder.start_recording(path)?;

        Ok(())
    }

    pub fn stop_recording(&self) {
        let imp = self.imp();
        if imp.viewfinder.is_recording()
            && let Err(err) = imp.viewfinder.stop_recording()
        {
            log::error!("Could not stop camera: {err}");
        }
    }

    pub async fn take_picture(&self, format: crate::PictureFormat) -> anyhow::Result<()> {
        let imp = self.imp();
        self.flush_pending_zoom();
        let window = self.root().and_downcast::<crate::Window>().unwrap();

        // We enable the shutter whenever picture-stored is emitted.
        window.set_shutter_enabled(false);

        let filename = utils::picture_file_name(format);
        let path = utils::pictures_dir()?.join(filename);

        let flash_started = self.start_hardware_flash();
        if flash_started {
            // Give the helper time to write the LED channels before camerabin
            // starts the still request. The delay is short compared with a
            // normal capture and the helper itself has a hard duration cap.
            glib::timeout_future(Duration::from_millis(80)).await;
        }

        if let Err(err) = imp.viewfinder.take_picture(&path) {
            if flash_started {
                self.stop_hardware_flash();
            }
            return Err(err.into());
        }
        imp.flash_bin.flash();

        let settings = imp.settings();
        if settings.boolean("play-shutter-sound") {
            self.play_shutter_sound();
        }

        Ok(())
    }

    fn camera_switched(&self) {
        self.stop_hardware_flash();
        let provider = self.imp().provider.get().unwrap();

        let current = self.imp().viewfinder.camera();

        let mut pos = 0;
        if current == provider.camera(0) {
            pos += 1;
        };
        if let Some(camera) = provider.camera(pos) {
            self.set_camera_inner(Some(camera));
        }
    }

    fn set_camera_inner(&self, camera: Option<aperture::Camera>) {
        let imp = self.imp();

        if let Some(ref camera) = camera {
            let id = id_from_pw(camera);
            imp.settings().set_string("last-camera-id", &id).unwrap();
        }

        if imp.viewfinder.is_recording() {
            self.stop_recording();
        }

        imp.viewfinder.set_camera(camera);
    }

    fn play_shutter_sound(&self) {
        // If we don't hold a reference to it there is a condition race which
        // will cause the sound to play only sometimes.
        let resource = "/io/github/lolren/AdvancedSnapshot/sounds/camera-shutter.wav";
        let player = gtk::MediaFile::for_resource(resource);
        player.play();

        self.imp().players.replace(Some(player));
    }

    pub fn set_countdown(&self, countdown: u32) {
        self.imp().camera_controls.set_countdown(countdown);
    }

    pub fn start_countdown(&self) {
        self.imp().camera_controls.start_countdown();
    }

    pub fn stop_countdown(&self) {
        self.imp().camera_controls.stop_countdown();
    }

    pub fn shutter_mode(&self) -> crate::ShutterMode {
        self.imp().camera_controls.shutter_mode()
    }

    pub fn set_shutter_mode(&self, shutter_mode: crate::ShutterMode) {
        if matches!(shutter_mode, crate::ShutterMode::Picture) {
            self.stop_recording();
        }
        self.imp().camera_controls.set_shutter_mode(shutter_mode);
    }

    fn set_detect_codes(&self, detect_codes: bool) {
        let imp = self.imp();

        imp.viewfinder.set_detect_codes(detect_codes);
        imp.qr_screen_bin.set_enabled(detect_codes);

        let layout = if detect_codes {
            ControlsLayout::DetectingCodes
        } else {
            ControlsLayout::Default
        };
        imp.camera_controls.set_layout(layout);
    }

    pub fn set_gallery(&self, gallery: crate::Gallery) {
        let imp = self.imp();

        imp.viewfinder.connect_picture_done(glib::clone!(
            #[weak]
            gallery,
            #[weak(rename_to = obj)]
            self,
            move |_, file| {
                obj.stop_hardware_flash();
                let window = obj.root().and_downcast::<crate::Window>().unwrap();
                window.set_shutter_enabled(true);
                if let Some(file) = file {
                    gallery.add_image(file);
                } else {
                    log::error!("Didn't find any file when taking a picture!");
                    window.send_toast(&gettext("Could not save photo"));
                }
            }
        ));
        imp.viewfinder.connect_recording_done(glib::clone!(
            #[weak]
            gallery,
            #[weak(rename_to = obj)]
            self,
            move |_, file| {
                if let Some(window) = obj.root().and_downcast::<crate::Window>() {
                    // Re-enable only after camerabin has emitted video-done.
                    // This prevents a second shutter press from racing the
                    // muxer's finalization and stop-capture transition.
                    window.recording_finished();
                }
                if let Some(file) = file {
                    gallery.add_video(file);
                } else {
                    log::error!("Didn't find any file when recording finished!");
                    if let Some(window) = obj.root().and_downcast::<crate::Window>() {
                        window.send_toast(&gettext("Could not save video"));
                    }
                }
            }
        ));
        imp.camera_controls.set_gallery(&gallery);
    }

    pub fn stop_stream(&self) {
        self.stop_hardware_flash();
        self.clear_manual_exposure_state();
        self.imp().viewfinder.stop_stream();
    }

    pub fn start_stream(&self) {
        self.imp().viewfinder.start_stream();
    }

    pub fn toggle_guidelines(&self) {
        let imp = self.imp();

        imp.guidelines
            .set_draw_guidelines(!imp.guidelines.draw_guidelines());
    }

    pub fn show_image_controls(&self) {
        let imp = self.imp();
        imp.sheet_stack.set_visible_child_name("image-controls");
        imp.bottom_sheet.set_open(true);
    }

    fn queue_image_adjustments(&self) {
        let imp = self.imp();
        if let Some(handler) = imp.adjustment_handler.take() {
            handler.remove();
        }

        let handler = glib::timeout_add_local_once(
            Duration::from_millis(120),
            glib::clone!(
                #[weak(rename_to = camera)]
                self,
                move || {
                    camera.imp().adjustment_handler.take();
                    camera.apply_image_adjustments();
                }
            ),
        );
        imp.adjustment_handler.replace(Some(handler));
    }

    fn clear_manual_exposure_state(&self) {
        if let Some(handler) = self.imp().manual_exposure_handler.take() {
            handler.remove();
        }
    }

    fn update_manual_exposure_controls(&self) {
        let manual = !self.imp().auto_exposure_switch.is_active();
        self.imp().shutter_scale.set_sensitive(manual);
        self.imp().gain_scale.set_sensitive(manual);
    }

    fn queue_manual_exposure(&self) {
        self.clear_manual_exposure_state();
        let handler = glib::timeout_add_local_once(
            Duration::from_millis(120),
            glib::clone!(
                #[weak(rename_to = camera)]
                self,
                move || {
                    camera.imp().manual_exposure_handler.take();
                    camera.apply_manual_exposure();
                }
            ),
        );
        self.imp().manual_exposure_handler.replace(Some(handler));
    }

    fn apply_manual_exposure(&self) {
        let imp = self.imp();
        if imp.auto_exposure_switch.is_active() {
            imp.viewfinder.set_auto_exposure();
            return;
        }

        imp.viewfinder.set_manual_exposure(
            imp.shutter_scale.value().round() as i32,
            imp.gain_scale.value(),
        );
    }

    fn queue_auto_exposure(&self) {
        self.clear_manual_exposure_state();
        let handler = glib::timeout_add_local_once(
            Duration::from_millis(120),
            glib::clone!(
                #[weak(rename_to = camera)]
                self,
                move || {
                    camera.imp().manual_exposure_handler.take();
                    camera.imp().viewfinder.set_auto_exposure();
                }
            ),
        );
        self.imp().manual_exposure_handler.replace(Some(handler));
    }

    fn begin_pinch_zoom(&self) {
        let imp = self.imp();
        if let Some(handler) = imp.pinch_zoom_handler.take() {
            handler.remove();
        }
        imp.pending_pinch_zoom.take();
        imp.pinch_zoom_start.set(imp.zoom_scale.value());
        imp.pinch_zoom_active.set(true);
    }

    fn request_zoom(&self, zoom: f64) {
        let imp = self.imp();
        if !imp.pinch_zoom_active.get() {
            imp.viewfinder.set_zoom(zoom);
            return;
        }

        // Replacing the pending value prevents stale intermediate crops from
        // being replayed after the user's fingers have already moved on.
        imp.pending_pinch_zoom.set(Some(zoom));
        if imp.pinch_zoom_handler.borrow().is_some() {
            return;
        }

        let handler = glib::timeout_add_local_once(
            PINCH_ZOOM_UPDATE_INTERVAL,
            glib::clone!(
                #[weak(rename_to = camera)]
                self,
                move || {
                    let imp = camera.imp();
                    imp.pinch_zoom_handler.take();
                    if let Some(zoom) = imp.pending_pinch_zoom.take() {
                        imp.viewfinder.set_zoom(zoom);
                    }
                }
            ),
        );
        imp.pinch_zoom_handler.replace(Some(handler));
    }

    fn flush_pending_zoom(&self) {
        let imp = self.imp();
        if let Some(handler) = imp.pinch_zoom_handler.take() {
            handler.remove();
        }
        imp.pending_pinch_zoom.take();
        imp.viewfinder.set_zoom(imp.zoom_scale.value());
    }

    fn finish_pinch_zoom(&self) {
        if !self.imp().pinch_zoom_active.replace(false) {
            return;
        }
        self.flush_pending_zoom();
    }

    fn apply_image_adjustments(&self) {
        let imp = self.imp();
        imp.viewfinder.set_image_adjustments(
            imp.exposure_scale.value(),
            imp.saturation_scale.value(),
            imp.contrast_scale.value(),
            imp.sharpness_scale.value(),
        );
    }

    fn reset_image_controls(&self) {
        let imp = self.imp();
        if let Some(camera) = imp.viewfinder.camera() {
            self.set_image_control_defaults(&camera);
            return;
        }

        imp.exposure_scale.set_value(0.0);
        imp.auto_exposure_switch.set_active(true);
        imp.shutter_scale.set_value(8333.0);
        imp.gain_scale.set_value(1.0);
        imp.saturation_scale.set_value(1.25);
        imp.contrast_scale.set_value(1.05);
        imp.sharpness_scale.set_value(1.0);
        imp.zoom_scale.set_value(1.0);
        self.queue_image_adjustments();
    }

    fn update_flash_availability(&self) {
        let imp = self.imp();
        let rear_camera = imp
            .viewfinder
            .camera()
            .is_some_and(|camera| matches!(camera.location(), aperture::CameraLocation::Back));
        imp.flash_switch
            .set_sensitive(rear_camera && flash_helper_available());
    }

    fn start_hardware_flash(&self) -> bool {
        let rear_camera = self
            .imp()
            .viewfinder
            .camera()
            .is_some_and(|camera| matches!(camera.location(), aperture::CameraLocation::Back));
        if !self.imp().flash_switch.is_active() || !rear_camera || !flash_helper_available() {
            return false;
        }

        self.stop_hardware_flash();
        let launcher = gio::SubprocessLauncher::new(
            gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE,
        );
        let process = match launcher.spawn(&[
            OsStr::new(FLASH_HELPER),
            OsStr::new("--pulse"),
            OsStr::new("--duration-ms"),
            OsStr::new(FLASH_DURATION_MS),
            OsStr::new("--level"),
            OsStr::new(FLASH_LEVEL),
        ]) {
            Ok(process) => process,
            Err(err) => {
                log::debug!("Hardware flash helper is unavailable: {err}");
                return false;
            }
        };

        self.imp().flash_process.replace(Some(process.clone()));
        let generation = self.imp().flash_generation.get();
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = camera)]
            self,
            async move {
                match process.communicate_utf8_future(None).await {
                    Ok((_, Some(stderr))) if !stderr.trim().is_empty() => {
                        log::debug!("Hardware flash helper: {}", stderr.trim());
                    }
                    Ok(_) => (),
                    Err(err) => log::debug!("Hardware flash helper stopped: {err}"),
                }
                let imp = camera.imp();
                if imp.flash_generation.get() == generation {
                    imp.flash_process.take();
                }
            }
        ));
        true
    }

    fn stop_hardware_flash(&self) {
        let imp = self.imp();
        imp.flash_generation
            .set(imp.flash_generation.get().wrapping_add(1));
        if let Some(process) = imp.flash_process.take()
            && !process.has_exited()
        {
            // camera-flash handles SIGINT by restoring the values it saved
            // before enabling the LEDs. SIGKILL is intentionally avoided.
            process.send_signal(2);
        }
    }

    fn set_image_control_defaults(&self, camera: &aperture::Camera) {
        let imp = self.imp();
        let (contrast, saturation) = image_control_defaults(&camera.display_name());

        imp.exposure_scale.set_value(0.0);
        imp.auto_exposure_switch.set_active(true);
        imp.shutter_scale.set_value(8333.0);
        imp.gain_scale.set_value(1.0);
        imp.saturation_scale.set_value(saturation);
        imp.contrast_scale.set_value(contrast);
        imp.sharpness_scale.set_value(1.0);
        imp.zoom_scale.set_value(1.0);
        self.queue_image_adjustments();
    }

    pub fn is_recording_active(&self) -> bool {
        self.imp().viewfinder.is_recording()
    }

    fn update_cameras_button(&self, provider: &aperture::DeviceProvider) {
        let imp = self.imp();

        imp.camera_controls
            .update_visible_camera_button(provider.n_items());

        // We need to set the correct selected item at least when loading. The
        // default camera might not be the first one. A similar thing happens
        // when a camera is removed.
        let camera = imp.viewfinder.camera();
        if let Some(pos) = imp
            .selection
            // gtk::SingleSelection will Always returns glib::Object as its
            // gio::ListModel::item_type().
            .iter::<glib::Object>()
            .enumerate()
            .find(|(_pos, cam)| {
                cam.as_ref()
                    .is_ok_and(|c| c.downcast_ref::<aperture::Camera>() == camera.as_ref())
            })
            .map(|(pos, _cam)| pos)
        {
            imp.selection.set_selected(pos as u32);
        }
    }

    fn update_state(&self) {
        let imp = self.imp();

        if imp.permission_denied.get() {
            imp.stack.set_visible_child_name("permission-denied");
            return;
        }

        match imp.viewfinder.state() {
            aperture::ViewfinderState::Loading => {
                imp.stack.set_visible_child_name("loading");
            }
            aperture::ViewfinderState::Ready => {
                imp.stack.set_visible_child_name("camera");
            }
            aperture::ViewfinderState::NoCameras => imp.stack.set_visible_child_name("not-found"),
            aperture::ViewfinderState::Error => {
                imp.stack.set_visible_child_name("camera");

                let window = self.root().and_downcast::<crate::Window>().unwrap();
                window.send_toast(&gettext("Could not play camera stream"));
            }
        }
    }

    fn show_recording_label(&self) {
        let imp = self.imp();

        let source = glib::timeout_add_seconds_local(
            1,
            glib::clone!(
                #[weak(rename_to = obj)]
                self,
                #[upgrade_or]
                glib::ControlFlow::Break,
                move || {
                    let imp = obj.imp();

                    imp.recording_duration.update(|d| d + 1);
                    let duration = imp.recording_duration.get();

                    let minutes = duration.div_euclid(60);
                    let seconds = duration.rem_euclid(60);
                    imp.recording_label
                        .set_label(&format!("{minutes}∶{seconds:02}"));

                    glib::ControlFlow::Continue
                }
            ),
        );
        if let Some(old) = imp.recording_source.replace(Some(source)) {
            old.remove();
        }
        imp.recording_duration.set(0);
        imp.recording_revealer.set_reveal_child(true);
        imp.recording_label.set_label("0∶00");
    }

    fn hide_recording_label(&self) {
        let imp = self.imp();

        if let Some(source) = imp.recording_source.take() {
            source.remove();
            imp.recording_duration.set(0);
            imp.recording_label.set_label("0∶00");
            imp.recording_revealer.set_reveal_child(false);
        }
    }
}

async fn stream() -> anyhow::Result<OwnedFd> {
    let proxy = camera::Camera::new().await?;
    proxy
        .request_access(camera::CameraAccessOptions::default())
        .await
        .context("org.freedesktop.portal.Camera.AccessCamera failed")?;
    let is_present = proxy
        .is_present()
        .await
        .context("org.freedesktop.portal.Camera.IsCameraPresent failed")?;
    log::debug!("org.freedesktop.portal.Camera:IsCameraPresent: {is_present}");

    proxy
        .open_pipe_wire_remote(camera::OpenPipeWireRemoteOptions::default())
        .await
        .context("org.freedesktop.portal.Camera.OpenPipeWireRemote")
}

// Id used to identify the last-used camera.
fn id_from_pw(camera: &aperture::Camera) -> glib::GString {
    camera.display_name()
}

fn pinch_zoom_value(start: f64, scale_delta: f64, lower: f64, upper: f64) -> f64 {
    let start = if start.is_finite() { start } else { lower };
    let scale_delta = if scale_delta.is_finite() && scale_delta > 0.0 {
        scale_delta
    } else {
        1.0
    };

    (start * scale_delta).clamp(lower, upper)
}

fn format_zoom_label(zoom: f64) -> String {
    format!("{zoom:.1}×")
}

fn image_control_defaults(camera_name: &str) -> (f64, f64) {
    let model = camera_name.to_ascii_lowercase();
    if model.contains("imx371") {
        (1.10, 1.25)
    } else if model.contains("imx376") {
        (1.05, 1.15)
    } else {
        // IMX519, and a conservative fallback for other colour sensors.
        (1.05, 1.25)
    }
}

fn flash_helper_available() -> bool {
    if FLASH_HELPER.contains('/') {
        Path::new(FLASH_HELPER).is_file()
    } else {
        glib::find_program_in_path(FLASH_HELPER).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{format_zoom_label, image_control_defaults, pinch_zoom_value};

    #[test]
    fn sensor_defaults_are_selected_for_all_phone_cameras() {
        assert_eq!(image_control_defaults("Sony IMX371 Main"), (1.10, 1.25));
        assert_eq!(image_control_defaults("Sony IMX376 Front"), (1.05, 1.15));
        assert_eq!(image_control_defaults("Sony IMX519 Wide"), (1.05, 1.25));
    }

    #[test]
    fn unknown_camera_uses_conservative_defaults() {
        assert_eq!(image_control_defaults("USB Webcam"), (1.05, 1.25));
    }

    #[test]
    fn pinch_zoom_scales_from_gesture_start() {
        assert_eq!(pinch_zoom_value(1.5, 2.0, 1.0, 4.0), 3.0);
        assert_eq!(pinch_zoom_value(3.0, 0.5, 1.0, 4.0), 1.5);
    }

    #[test]
    fn pinch_zoom_clamps_to_supported_ui_range() {
        assert_eq!(pinch_zoom_value(3.0, 2.0, 1.0, 4.0), 4.0);
        assert_eq!(pinch_zoom_value(2.0, 0.1, 1.0, 4.0), 1.0);
    }

    #[test]
    fn pinch_zoom_rejects_invalid_scale_values() {
        assert_eq!(pinch_zoom_value(2.0, f64::NAN, 1.0, 4.0), 2.0);
        assert_eq!(pinch_zoom_value(2.0, -1.0, 1.0, 4.0), 2.0);
        assert_eq!(pinch_zoom_value(f64::NAN, 2.0, 1.0, 4.0), 2.0);
    }

    #[test]
    fn zoom_label_uses_one_decimal_and_multiplication_sign() {
        assert_eq!(format_zoom_label(1.0), "1.0×");
        assert_eq!(format_zoom_label(2.34), "2.3×");
    }
}
