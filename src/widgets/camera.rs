// SPDX-License-Identifier: GPL-3.0-or-later
use std::ffi::OsStr;
use std::ffi::OsString;
use std::os::unix::io::OwnedFd;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use anyhow::Context;
use ashpd::desktop::camera;
use gettextrs::gettext;
use gtk::CompositeTemplate;
use gtk::{gdk, gio, glib, graphene};

use super::CameraControls;
use crate::enums::ControlsLayout;
use crate::{camera_profile, config, utils};

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
const HDR_HELPER: &str = match option_env!("ADVANCED_SNAPSHOT_HDR_HELPER") {
    Some(path) => path,
    None => "advanced-snapshot-hdr",
};
const HDR_SETTLE_TIME: Duration = Duration::from_millis(220);
const HDR_EXPOSURE_OFFSETS: [f64; 3] = [-1.0, 0.0, 1.0];
const COLOUR_PRESET_SENSOR_DEFAULT: u32 = 0;
const COLOUR_PRESET_NEUTRAL: u32 = 1;
const COLOUR_PRESET_NATURAL: u32 = 2;
const COLOUR_PRESET_VIVID: u32 = 3;
const COLOUR_PRESET_CUSTOM: u32 = 4;

#[derive(Debug)]
pub(super) struct HdrCapture {
    output: PathBuf,
    inputs: Vec<PathBuf>,
    exposures: [f64; 3],
    next_index: usize,
    completed: Vec<PathBuf>,
}

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
        pub white_balance_handler: RefCell<Option<glib::source::SourceId>>,
        pub custom_colour_matrix: Cell<bool>,
        pub colour_matrix: Cell<[f64; 9]>,
        pub colour_matrix_reset_pending: Cell<bool>,
        pub suppress_colour_preset: Cell<bool>,
        pub manual_focus_handler: RefCell<Option<glib::source::SourceId>>,
        pub suppress_manual_focus: Cell<bool>,
        pub flash_generation: Cell<u64>,
        pub flash_process: RefCell<Option<gio::Subprocess>>,
        pub hdr_generation: Cell<u64>,
        pub(super) hdr_capture: RefCell<Option<HdrCapture>>,
        pub hdr_process: RefCell<Option<gio::Subprocess>>,
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
        pub image_controls_revealer: TemplateChild<gtk::Revealer>,
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
        pub image_controls_scroll: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub exposure_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub auto_exposure_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub focus_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub auto_focus_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub shutter_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub gain_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub auto_white_balance_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub red_gain_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub blue_gain_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub saturation_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub contrast_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub sharpness_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub gamma_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub colour_preset_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub zoom_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub zoom_reset_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub reset_image_controls: TemplateChild<gtk::Button>,
        #[template_child]
        pub calibration_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub flash_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub hdr_switch: TemplateChild<gtk::Switch>,
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
            self.colour_matrix.set(camera_profile::IDENTITY_CCM);

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
                        obj.queue_white_balance();
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

            self.exposure_scale.connect_value_changed(glib::clone!(
                #[weak]
                obj,
                move |_| obj.queue_image_adjustments()
            ));
            for scale in [
                &*self.saturation_scale,
                &*self.contrast_scale,
                &*self.sharpness_scale,
                &*self.gamma_scale,
            ] {
                scale.connect_value_changed(glib::clone!(
                    #[weak]
                    obj,
                    move |_| {
                        obj.mark_colour_preset_custom();
                        obj.queue_image_adjustments();
                    }
                ));
            }
            self.colour_preset_dropdown
                .set_model(Some(&gtk::StringList::new(&[
                    "Sensor default",
                    "Neutral",
                    "Natural",
                    "Vivid",
                    "Custom",
                ])));
            self.colour_preset_dropdown
                .connect_selected_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |dropdown| obj.apply_colour_preset(dropdown.selected())
                ));
            obj.set_colour_preset_selection(COLOUR_PRESET_SENSOR_DEFAULT);
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
            self.auto_white_balance_switch
                .connect_active_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |_| {
                        obj.update_manual_white_balance_controls();
                        obj.queue_white_balance();
                    }
                ));
            for scale in [&*self.red_gain_scale, &*self.blue_gain_scale] {
                scale.connect_value_changed(glib::clone!(
                    #[weak]
                    obj,
                    move |_| {
                        if !obj.imp().auto_white_balance_switch.is_active() {
                            obj.queue_white_balance();
                        }
                    }
                ));
            }
            obj.update_manual_white_balance_controls();
            self.focus_scale.connect_value_changed(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    if !obj.imp().suppress_manual_focus.get() {
                        obj.queue_manual_focus();
                    }
                }
            ));
            self.auto_focus_button.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.enable_auto_focus()
            ));
            self.calibration_button.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.show_calibration()
            ));

            let focus_gesture = gtk::GestureClick::new();
            focus_gesture.set_button(gdk::BUTTON_PRIMARY);
            focus_gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
            focus_gesture.connect_released(glib::clone!(
                #[weak]
                obj,
                move |_, n_press, x, y| {
                    if n_press != 1
                        || obj.imp().bottom_sheet.is_open()
                        || obj.imp().image_controls_revealer.reveals_child()
                    {
                        return;
                    }

                    let viewfinder = &*obj.imp().viewfinder;
                    if let Some(picked) = obj.pick(x, y, gtk::PickFlags::DEFAULT)
                        && is_focus_control(&picked)
                    {
                        return;
                    }

                    let point =
                        obj.compute_point(viewfinder, &graphene::Point::new(x as f32, y as f32));
                    let Some(point) = point else {
                        return;
                    };
                    if point.x() < 0.0
                        || point.y() < 0.0
                        || point.x() >= viewfinder.width() as f32
                        || point.y() >= viewfinder.height() as f32
                    {
                        return;
                    }
                    viewfinder.focus_at_point(point.x() as f64, point.y() as f64);
                }
            ));
            obj.add_controller(focus_gesture);

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

            self.settings()
                .bind("hdr-capture", &*self.hdr_switch, "active")
                .build();

            obj.update_flash_availability();
            obj.update_hdr_availability();

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

        // Do not capture an intermediate frame while the rear lens is still
        // traversing its contrast-detect scan. The wait is bounded in the
        // helper and is a no-op for fixed-focus or older camera stacks.
        window.set_shutter_enabled(false);
        imp.viewfinder.wait_for_focus().await;

        if imp.settings().boolean("hdr-capture") {
            return self.start_hdr_capture(format).await;
        }

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

    async fn start_hdr_capture(&self, format: crate::PictureFormat) -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(format, crate::PictureFormat::Jpeg),
            "software HDR currently writes JPEG output only"
        );
        anyhow::ensure!(
            hdr_helper_available(),
            "the advanced-snapshot-hdr helper is not installed"
        );
        anyhow::ensure!(
            self.imp().auto_exposure_switch.is_active(),
            "software HDR requires automatic exposure"
        );
        anyhow::ensure!(
            self.imp().hdr_capture.borrow().is_none(),
            "an HDR capture is already in progress"
        );

        let window = self.root().and_downcast::<crate::Window>().unwrap();
        let pictures_dir = utils::pictures_dir()?;
        let output = pictures_dir.join(utils::picture_file_name(format));
        let token = format!("{}-{}", std::process::id(), glib::random_int());
        let inputs = (0..HDR_EXPOSURE_OFFSETS.len())
            .map(|index| pictures_dir.join(format!(".advanced-snapshot-hdr-{token}-{index}.jpg")))
            .collect::<Vec<_>>();
        let base_exposure = self.imp().exposure_scale.value().clamp(-1.0, 1.0);

        self.imp().hdr_capture.replace(Some(HdrCapture {
            output,
            inputs,
            exposures: hdr_exposure_values(base_exposure),
            next_index: 0,
            completed: Vec::new(),
        }));
        self.set_hdr_controls_sensitive(false);
        window.set_shutter_enabled(false);

        if let Err(error) = self.capture_next_hdr_frame().await {
            self.abort_hdr_capture(Some(&error.to_string()));
            return Err(error);
        }

        self.imp().flash_bin.flash();
        if self.imp().settings().boolean("play-shutter-sound") {
            self.play_shutter_sound();
        }

        Ok(())
    }

    async fn capture_next_hdr_frame(&self) -> anyhow::Result<()> {
        let (path, exposure) = {
            let mut capture = self.imp().hdr_capture.borrow_mut();
            let Some(capture) = capture.as_mut() else {
                anyhow::bail!("HDR capture state disappeared");
            };
            let index = capture.next_index;
            anyhow::ensure!(
                index < capture.inputs.len() && index < capture.exposures.len(),
                "HDR capture sequence exceeded its frame limit"
            );
            capture.next_index += 1;
            (capture.inputs[index].clone(), capture.exposures[index])
        };

        self.apply_hdr_exposure(exposure);
        glib::timeout_future(HDR_SETTLE_TIME).await;
        self.imp().viewfinder.take_picture(&path)?;
        Ok(())
    }

    fn apply_hdr_exposure(&self, exposure: f64) {
        let imp = self.imp();
        imp.viewfinder.set_image_adjustments(
            exposure.clamp(-1.0, 1.0),
            imp.saturation_scale.value(),
            imp.contrast_scale.value(),
            imp.sharpness_scale.value(),
            imp.gamma_scale.value(),
        );
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

        self.abort_hdr_capture(None);
        self.clear_manual_exposure_state();
        self.clear_white_balance_state();
        self.clear_manual_focus_state();

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
                if obj.imp().hdr_capture.borrow().is_some() {
                    obj.handle_hdr_picture_done(gallery.clone(), file);
                    return;
                }
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
        self.abort_hdr_capture(None);
        self.clear_manual_exposure_state();
        self.clear_white_balance_state();
        self.clear_manual_focus_state();
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
        /*
         * Keep the already-bound panel in the camera-page overlay drawer.
         * AdwBottomSheet measures a GtkScrolledWindow at its minimum height;
         * on a phone that minimum is zero, so using the sheet for camera
         * tuning can produce an empty strip. The overlay has a bounded,
         * scrollable height and keeps the upper live preview visible while
         * controls are changed.
         */
        if imp.image_controls_revealer.child().is_none() {
            imp.image_controls_scroll.unparent();
            imp.image_controls_revealer
                .set_child(Some(&*imp.image_controls_scroll));
        }
        imp.image_controls_revealer
            .set_reveal_child(!imp.image_controls_revealer.reveals_child());
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

    fn clear_white_balance_state(&self) {
        if let Some(handler) = self.imp().white_balance_handler.take() {
            handler.remove();
        }
    }

    fn update_manual_white_balance_controls(&self) {
        let manual = !self.imp().auto_white_balance_switch.is_active();
        self.imp().red_gain_scale.set_sensitive(manual);
        self.imp().blue_gain_scale.set_sensitive(manual);
    }

    fn queue_white_balance(&self) {
        self.clear_white_balance_state();
        let handler = glib::timeout_add_local_once(
            Duration::from_millis(120),
            glib::clone!(
                #[weak(rename_to = camera)]
                self,
                move || {
                    camera.imp().white_balance_handler.take();
                    camera.apply_white_balance();
                }
            ),
        );
        self.imp().white_balance_handler.replace(Some(handler));
    }

    fn apply_white_balance(&self) {
        let imp = self.imp();
        if imp.auto_white_balance_switch.is_active() {
            imp.colour_matrix_reset_pending.set(false);
            imp.viewfinder.set_auto_white_balance();
        } else if imp.custom_colour_matrix.get() {
            imp.colour_matrix_reset_pending.set(false);
            imp.viewfinder.set_manual_colour_calibration(
                imp.red_gain_scale.value(),
                imp.blue_gain_scale.value(),
                imp.colour_matrix.get(),
            );
        } else if imp.colour_matrix_reset_pending.replace(false) {
            imp.viewfinder.set_manual_colour_calibration(
                imp.red_gain_scale.value(),
                imp.blue_gain_scale.value(),
                camera_profile::IDENTITY_CCM,
            );
        } else {
            imp.viewfinder
                .set_manual_white_balance(imp.red_gain_scale.value(), imp.blue_gain_scale.value());
        }
    }

    fn clear_manual_focus_state(&self) {
        if let Some(handler) = self.imp().manual_focus_handler.take() {
            handler.remove();
        }
    }

    fn queue_manual_focus(&self) {
        self.clear_manual_focus_state();
        let handler = glib::timeout_add_local_once(
            Duration::from_millis(80),
            glib::clone!(
                #[weak(rename_to = camera)]
                self,
                move || {
                    camera.imp().manual_focus_handler.take();
                    camera.apply_manual_focus();
                }
            ),
        );
        self.imp().manual_focus_handler.replace(Some(handler));
    }

    fn apply_manual_focus(&self) {
        self.imp()
            .viewfinder
            .set_manual_focus(self.imp().focus_scale.value());
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
            imp.gamma_scale.value(),
        );
    }

    fn current_profile(&self) -> camera_profile::CameraProfile {
        let imp = self.imp();
        camera_profile::CameraProfile {
            exposure: imp.exposure_scale.value(),
            auto_exposure: imp.auto_exposure_switch.is_active(),
            shutter_us: imp.shutter_scale.value(),
            analogue_gain: imp.gain_scale.value(),
            auto_white_balance: imp.auto_white_balance_switch.is_active(),
            red_gain: imp.red_gain_scale.value(),
            blue_gain: imp.blue_gain_scale.value(),
            custom_colour_matrix: imp.custom_colour_matrix.get(),
            colour_matrix: imp.colour_matrix.get(),
            gamma: imp.gamma_scale.value(),
            saturation: imp.saturation_scale.value(),
            contrast: imp.contrast_scale.value(),
            sharpness: imp.sharpness_scale.value(),
            focus: imp.focus_scale.value(),
            restore_manual_focus: false,
        }
    }

    fn default_profile(&self, camera: &aperture::Camera) -> camera_profile::CameraProfile {
        let camera_name = camera_model_name(camera);
        let (contrast, saturation) = image_control_defaults(&camera_name);
        camera_profile::CameraProfile {
            exposure: 0.0,
            auto_exposure: true,
            shutter_us: 8333.0,
            analogue_gain: 1.0,
            auto_white_balance: true,
            red_gain: 1.0,
            blue_gain: 1.0,
            custom_colour_matrix: false,
            colour_matrix: camera_profile::IDENTITY_CCM,
            gamma: image_gamma_default(&camera_name),
            saturation,
            contrast,
            sharpness: 1.0,
            focus: 1.0,
            restore_manual_focus: false,
        }
    }

    fn set_profile_values(&self, profile: camera_profile::CameraProfile) {
        let imp = self.imp();
        imp.exposure_scale.set_value(profile.exposure);
        imp.auto_exposure_switch.set_active(profile.auto_exposure);
        imp.shutter_scale.set_value(profile.shutter_us);
        imp.gain_scale.set_value(profile.analogue_gain);
        imp.auto_white_balance_switch
            .set_active(profile.auto_white_balance);
        imp.red_gain_scale.set_value(profile.red_gain);
        imp.blue_gain_scale.set_value(profile.blue_gain);
        let was_custom = imp
            .custom_colour_matrix
            .replace(profile.custom_colour_matrix);
        imp.colour_matrix.set(profile.colour_matrix);
        if was_custom && !profile.custom_colour_matrix {
            imp.colour_matrix_reset_pending.set(true);
        }
        let was_suppressing = imp.suppress_colour_preset.replace(true);
        imp.saturation_scale.set_value(profile.saturation);
        imp.contrast_scale.set_value(profile.contrast);
        imp.sharpness_scale.set_value(profile.sharpness);
        imp.gamma_scale.set_value(profile.gamma);
        imp.suppress_colour_preset.set(was_suppressing);
        let suppressed = imp.suppress_manual_focus.replace(true);
        imp.focus_scale.set_value(profile.focus);
        imp.suppress_manual_focus.set(suppressed);
        self.update_manual_exposure_controls();
        self.update_manual_white_balance_controls();
    }

    fn set_colour_preset_selection(&self, selected: u32) {
        let imp = self.imp();
        let was_suppressed = imp.suppress_colour_preset.replace(true);
        imp.colour_preset_dropdown.set_selected(selected);
        imp.suppress_colour_preset.set(was_suppressed);
    }

    fn mark_colour_preset_custom(&self) {
        let imp = self.imp();
        if !imp.suppress_colour_preset.get()
            && imp.colour_preset_dropdown.selected() != COLOUR_PRESET_CUSTOM
        {
            self.set_colour_preset_selection(COLOUR_PRESET_CUSTOM);
        }
    }

    fn apply_colour_preset(&self, selected: u32) {
        let imp = self.imp();
        if imp.suppress_colour_preset.get() || selected == COLOUR_PRESET_CUSTOM {
            return;
        }

        let Some(camera) = imp.viewfinder.camera() else {
            return;
        };
        let camera_name = camera_model_name(&camera);
        let Some((saturation, contrast, sharpness, gamma)) =
            colour_preset_values(&camera_name, selected)
        else {
            return;
        };

        let was_suppressed = imp.suppress_colour_preset.replace(true);
        imp.saturation_scale.set_value(saturation);
        imp.contrast_scale.set_value(contrast);
        imp.sharpness_scale.set_value(sharpness);
        imp.gamma_scale.set_value(gamma);
        imp.suppress_colour_preset.set(was_suppressed);
        self.queue_image_adjustments();
    }

    fn apply_profile(&self, profile: camera_profile::CameraProfile) {
        self.set_profile_values(profile);
        self.set_colour_preset_selection(COLOUR_PRESET_CUSTOM);
        self.queue_image_adjustments();
        self.queue_white_balance();

        let rear_camera = self
            .imp()
            .viewfinder
            .camera()
            .is_some_and(|camera| matches!(camera.location(), aperture::CameraLocation::Back));
        if rear_camera {
            if profile.restore_manual_focus {
                self.imp().viewfinder.set_manual_focus(profile.focus);
            } else {
                self.imp().viewfinder.set_auto_focus();
            }
        }
    }

    pub fn show_calibration(&self) {
        let Some(window) = self.root().and_downcast::<crate::Window>() else {
            return;
        };
        if window.visible_dialog().is_some() {
            return;
        }
        let Some(camera) = self.imp().viewfinder.camera() else {
            window.send_toast(&gettext("No camera is active"));
            return;
        };

        let settings = self.imp().settings().clone();
        let current = self.current_profile();
        let saved = camera_profile::load(&settings, &camera);
        let dialog = crate::CalibrationDialog::new(
            &camera.display_name(),
            &camera_profile::identity(&camera),
            current,
            saved,
        );

        dialog.connect_colour_calibration_changed(glib::clone!(
            #[weak(rename_to = camera_widget)]
            self,
            move |dialog| {
                camera_widget.set_colour_calibration(
                    dialog.custom_colour_matrix(),
                    dialog.colour_matrix(),
                );
                dialog.set_current_profile(camera_widget.current_profile());
                if dialog.custom_colour_matrix() {
                    let auto_white_balance_was_enabled = camera_widget
                        .imp()
                        .auto_white_balance_switch
                        .is_active();
                    if auto_white_balance_was_enabled {
                        camera_widget
                            .imp()
                            .auto_white_balance_switch
                            .set_active(false);
                    }
                    dialog.set_status(if auto_white_balance_was_enabled {
                        "Custom matrix selected; automatic white balance was turned off so it applies to the live preview."
                    } else {
                        "Colour matrix will update the live preview while manual white balance is active."
                    });
                } else {
                    dialog.set_status("Sensor/identity colour processing selected.");
                }
            }
        ));

        dialog.connect_save(glib::clone!(
            #[weak(rename_to = camera_widget)]
            self,
            #[strong]
            camera,
            #[strong]
            settings,
            move |dialog| {
                camera_widget.set_colour_calibration(
                    dialog.custom_colour_matrix(),
                    dialog.colour_matrix(),
                );
                let mut profile = camera_widget.current_profile();
                profile.restore_manual_focus = dialog.restore_manual_focus();
                match camera_profile::save(&settings, &camera, profile) {
                    Ok(()) => {
                        camera_widget.apply_profile(profile);
                        dialog.set_current_profile(profile);
                        dialog.set_saved_profile(Some(profile));
                        dialog.set_status("Saved for this sensor. The profile will be reused when it is selected.");
                    }
                    Err(error) => dialog.set_status(&format!("Could not save profile: {error}")),
                }
            }
        ));
        dialog.connect_apply(glib::clone!(
            #[weak(rename_to = camera_widget)]
            self,
            #[strong]
            camera,
            #[strong]
            settings,
            move |dialog| {
                if let Some(profile) = camera_profile::load(&settings, &camera) {
                    camera_widget.apply_profile(profile);
                    dialog.set_colour_calibration(
                        profile.custom_colour_matrix,
                        profile.colour_matrix,
                    );
                    dialog.set_current_profile(profile);
                    dialog.set_status("Saved profile applied to the active sensor.");
                } else {
                    dialog.set_status("No saved profile is available for this sensor.");
                }
            }
        ));
        dialog.connect_clear(glib::clone!(
            #[weak(rename_to = camera_widget)]
            self,
            #[strong]
            camera,
            #[strong]
            settings,
            move |dialog| {
                match camera_profile::clear(&settings, &camera) {
                    Ok(()) => {
                        let profile = camera_widget.default_profile(&camera);
                        camera_widget.apply_profile(profile);
                        camera_widget.set_colour_preset_selection(COLOUR_PRESET_SENSOR_DEFAULT);
                        dialog.set_colour_calibration(
                            profile.custom_colour_matrix,
                            profile.colour_matrix,
                        );
                        dialog.set_current_profile(profile);
                        dialog.set_saved_profile(None);
                        dialog.set_status(
                            "Profile cleared; built-in defaults and continuous autofocus restored.",
                        );
                    }
                    Err(error) => dialog.set_status(&format!("Could not clear profile: {error}")),
                }
            }
        ));
        dialog.present(Some(&window));
    }

    fn set_colour_calibration(&self, custom: bool, matrix: [f64; 9]) {
        let imp = self.imp();
        let was_custom = imp.custom_colour_matrix.replace(custom);
        imp.colour_matrix
            .set(camera_profile::clamp_colour_matrix(matrix));
        if was_custom && !custom {
            imp.colour_matrix_reset_pending.set(true);
        }
        self.queue_white_balance();
    }

    fn reset_image_controls(&self) {
        let imp = self.imp();
        self.clear_manual_focus_state();
        imp.suppress_manual_focus.set(true);
        if let Some(camera) = imp.viewfinder.camera() {
            self.apply_profile(self.default_profile(&camera));
            self.set_colour_preset_selection(COLOUR_PRESET_SENSOR_DEFAULT);
            imp.suppress_manual_focus.set(false);
            return;
        }

        imp.exposure_scale.set_value(0.0);
        imp.auto_exposure_switch.set_active(true);
        imp.shutter_scale.set_value(8333.0);
        imp.gain_scale.set_value(1.0);
        imp.auto_white_balance_switch.set_active(true);
        imp.red_gain_scale.set_value(1.0);
        imp.blue_gain_scale.set_value(1.0);
        imp.saturation_scale.set_value(1.35);
        imp.contrast_scale.set_value(1.10);
        imp.sharpness_scale.set_value(1.0);
        imp.gamma_scale.set_value(2.2);
        imp.focus_scale.set_value(1.0);
        imp.zoom_scale.set_value(1.0);
        self.set_colour_preset_selection(COLOUR_PRESET_SENSOR_DEFAULT);
        imp.suppress_manual_focus.set(false);
        self.queue_image_adjustments();
        self.queue_white_balance();
    }

    fn enable_auto_focus(&self) {
        self.clear_manual_focus_state();
        self.imp().viewfinder.set_auto_focus();
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

    fn update_hdr_availability(&self) {
        self.imp().hdr_switch.set_sensitive(hdr_helper_available());
    }

    fn set_hdr_controls_sensitive(&self, sensitive: bool) {
        let imp = self.imp();
        let controls: [&gtk::Widget; 20] = [
            imp.exposure_scale.upcast_ref(),
            imp.auto_exposure_switch.upcast_ref(),
            imp.shutter_scale.upcast_ref(),
            imp.gain_scale.upcast_ref(),
            imp.auto_white_balance_switch.upcast_ref(),
            imp.red_gain_scale.upcast_ref(),
            imp.blue_gain_scale.upcast_ref(),
            imp.focus_scale.upcast_ref(),
            imp.saturation_scale.upcast_ref(),
            imp.contrast_scale.upcast_ref(),
            imp.sharpness_scale.upcast_ref(),
            imp.gamma_scale.upcast_ref(),
            imp.colour_preset_dropdown.upcast_ref(),
            imp.zoom_scale.upcast_ref(),
            imp.zoom_reset_button.upcast_ref(),
            imp.reset_image_controls.upcast_ref(),
            imp.auto_focus_button.upcast_ref(),
            imp.calibration_button.upcast_ref(),
            imp.flash_switch.upcast_ref(),
            imp.hdr_switch.upcast_ref(),
        ];
        for control in controls {
            control.set_sensitive(sensitive);
        }
        if sensitive {
            self.update_manual_exposure_controls();
            self.update_manual_white_balance_controls();
            self.update_flash_availability();
            self.update_hdr_availability();
            let rear_camera =
                self.imp().viewfinder.camera().is_some_and(|camera| {
                    matches!(camera.location(), aperture::CameraLocation::Back)
                });
            imp.focus_scale.set_sensitive(rear_camera);
            imp.auto_focus_button.set_sensitive(rear_camera);
        }
    }

    fn handle_hdr_picture_done(&self, gallery: crate::Gallery, file: Option<&gio::File>) {
        let Some(path) = file.and_then(|file| file.path()) else {
            self.abort_hdr_capture(Some("the camera did not return a valid HDR frame"));
            return;
        };

        let capture_more = {
            let mut state = self.imp().hdr_capture.borrow_mut();
            let Some(state) = state.as_mut() else {
                return;
            };
            state.completed.push(path);
            state.completed.len() < HDR_EXPOSURE_OFFSETS.len()
        };

        if capture_more {
            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = obj)]
                self,
                async move {
                    if let Err(error) = obj.capture_next_hdr_frame().await {
                        obj.abort_hdr_capture(Some(&error.to_string()));
                    }
                }
            ));
        } else if let Some(capture) = self.imp().hdr_capture.take() {
            self.finish_hdr_capture(gallery, capture);
        }
    }

    fn finish_hdr_capture(&self, gallery: crate::Gallery, capture: HdrCapture) {
        let output = capture.output.clone();
        let inputs = capture.completed.clone();
        let cleanup_inputs = inputs.clone();
        self.imp().hdr_capture.replace(Some(HdrCapture {
            output: output.clone(),
            inputs: cleanup_inputs,
            exposures: capture.exposures,
            next_index: capture.next_index,
            completed: inputs.clone(),
        }));
        self.apply_image_adjustments();

        let mut arguments = vec![OsString::from("--output"), output.clone().into_os_string()];
        for input in &inputs {
            arguments.push(OsString::from("--input"));
            arguments.push(input.clone().into_os_string());
        }
        let argument_refs = arguments
            .iter()
            .map(OsString::as_os_str)
            .collect::<Vec<_>>();
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        let process = match launcher.spawn(&argument_refs) {
            Ok(process) => process,
            Err(error) => {
                self.imp().hdr_capture.take();
                self.cleanup_hdr_files(&inputs);
                self.cleanup_hdr_output(&output);
                self.finish_hdr_ui(false, &format!("could not start HDR merge: {error}"));
                return;
            }
        };

        let imp = self.imp();
        let generation = imp.hdr_generation.get().wrapping_add(1);
        imp.hdr_generation.set(generation);
        imp.hdr_process.replace(Some(process.clone()));

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            #[strong]
            gallery,
            async move {
                let result = process.wait_check_future().await;
                let imp = obj.imp();
                if imp.hdr_generation.get() != generation {
                    return;
                }
                imp.hdr_process.take();
                imp.hdr_capture.take();
                obj.cleanup_hdr_files(&inputs);
                obj.set_hdr_controls_sensitive(true);
                if let Err(error) = result {
                    obj.cleanup_hdr_output(&output);
                    obj.finish_hdr_ui(false, &format!("HDR merge failed: {error}"));
                    return;
                }

                match std::fs::metadata(&output) {
                    Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                        gallery.add_image(&gio::File::for_path(output));
                        obj.finish_hdr_ui(true, "");
                    }
                    _ => obj.finish_hdr_ui(false, "HDR merge produced no photo"),
                }
            }
        ));
    }

    fn abort_hdr_capture(&self, reason: Option<&str>) {
        let imp = self.imp();
        if imp.hdr_capture.borrow().is_none() && imp.hdr_process.borrow().is_none() {
            return;
        }
        imp.hdr_generation
            .set(imp.hdr_generation.get().wrapping_add(1));
        if let Some(process) = imp.hdr_process.take()
            && !process.has_exited()
        {
            process.force_exit();
        }
        let (inputs, output) = imp
            .hdr_capture
            .take()
            .map(|capture| (capture.inputs, Some(capture.output)))
            .unwrap_or_default();
        self.cleanup_hdr_files(&inputs);
        if let Some(output) = output {
            self.cleanup_hdr_output(&output);
        }
        self.set_hdr_controls_sensitive(true);
        self.apply_image_adjustments();
        if let Some(window) = self.root().and_downcast::<crate::Window>() {
            window.set_shutter_enabled(true);
            if let Some(reason) = reason {
                window.send_toast(&gettext(reason));
            }
        }
    }

    fn finish_hdr_ui(&self, success: bool, message: &str) {
        self.set_hdr_controls_sensitive(true);
        if let Some(window) = self.root().and_downcast::<crate::Window>() {
            window.set_shutter_enabled(true);
            if !success && !message.is_empty() {
                window.send_toast(&gettext(message));
            }
        }
    }

    fn cleanup_hdr_files(&self, paths: &[PathBuf]) {
        for path in paths {
            if let Err(error) = std::fs::remove_file(path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                log::debug!(
                    "Could not remove HDR temporary frame {}: {error}",
                    path.display()
                );
            }
        }
    }

    fn cleanup_hdr_output(&self, path: &Path) {
        let Some(file_name) = path.file_name() else {
            return;
        };
        if let Err(error) = std::fs::remove_file(path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            log::debug!("Could not remove HDR output {}: {error}", path.display());
        }
        let temporary = path.with_file_name(format!(".{}.tmp", file_name.to_string_lossy()));
        if let Err(error) = std::fs::remove_file(&temporary)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            log::debug!(
                "Could not remove HDR temporary output {}: {error}",
                temporary.display()
            );
        }
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
        let rear_camera = matches!(camera.location(), aperture::CameraLocation::Back);
        let saved_profile = camera_profile::load(imp.settings(), camera);
        let profile = saved_profile.unwrap_or_else(|| self.default_profile(camera));

        self.set_profile_values(profile);
        self.set_colour_preset_selection(if saved_profile.is_some() {
            COLOUR_PRESET_CUSTOM
        } else {
            COLOUR_PRESET_SENSOR_DEFAULT
        });
        imp.focus_scale.set_sensitive(rear_camera);
        imp.auto_focus_button.set_sensitive(rear_camera);
        imp.zoom_scale.set_value(1.0);
        self.queue_image_adjustments();
        self.queue_white_balance();

        // Do not disturb the normal startup path for an unprofiled camera.
        // A saved profile explicitly opts into restoring either a manual lens
        // position or continuous autofocus.
        if saved_profile.is_some() && rear_camera {
            if profile.restore_manual_focus {
                imp.viewfinder.set_manual_focus(profile.focus);
            } else {
                imp.viewfinder.set_auto_focus();
            }
        }
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

