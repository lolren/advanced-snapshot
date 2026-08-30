// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::glib;

use crate::camera_profile::CameraProfile;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/lolren/AdvancedSnapshot/ui/calibration.ui")]
    pub struct CalibrationDialog {
        #[template_child]
        pub camera_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub identity_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub current_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub saved_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub restore_manual_focus: TemplateChild<gtk::Switch>,
        #[template_child]
        pub apply_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub save_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub clear_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CalibrationDialog {
        const NAME: &'static str = "CalibrationDialog";
        type Type = super::CalibrationDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CalibrationDialog {}
    impl WidgetImpl for CalibrationDialog {}
    impl AdwDialogImpl for CalibrationDialog {}
}

glib::wrapper! {
    pub struct CalibrationDialog(ObjectSubclass<imp::CalibrationDialog>)
        @extends gtk::Widget, adw::Dialog,
        @implements gtk::ConstraintTarget, gtk::Buildable, gtk::Accessible;
}

impl CalibrationDialog {
    pub fn new(
        camera_name: &str,
        camera_identity: &str,
        current: CameraProfile,
        saved: Option<CameraProfile>,
    ) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();

        imp.camera_label.set_label(camera_name);
        imp.identity_label.set_label(camera_identity);
        imp.current_label.set_label(&format_profile(current));
        imp.restore_manual_focus
            .set_active(current.restore_manual_focus);
        dialog.set_saved_profile(saved);
        dialog
    }

    pub fn connect_save<F: Fn(&Self) + 'static>(&self, callback: F) {
        self.imp().save_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| callback(&dialog)
        ));
    }

    pub fn connect_apply<F: Fn(&Self) + 'static>(&self, callback: F) {
        self.imp().apply_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| callback(&dialog)
        ));
    }

    pub fn connect_clear<F: Fn(&Self) + 'static>(&self, callback: F) {
        self.imp().clear_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            self,
            move |_| callback(&dialog)
        ));
    }

    pub fn restore_manual_focus(&self) -> bool {
        self.imp().restore_manual_focus.is_active()
    }

    pub fn set_saved_profile(&self, profile: Option<CameraProfile>) {
        let imp = self.imp();
        match profile {
            Some(profile) => {
                imp.saved_label.set_label(&format_profile(profile));
                imp.apply_button.set_sensitive(true);
                imp.clear_button.set_sensitive(true);
            }
            None => {
                imp.saved_label
                    .set_label("No saved profile for this sensor");
                imp.apply_button.set_sensitive(false);
                imp.clear_button.set_sensitive(false);
            }
        }
    }

    pub fn set_status(&self, status: &str) {
        self.imp().status_label.set_label(status);
    }
}

fn format_profile(profile: CameraProfile) -> String {
    let exposure_mode = if profile.auto_exposure {
        "auto"
    } else {
        "manual"
    };
    let focus_mode = if profile.restore_manual_focus {
        format!("manual {:.2}", profile.focus)
    } else {
        "continuous auto".to_string()
    };
    let white_balance = if profile.auto_white_balance {
        "WB auto".to_string()
    } else {
        format!("WB R {:.2} / B {:.2}", profile.red_gain, profile.blue_gain)
    };

    format!(
        "EV {:+.1} · {exposure_mode} · {white_balance} · gamma {:.1} · colour {:.2} · contrast {:.2} · detail {:.2} · focus {focus_mode}",
        profile.exposure, profile.gamma, profile.saturation, profile.contrast, profile.sharpness,
    )
}
