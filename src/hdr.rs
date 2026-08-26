// SPDX-License-Identifier: GPL-3.0-or-later
//! Small, bounded software exposure-fusion HDR implementation.
//!
//! The camera pipeline supplies three JPEGs captured at -1, 0 and +1 EV.
//! This module converts the decoded sRGB samples to a linear working space,
//! estimates the scene value from each exposure, rejects clipped samples and
//! applies a conservative global tone map. It deliberately does not claim
//! motion alignment, local tone mapping, lens shading or vendor ISP parity.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use gdk_pixbuf::{Colorspace, Pixbuf};

const EXPECTED_FRAMES: usize = 3;
const MAX_PIXELS: usize = 40_000_000;
const EXPOSURE_SCALES: [f32; EXPECTED_FRAMES] = [0.5, 1.0, 2.0];

#[derive(Debug, Clone, PartialEq, Eq)]
struct RgbFrame {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl RgbFrame {
    fn new(width: usize, height: usize, pixels: Vec<u8>) -> anyhow::Result<Self> {
        if width == 0 || height == 0 {
            bail!("HDR frame has an empty dimension");
        }
        let pixel_count = width
            .checked_mul(height)
            .context("HDR frame dimensions overflow")?;
        if pixel_count > MAX_PIXELS {
            bail!("HDR frame is too large ({pixel_count} pixels)");
        }
        let expected_bytes = pixel_count
            .checked_mul(3)
            .context("HDR frame byte count overflow")?;
        if pixels.len() != expected_bytes {
            bail!(
                "HDR frame has {} bytes, expected {expected_bytes}",
                pixels.len()
            );
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }
}

fn load_rgb_frame(path: &Path) -> anyhow::Result<RgbFrame> {
    let pixbuf = Pixbuf::from_file(path)
        .with_context(|| format!("could not decode HDR input {}", path.display()))?;
    if pixbuf.bits_per_sample() != 8 {
        bail!("HDR input {} is not an 8-bit image", path.display());
    }
    if pixbuf.n_channels() < 3 {
        bail!("HDR input {} has no RGB channels", path.display());
    }

    let width = usize::try_from(pixbuf.width()).context("HDR width is negative")?;
    let height = usize::try_from(pixbuf.height()).context("HDR height is negative")?;
    let channels = usize::try_from(pixbuf.n_channels()).context("HDR channel count is negative")?;
    let rowstride = usize::try_from(pixbuf.rowstride()).context("HDR rowstride is negative")?;
    let pixel_count = width
        .checked_mul(height)
        .context("HDR frame dimensions overflow")?;
    if pixel_count > MAX_PIXELS {
        bail!("HDR frame is too large ({pixel_count} pixels)");
    }
    let byte_count = pixel_count
        .checked_mul(3)
        .context("HDR frame byte count overflow")?;
    let mut pixels = vec![0u8; byte_count];

    // GdkPixbuf owns the decoded buffer. Copy it into a tightly packed RGB
    // buffer before any work is handed to the pure merge path.
    let source = unsafe { pixbuf.pixels() };
    for y in 0..height {
        let source_row = &source[y * rowstride..];
        for x in 0..width {
            let source_offset = x * channels;
            let output_offset = (y * width + x) * 3;
            pixels[output_offset..output_offset + 3]
                .copy_from_slice(&source_row[source_offset..source_offset + 3]);
        }
    }

    RgbFrame::new(width, height, pixels)
}

fn srgb_to_linear(value: u8) -> f32 {
    let value = f32::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round().clamp(0.0, 255.0) as u8
}

fn merge_rgb_frames(frames: &[RgbFrame]) -> anyhow::Result<RgbFrame> {
    if frames.len() != EXPECTED_FRAMES {
        bail!(
            "HDR requires {EXPECTED_FRAMES} frames, received {}",
            frames.len()
        );
    }
    let first = &frames[0];
    if frames
        .iter()
        .any(|frame| frame.width != first.width || frame.height != first.height)
    {
        bail!("HDR frames do not have identical dimensions");
    }

    let mut output = vec![0u8; first.pixels.len()];
    for pixel in 0..(first.width * first.height) {
        let offset = pixel * 3;
        let mut weighted = [0.0f32; 3];
        let mut total_weight = 0.0f32;

        for (frame_index, frame) in frames.iter().enumerate() {
            let red = srgb_to_linear(frame.pixels[offset]);
            let green = srgb_to_linear(frame.pixels[offset + 1]);
            let blue = srgb_to_linear(frame.pixels[offset + 2]);
            let luminance = 0.2126 * red + 0.7152 * green + 0.0722 * blue;

            // Prefer middle tones and avoid samples that are close to either
            // sensor/codec endpoint. The fourth-power curve makes the merge
            // naturally use the bright frame for shadows and the dark frame
            // for highlights without hard seams.
            let normalized = (luminance / EXPOSURE_SCALES[frame_index]).clamp(0.0, 1.0);
            let distance = ((normalized - 0.42) / 0.42).abs();
            let midtone_weight = (1.0 - distance * distance).max(0.0).powi(2);
            let endpoint_weight = if luminance <= 0.002 || luminance >= 0.985 {
                0.0
            } else {
                1.0
            };
            let weight = midtone_weight * endpoint_weight;

            weighted[0] += red / EXPOSURE_SCALES[frame_index] * weight;
            weighted[1] += green / EXPOSURE_SCALES[frame_index] * weight;
            weighted[2] += blue / EXPOSURE_SCALES[frame_index] * weight;
            total_weight += weight;
        }

        if total_weight <= f32::EPSILON {
            // An all-black/all-clipped pixel has no trustworthy exposure.
            // Keep the middle exposure as a deterministic fallback.
            for (channel, value) in weighted.iter_mut().enumerate() {
                *value = srgb_to_linear(frames[1].pixels[offset + channel]);
            }
            total_weight = 1.0;
        }

        for channel in 0..3 {
            let scene_value = weighted[channel] / total_weight;
            // A global Reinhard curve keeps the output in displayable range
            // while retaining highlight detail from the dark bracket.
            let tone_mapped = (scene_value * 1.25) / (1.0 + scene_value * 1.25);
            output[offset + channel] = linear_to_srgb(tone_mapped);
        }
    }

    RgbFrame::new(first.width, first.height, output)
}

fn save_rgb_frame(frame: RgbFrame, path: &Path) -> anyhow::Result<()> {
    let width = i32::try_from(frame.width).context("HDR output width is too large")?;
    let height = i32::try_from(frame.height).context("HDR output height is too large")?;
    let rowstride = i32::try_from(
        frame
            .width
            .checked_mul(3)
            .context("HDR rowstride overflow")?,
    )
    .context("HDR output rowstride is too large")?;
    let pixbuf = Pixbuf::from_mut_slice(
        frame.pixels,
        Colorspace::Rgb,
        false,
        8,
        width,
        height,
        rowstride,
    );
    pixbuf
        .savev(path, "jpeg", &[("quality", "95")])
        .with_context(|| format!("could not write HDR output {}", path.display()))
}

/// Merge exactly three same-sized JPEG-compatible images into `output`.
pub fn merge_hdr_files(inputs: &[PathBuf], output: &Path) -> anyhow::Result<()> {
    if inputs.len() != EXPECTED_FRAMES {
        bail!(
            "HDR requires {EXPECTED_FRAMES} input files, received {}",
            inputs.len()
        );
    }
    if inputs.iter().any(|input| input == output) {
        bail!("HDR output must not overwrite an input frame");
    }

    let frames = inputs
        .iter()
        .map(|path| load_rgb_frame(path))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let merged = merge_rgb_frames(&frames)?;

    let file_name = output
        .file_name()
        .context("HDR output has no file name")?
        .to_string_lossy();
    let temporary = output.with_file_name(format!(".{file_name}.tmp"));
    if let Err(error) = save_rgb_frame(merged, &temporary) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error).with_context(|| {
            format!(
                "could not atomically install HDR output {}",
                output.display()
            )
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::RgbFrame;
    use super::linear_to_srgb;
    use super::load_rgb_frame;
    use super::merge_rgb_frames;
    use super::save_rgb_frame;
    use super::srgb_to_linear;

    fn frame(value: u8) -> RgbFrame {
        RgbFrame::new(1, 1, vec![value; 3]).unwrap()
    }

    #[test]
    fn transfer_functions_round_trip_midtones() {
        for value in [0, 1, 32, 96, 128, 200, 254, 255] {
            let round_trip = linear_to_srgb(srgb_to_linear(value));
            assert!((i16::from(round_trip) - i16::from(value)).abs() <= 1);
        }
    }

    #[test]
    fn merge_requires_three_equal_sized_frames() {
        assert!(merge_rgb_frames(&[frame(10), frame(20)]).is_err());
        let different = RgbFrame::new(2, 1, vec![20; 6]).unwrap();
        assert!(merge_rgb_frames(&[frame(10), frame(20), different]).is_err());
    }

    #[test]
    fn merge_keeps_a_valid_middle_tone() {
        let result = merge_rgb_frames(&[frame(32), frame(96), frame(180)]).unwrap();
        assert_eq!(result.pixels.len(), 3);
        assert!(result.pixels[0] > 40);
        assert!(result.pixels[0] < 220);
    }

    #[test]
    fn merge_has_a_deterministic_black_fallback() {
        let result = merge_rgb_frames(&[frame(0), frame(0), frame(0)]).unwrap();
        assert_eq!(result.pixels, vec![0, 0, 0]);
    }

    #[test]
    fn file_merge_decodes_and_atomically_writes_jpeg() {
        let root = std::env::temp_dir().join(format!(
            "advanced-snapshot-hdr-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let inputs = [
            root.join("dark.jpg"),
            root.join("normal.jpg"),
            root.join("bright.jpg"),
        ];
        for (path, value) in inputs.iter().zip([32, 96, 180]) {
            save_rgb_frame(frame(value), path).unwrap();
        }
        let output = root.join("merged.jpg");

        super::merge_hdr_files(&inputs, &output).unwrap();
        let decoded = load_rgb_frame(&output).unwrap();
        assert_eq!((decoded.width, decoded.height), (1, 1));
        assert!(decoded.pixels[0] > 40 && decoded.pixels[0] < 220);
        assert!(!root.join(".merged.jpg.tmp").exists());

        std::fs::remove_dir_all(root).unwrap();
    }
}