fn camera_model_name(camera: &aperture::Camera) -> String {
    let mut name = camera.display_name().to_string();
    for key in ["device.product.name", "node.nick", "device.name"] {
        if let Some(value) = camera
            .properties()
            .get(key)
            .and_then(|value| value.get::<String>().ok())
            && !value.is_empty()
            && !name
                .to_ascii_lowercase()
                .contains(&value.to_ascii_lowercase())
        {
            name.push(' ');
            name.push_str(&value);
        }
    }
    name
}

fn image_control_defaults(camera_name: &str) -> (f64, f64) {
    let model = camera_name.to_ascii_lowercase();
    if model.contains("imx371") || model.contains("imx376") || model.contains("imx519") {
        // Keep the application controls aligned with the simple-IPA tuning
        // shipped for all three OnePlus 6T sensors. Older UI defaults here
        // silently overrode that tuning and made photos look washed out.
        (1.10, 1.35)
    } else {
        // A modest generic fallback for other colour sensors.
        (1.10, 1.25)
    }
}

fn image_gamma_default(camera_name: &str) -> f64 {
    let model = camera_name.to_ascii_lowercase();
    if model.contains("imx371") {
        2.0
    } else if model.contains("imx376") {
        2.1
    } else {
        // The IMX519 and generic colour sensors use the neutral standard
        // default unless a calibration profile overrides it.
        2.2
    }
}

