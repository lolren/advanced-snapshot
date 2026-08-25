// SPDX-License-Identifier: GPL-3.0-or-later
use std::os::unix::io::OwnedFd;
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
        pub pinch_zoom_start: Cell<f64>,

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
            self.zoom_scale.connect_value_changed(glib::clone!(
                #[weak]
                obj,
                move |scale| {
                    let zoom = scale.value();
                    obj.imp().viewfinder.set_zoom(zoom);
                    obj.imp()
                        .zoom_reset_button
                        .set_label(&format_zoom_label(zoom));
                }
            ));
            self.zoom_reset_button.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.imp().zoom_scale.set_value(1.0)
            ));

            let zoom_gesture = gtk::GestureZoom::new();
            zoom_gesture.connect_begin(glib::clone!(
                #[weak]
                obj,
                move |gesture, _| {
                    obj.imp().pinch_zoom_start.set(obj.imp().zoom_scale.value());
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
            self.viewfinder.add_controller(zoom_gesture);

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
        let format = imp.viewfinder.video_format();
        let filename = utils::video_file_name(format);
        let path = utils::videos_dir()?.join(filename);

        imp.viewfinder.start_recording(path)?;

        Ok(())
    }

    pub fn stop_recording(&self) {
        let imp = self.imp();
        if matches!(imp.viewfinder.state(), aperture::ViewfinderState::Ready)
            && imp.viewfinder.is_recording()
            && let Err(err) = imp.viewfinder.stop_recording()
        {
            log::error!("Could not stop camera: {err}");
        }
    }

    pub async fn take_picture(&self, format: crate::PictureFormat) -> anyhow::Result<()> {
        let imp = self.imp();
        let window = self.root().and_downcast::<crate::Window>().unwrap();

        // We enable the shutter whenever picture-stored is emitted.
        window.set_shutter_enabled(false);

        let filename = utils::picture_file_name(format);
        let path = utils::pictures_dir()?.join(filename);

        imp.viewfinder.take_picture(path)?;
        imp.flash_bin.flash();

        let settings = imp.settings();
        if settings.boolean("play-shutter-sound") {
            self.play_shutter_sound();
        }

        Ok(())
    }

    fn camera_switched(&self) {
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
            self.set_image_control_defaults(camera);
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
                let window = obj.root().and_downcast::<crate::Window>().unwrap();
                window.set_shutter_enabled(true);
                // TODO Maybe report error via toast on None
                if let Some(file) = file {
                    gallery.add_image(file);
                }
            }
        ));
        imp.viewfinder.connect_recording_done(glib::clone!(
            #[weak]
            gallery,
            move |_, file| {
                if let Some(file) = file {
                    gallery.add_video(file);
                } else {
                    log::error!("Didn't find any file when recording finished!");
                }
            }
        ));
        imp.camera_controls.set_gallery(&gallery);
    }

    pub fn stop_stream(&self) {
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
        imp.saturation_scale.set_value(1.25);
        imp.contrast_scale.set_value(1.05);
        imp.sharpness_scale.set_value(1.0);
        imp.zoom_scale.set_value(1.0);
        self.queue_image_adjustments();
    }

    fn set_image_control_defaults(&self, camera: &aperture::Camera) {
        let imp = self.imp();
        let model = camera.display_name().to_ascii_lowercase();
        let (contrast, saturation) = if model.contains("imx371") {
            (1.10, 1.25)
        } else if model.contains("imx376") {
            (1.05, 1.15)
        } else {
            // IMX519, and a conservative fallback for other colour sensors.
            (1.05, 1.25)
        };

        imp.exposure_scale.set_value(0.0);
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

#[cfg(test)]
mod tests {
    use super::{format_zoom_label, pinch_zoom_value};

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
