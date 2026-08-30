// SPDX-License-Identifier: GPL-3.0-or-later
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::glib;
use std::rc::Rc;

use crate::camera_profile::{self, CameraProfile};

const COLOUR_BOOST_CCM: [f64; 9] = [1.10, -0.05, -0.05, -0.05, 1.10, -0.05, -0.05, -0.05, 1.10];

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
        pub custom_colour_matrix: TemplateChild<gtk::Switch>,
        #[template_child]
        pub ccm_red_row: TemplateChild<gtk::Box>,
        #[template_child]
        pub ccm_green_row: TemplateChild<gtk::Box>,
        #[template_child]
        pub ccm_blue_row: TemplateChild<gtk::Box>,
        #[template_child]
        pub ccm_00: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub ccm_01: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub ccm_02: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub ccm_10: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub ccm_11: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub ccm_12: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub ccm_20: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub ccm_21: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub ccm_22: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub identity_matrix_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub colour_boost_matrix_button: TemplateChild<gtk::Button>,
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
        dialog.set_colour_calibration(current.custom_colour_matrix, current.colour_matrix);
        imp.custom_colour_matrix.connect_active_notify(glib::clone!(
            #[weak(rename_to = dialog)]
            dialog,
            move |_| dialog.update_matrix_sensitivity()
        ));
        imp.identity_matrix_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            dialog,
            move |_| dialog.set_colour_calibration(true, camera_profile::IDENTITY_CCM)
        ));
        imp.colour_boost_matrix_button.connect_clicked(glib::clone!(
            #[weak(rename_to = dialog)]
            dialog,
            move |_| dialog.set_colour_calibration(true, COLOUR_BOOST_CCM)
        ));
        dialog.update_matrix_sensitivity();
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

    pub fn custom_colour_matrix(&self) -> bool {
        self.imp().custom_colour_matrix.is_active()
    }

    pub fn colour_matrix(&self) -> [f64; 9] {
        let inputs = self.matrix_inputs();
        camera_profile::clamp_colour_matrix(std::array::from_fn(|index| inputs[index].value()))
    }

    pub fn connect_colour_calibration_changed<F: Fn(&Self) + 'static>(&self, callback: F) {
        let callback = Rc::new(callback);
        self.imp()
            .custom_colour_matrix
            .connect_active_notify(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                #[strong]
                callback,
                move |_| callback(&dialog)
            ));
        for input in self.matrix_inputs() {
            input.connect_value_changed(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                #[strong]
                callback,
                move |_| callback(&dialog)
            ));
        }
    }

    pub fn set_colour_calibration(&self, custom: bool, matrix: [f64; 9]) {
        let matrix = camera_profile::clamp_colour_matrix(matrix);
        self.imp().custom_colour_matrix.set_active(custom);
        for (input, value) in self.matrix_inputs().into_iter().zip(matrix) {
            input.set_value(value);
        }
        self.update_matrix_sensitivity();
    }

    pub fn set_current_profile(&self, profile: CameraProfile) {
        self.imp().current_label.set_label(&format_profile(profile));
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

    fn matrix_inputs(&self) -> [&gtk::SpinButton; 9] {
        let imp = self.imp();
        [
            &imp.ccm_00,
            &imp.ccm_01,
            &imp.ccm_02,
            &imp.ccm_10,
            &imp.ccm_11,
            &imp.ccm_12,
            &imp.ccm_20,
            &imp.ccm_21,
            &imp.ccm_22,
        ]
    }

    fn update_matrix_sensitivity(&self) {
        let imp = self.imp();
        let enabled = imp.custom_colour_matrix.is_active();
        imp.ccm_red_row.set_sensitive(enabled);
        imp.ccm_green_row.set_sensitive(enabled);
        imp.ccm_blue_row.set_sensitive(enabled);
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
    let colour_matrix = if profile.custom_colour_matrix {
        "CCM custom"
    } else {
        "CCM sensor/identity"
    };

    format!(
        "EV {:+.1} · {exposure_mode} · {white_balance} · {colour_matrix} · gamma {:.1} · colour {:.2} · contrast {:.2} · detail {:.2} · focus {focus_mode}",
        profile.exposure, profile.gamma, profile.saturation, profile.contrast, profile.sharpness,
    )
}