/// Returns the safe, userspace colour-processing presets offered by the UI.
///
/// These values intentionally change only the software-ISP tone/detail
/// controls. Exposure, white balance, focus and a measured colour matrix stay
/// under the user's control, so selecting a look cannot silently discard a
/// calibration or make a scene unexpectedly darker.
fn colour_preset_values(camera_name: &str, preset: u32) -> Option<(f64, f64, f64, f64)> {
    let (_, sensor_saturation) = image_control_defaults(camera_name);
    let sensor_gamma = image_gamma_default(camera_name);

    match preset {
        COLOUR_PRESET_SENSOR_DEFAULT => {
            let (contrast, saturation) = image_control_defaults(camera_name);
            Some((saturation, contrast, 1.0, sensor_gamma))
        }
        COLOUR_PRESET_NEUTRAL => Some((1.0, 1.0, 1.0, sensor_gamma)),
        COLOUR_PRESET_NATURAL => Some((sensor_saturation.min(1.35), 1.05, 1.05, sensor_gamma)),
        COLOUR_PRESET_VIVID => Some((1.55, 1.15, 1.10, sensor_gamma)),
        _ => None,
    }
}

fn is_focus_control(widget: &gtk::Widget) -> bool {
    let mut current = Some(widget.clone());
    while let Some(widget) = current {
        if widget.is::<gtk::Button>()
            || widget.is::<gtk::MenuButton>()
            || widget.is::<gtk::Scale>()
            || widget.is::<gtk::Switch>()
            || widget.is::<gtk::DropDown>()
            || widget.is::<gtk::ToggleButton>()
        {
            return true;
        }
        current = widget.parent();
    }
    false
}

