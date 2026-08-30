// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistent, per-camera calibration profiles.
//!
//! The profile deliberately stores only controls that are exposed through the
//! standard libcamera/PipeWire interface.  It is not a replacement for the
//! vendor's factory colour matrix or lens-shading tables.

use gtk::{gio, gio::prelude::SettingsExt, glib};

const PROFILES_KEY: &str = "camera-calibration-profiles";
const PROFILE_GROUP_PREFIX: &str = "camera-";

/// The values controlled by the Advanced Snapshot image-controls panel.
///
/// Values are validated and clamped when loaded so a hand-edited or damaged
/// settings value cannot send an unsafe request to a camera node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraProfile {
    pub exposure: f64,
    pub auto_exposure: bool,
    pub shutter_us: f64,
    pub analogue_gain: f64,
    pub gamma: f64,
    pub saturation: f64,
    pub contrast: f64,
    pub sharpness: f64,
    pub focus: f64,
    /// If true, restore `focus` as a manual lens position.  The default is
    /// false so every camera starts in continuous autofocus.
    pub restore_manual_focus: bool,
}

impl CameraProfile {
    pub fn clamped(self) -> Self {
        Self {
            exposure: clamp_finite(self.exposure, 0.0, -1.0, 1.0),
            auto_exposure: self.auto_exposure,
            shutter_us: clamp_finite(self.shutter_us, 8333.0, 1.0, 2_000_000.0),
            analogue_gain: clamp_finite(self.analogue_gain, 1.0, 0.1, 256.0),
            gamma: clamp_finite(self.gamma, 2.2, 0.1, 10.0),
            saturation: clamp_finite(self.saturation, 1.0, 0.0, 2.0),
            contrast: clamp_finite(self.contrast, 1.0, 0.0, 2.0),
            sharpness: clamp_finite(self.sharpness, 1.0, 0.0, 2.0),
            focus: clamp_finite(self.focus, 1.0, 0.0, 2.0),
            restore_manual_focus: self.restore_manual_focus,
        }
    }
}

fn clamp_finite(value: f64, fallback: f64, lower: f64, upper: f64) -> f64 {
    if value.is_finite() {
        value.clamp(lower, upper)
    } else {
        fallback
    }
}

fn profile_data(settings: &gio::Settings) -> glib::KeyFile {
    let file = glib::KeyFile::new();
    let data = settings.string(PROFILES_KEY);
    if !data.is_empty()
        && let Err(error) = file.load_from_data(data.as_str(), glib::KeyFileFlags::NONE)
    {
        log::warn!("Ignoring invalid camera calibration profiles: {error}");
    }
    file
}

fn read_double(
    file: &glib::KeyFile,
    group: &str,
    key: &str,
    fallback: f64,
    lower: f64,
    upper: f64,
) -> f64 {
    file.double(group, key)
        .ok()
        .map(|value| clamp_finite(value, fallback, lower, upper))
        .unwrap_or(fallback)
}

fn read_bool(file: &glib::KeyFile, group: &str, key: &str, fallback: bool) -> bool {
    file.boolean(group, key).unwrap_or(fallback)
}

fn camera_identity(camera: &aperture::Camera) -> String {
    let properties = camera.properties();
    for key in ["node.name", "api.libcamera.path", "device.name"] {
        if let Some(value) = properties
            .get(key)
            .and_then(|value| value.get::<String>().ok())
            && !value.is_empty()
        {
            return value;
        }
    }

    format!("{}:{:?}", camera.display_name(), camera.location())
}

/// A stable, human-readable identifier used as the key-file group name.
///
/// PipeWire object serials are intentionally not used: they can change after
/// a service restart.  The libcamera node name/path is stable for a physical
/// sensor on the OnePlus 6T and remains useful for other devices too.
pub fn profile_group(camera: &aperture::Camera) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in camera_identity(camera).as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{PROFILE_GROUP_PREFIX}{hash:016x}")
}

/// Returns the identity used for the current sensor in the calibration UI.
pub fn identity(camera: &aperture::Camera) -> String {
    camera_identity(camera)
}

pub fn load(settings: &gio::Settings, camera: &aperture::Camera) -> Option<CameraProfile> {
    let file = profile_data(settings);
    let group = profile_group(camera);
    if !file.has_group(&group) {
        return None;
    }

    Some(
        CameraProfile {
            exposure: read_double(&file, &group, "exposure", 0.0, -1.0, 1.0),
            auto_exposure: read_bool(&file, &group, "auto-exposure", true),
            shutter_us: read_double(&file, &group, "shutter-us", 8333.0, 1.0, 2_000_000.0),
            analogue_gain: read_double(&file, &group, "analogue-gain", 1.0, 0.1, 256.0),
            gamma: read_double(&file, &group, "gamma", 2.2, 0.1, 10.0),
            saturation: read_double(&file, &group, "saturation", 1.0, 0.0, 2.0),
            contrast: read_double(&file, &group, "contrast", 1.0, 0.0, 2.0),
            sharpness: read_double(&file, &group, "sharpness", 1.0, 0.0, 2.0),
            focus: read_double(&file, &group, "focus", 1.0, 0.0, 2.0),
            restore_manual_focus: read_bool(&file, &group, "restore-manual-focus", false),
        }
        .clamped(),
    )
}

pub fn save(
    settings: &gio::Settings,
    camera: &aperture::Camera,
    profile: CameraProfile,
) -> Result<(), glib::BoolError> {
    let profile = profile.clamped();
    let file = profile_data(settings);
    let group = profile_group(camera);

    file.set_integer(&group, "version", 1);
    file.set_string(&group, "camera-identity", &camera_identity(camera));
    file.set_double(&group, "exposure", profile.exposure);
    file.set_boolean(&group, "auto-exposure", profile.auto_exposure);
    file.set_double(&group, "shutter-us", profile.shutter_us);
    file.set_double(&group, "analogue-gain", profile.analogue_gain);
    file.set_double(&group, "gamma", profile.gamma);
    file.set_double(&group, "saturation", profile.saturation);
    file.set_double(&group, "contrast", profile.contrast);
    file.set_double(&group, "sharpness", profile.sharpness);
    file.set_double(&group, "focus", profile.focus);
    file.set_boolean(&group, "restore-manual-focus", profile.restore_manual_focus);

    settings.set_string(PROFILES_KEY, file.to_data().as_str())
}

pub fn clear(settings: &gio::Settings, camera: &aperture::Camera) -> Result<(), glib::BoolError> {
    let file = profile_data(settings);
    let group = profile_group(camera);
    if file.has_group(&group) {
        file.remove_group(&group)
            .map_err(|error| glib::bool_error!("Could not remove camera profile: {error}"))?;
        settings.set_string(PROFILES_KEY, file.to_data().as_str())?;
    }
    Ok(())
}