fn flash_helper_available() -> bool {
    if FLASH_HELPER.contains('/') {
        Path::new(FLASH_HELPER).is_file()
    } else {
        glib::find_program_in_path(FLASH_HELPER).is_some()
    }
}

fn hdr_helper_available() -> bool {
    if HDR_HELPER.contains('/') {
        Path::new(HDR_HELPER).is_file()
    } else {
        glib::find_program_in_path(HDR_HELPER).is_some()
    }
}

fn hdr_exposure_values(base: f64) -> [f64; 3] {
    let base = base.clamp(-1.0, 1.0);
    let low = (base + HDR_EXPOSURE_OFFSETS[0]).clamp(-1.0, 1.0);
    let high = (base + HDR_EXPOSURE_OFFSETS[2]).clamp(-1.0, 1.0);

    // At either UI endpoint clamping would collapse two brackets. Preserve a
    // useful three-stop sequence in that case and restore the user's EV after
    // the sequence has completed.
    if low >= base || high <= base || high - low < 0.75 {
        [-1.0, 0.0, 1.0]
    } else {
        [low, base, high]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        COLOUR_PRESET_NATURAL, COLOUR_PRESET_NEUTRAL, COLOUR_PRESET_SENSOR_DEFAULT,
        COLOUR_PRESET_VIVID, colour_preset_values, format_zoom_label, hdr_exposure_values,
        image_control_defaults, image_gamma_default, pinch_zoom_value,
    };

    #[test]
    fn sensor_defaults_are_selected_for_all_phone_cameras() {
        assert_eq!(image_control_defaults("Sony IMX371 Main"), (1.10, 1.35));
        assert_eq!(image_control_defaults("Sony IMX376 Front"), (1.10, 1.35));
        assert_eq!(image_control_defaults("Sony IMX519 Wide"), (1.10, 1.35));
    }

    #[test]
    fn unknown_camera_uses_conservative_defaults() {
        assert_eq!(image_control_defaults("USB Webcam"), (1.10, 1.25));
    }

    #[test]
    fn sensor_gamma_defaults_follow_the_phone_tuning() {
        assert_eq!(image_gamma_default("Built-in Front Camera imx371"), 2.0);
        assert_eq!(image_gamma_default("Built-in Back Camera imx376"), 2.1);
        assert_eq!(image_gamma_default("Built-in Back Camera imx519"), 2.2);
        assert_eq!(image_gamma_default("USB Webcam"), 2.2);
    }

    #[test]
    fn colour_presets_keep_sensor_gamma_and_only_change_processing() {
        assert_eq!(
            colour_preset_values("Sony IMX371 Main", COLOUR_PRESET_SENSOR_DEFAULT),
            Some((1.35, 1.10, 1.0, 2.0))
        );
        assert_eq!(
            colour_preset_values("Sony IMX371 Main", COLOUR_PRESET_NEUTRAL),
            Some((1.0, 1.0, 1.0, 2.0))
        );
        assert_eq!(
            colour_preset_values("Sony IMX371 Main", COLOUR_PRESET_NATURAL),
            Some((1.35, 1.05, 1.05, 2.0))
        );
        assert_eq!(
            colour_preset_values("Sony IMX371 Main", COLOUR_PRESET_VIVID),
            Some((1.55, 1.15, 1.10, 2.0))
        );
    }

    #[test]
    fn custom_colour_preset_is_not_a_preset_request() {
        assert_eq!(colour_preset_values("Sony IMX519", 4), None);
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

    #[test]
    fn hdr_brackets_are_symmetric_around_the_requested_ev() {
        assert_eq!(hdr_exposure_values(0.0), [-1.0, 0.0, 1.0]);
        assert_eq!(hdr_exposure_values(0.25), [-0.75, 0.25, 1.0]);
    }

    #[test]
    fn hdr_brackets_do_not_collapse_at_ev_endpoints() {
        assert_eq!(hdr_exposure_values(-1.0), [-1.0, 0.0, 1.0]);
        assert_eq!(hdr_exposure_values(1.0), [-1.0, 0.0, 1.0]);
    }
}
