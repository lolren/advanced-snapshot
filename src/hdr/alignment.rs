// SPDX-License-Identifier: GPL-3.0-or-later
//! Conservative global translation alignment for bracketed HDR frames.
//!
//! The implementation deliberately handles only small handheld camera motion.
//! It compares log-luminance gradients, which makes the score largely
//! invariant to the exposure difference between bracketed frames. A coarse
//! bounded thumbnail search is refined against sparse full-resolution samples.
//! If the best translation does not improve the zero-motion score enough, the
//! frame is left untouched.

const THUMBNAIL_MAX_DIMENSION: usize = 512;
const SCORE_GRID_DIMENSION: usize = 128;
const MAX_TRANSLATION_PIXELS: usize = 96;
const MIN_VALID_SAMPLES: usize = 16;
const MIN_RELATIVE_IMPROVEMENT: f32 = 0.06;
const BLACK_ENDPOINT: f32 = 0.004;
const WHITE_ENDPOINT: f32 = 0.985;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Translation {
    pub(crate) x: isize,
    pub(crate) y: isize,
}

#[derive(Debug)]
struct GradientMap {
    width: usize,
    height: usize,
    scale: usize,
    horizontal: Vec<f32>,
    vertical: Vec<f32>,
    valid: Vec<bool>,
}

impl GradientMap {
    fn from_rgb(width: usize, height: usize, pixels: &[u8], lut: &[f32; 256]) -> Self {
        let scale = width.max(height).div_ceil(THUMBNAIL_MAX_DIMENSION).max(1);
        let thumbnail_width = width.div_ceil(scale);
        let thumbnail_height = height.div_ceil(scale);
        let sample_count = thumbnail_width * thumbnail_height;
        let mut luminance = vec![0.0f32; sample_count];
        let mut usable = vec![false; sample_count];

        for y in 0..thumbnail_height {
            let source_y = (y * scale + scale / 2).min(height - 1);
            for x in 0..thumbnail_width {
                let source_x = (x * scale + scale / 2).min(width - 1);
                let value = pixel_luminance(pixels, width, source_x, source_y, lut);
                let index = y * thumbnail_width + x;
                luminance[index] = value.max(f32::MIN_POSITIVE).ln();
                usable[index] = is_usable_luminance(value);
            }
        }

        let mut horizontal = vec![0.0f32; sample_count];
        let mut vertical = vec![0.0f32; sample_count];
        let mut valid = vec![false; sample_count];
        if thumbnail_width >= 3 && thumbnail_height >= 3 {
            for y in 1..thumbnail_height - 1 {
                for x in 1..thumbnail_width - 1 {
                    let index = y * thumbnail_width + x;
                    let left = index - 1;
                    let right = index + 1;
                    let above = index - thumbnail_width;
                    let below = index + thumbnail_width;
                    if usable[left] && usable[right] && usable[above] && usable[below] {
                        horizontal[index] = luminance[right] - luminance[left];
                        vertical[index] = luminance[below] - luminance[above];
                        valid[index] = true;
                    }
                }
            }
        }

        Self {
            width: thumbnail_width,
            height: thumbnail_height,
            scale,
            horizontal,
            vertical,
            valid,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct GradientSample {
    x: usize,
    y: usize,
    horizontal: f32,
    vertical: f32,
}

fn linear_lut() -> [f32; 256] {
    let mut result = [0.0f32; 256];
    for (index, value) in result.iter_mut().enumerate() {
        let encoded = index as f32 / 255.0;
        *value = if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        };
    }
    result
}

fn pixel_luminance(pixels: &[u8], width: usize, x: usize, y: usize, lut: &[f32; 256]) -> f32 {
    let offset = (y * width + x) * 3;
    0.2126 * lut[usize::from(pixels[offset])]
        + 0.7152 * lut[usize::from(pixels[offset + 1])]
        + 0.0722 * lut[usize::from(pixels[offset + 2])]
}

fn is_usable_luminance(value: f32) -> bool {
    (BLACK_ENDPOINT..WHITE_ENDPOINT).contains(&value)
}

fn pixel_gradient(
    pixels: &[u8],
    width: usize,
    x: usize,
    y: usize,
    lut: &[f32; 256],
) -> Option<(f32, f32)> {
    let left = pixel_luminance(pixels, width, x - 1, y, lut);
    let right = pixel_luminance(pixels, width, x + 1, y, lut);
    let above = pixel_luminance(pixels, width, x, y - 1, lut);
    let below = pixel_luminance(pixels, width, x, y + 1, lut);
    if ![left, right, above, below]
        .into_iter()
        .all(is_usable_luminance)
    {
        return None;
    }

    Some((
        right.max(f32::MIN_POSITIVE).ln() - left.max(f32::MIN_POSITIVE).ln(),
        below.max(f32::MIN_POSITIVE).ln() - above.max(f32::MIN_POSITIVE).ln(),
    ))
}

fn maximum_translation(width: usize, height: usize) -> usize {
    MAX_TRANSLATION_PIXELS
        .min(width.saturating_sub(4) / 4)
        .min(height.saturating_sub(4) / 4)
}

fn score_step(width: usize, height: usize) -> usize {
    width.max(height).div_ceil(SCORE_GRID_DIMENSION).max(1)
}

fn score_gradient_maps(
    reference: &GradientMap,
    candidate: &GradientMap,
    translation: Translation,
    radius: usize,
) -> Option<f32> {
    if reference.width != candidate.width || reference.height != candidate.height {
        return None;
    }
    let margin = radius + 1;
    if margin * 2 >= reference.width || margin * 2 >= reference.height {
        return None;
    }

    let step = score_step(reference.width, reference.height);
    let mut difference = 0.0f32;
    let mut count = 0usize;
    for y in (margin..reference.height - margin).step_by(step) {
        for x in (margin..reference.width - margin).step_by(step) {
            let candidate_x = x.checked_add_signed(translation.x)?;
            let candidate_y = y.checked_add_signed(translation.y)?;
            if candidate_x >= candidate.width || candidate_y >= candidate.height {
                continue;
            }
            let reference_index = y * reference.width + x;
            let candidate_index = candidate_y * candidate.width + candidate_x;
            if !reference.valid[reference_index] || !candidate.valid[candidate_index] {
                continue;
            }
            difference += (reference.horizontal[reference_index]
                - candidate.horizontal[candidate_index])
                .abs()
                + (reference.vertical[reference_index] - candidate.vertical[candidate_index]).abs();
            count += 1;
        }
    }

    (count >= MIN_VALID_SAMPLES).then_some(difference / count as f32)
}

fn reference_samples(
    pixels: &[u8],
    width: usize,
    height: usize,
    radius: usize,
    lut: &[f32; 256],
) -> Vec<GradientSample> {
    let margin = radius + 1;
    if margin * 2 >= width || margin * 2 >= height {
        return Vec::new();
    }

    let step = score_step(width, height);
    let mut samples = Vec::new();
    for y in (margin..height - margin).step_by(step) {
        for x in (margin..width - margin).step_by(step) {
            if let Some((horizontal, vertical)) = pixel_gradient(pixels, width, x, y, lut) {
                samples.push(GradientSample {
                    x,
                    y,
                    horizontal,
                    vertical,
                });
            }
        }
    }
    samples
}

fn score_full_resolution(
    samples: &[GradientSample],
    candidate: &[u8],
    width: usize,
    height: usize,
    translation: Translation,
    lut: &[f32; 256],
) -> Option<f32> {
    let mut difference = 0.0f32;
    let mut count = 0usize;
    for sample in samples {
        let Some(candidate_x) = sample.x.checked_add_signed(translation.x) else {
            continue;
        };
        let Some(candidate_y) = sample.y.checked_add_signed(translation.y) else {
            continue;
        };
        if candidate_x == 0
            || candidate_y == 0
            || candidate_x + 1 >= width
            || candidate_y + 1 >= height
        {
            continue;
        }
        let Some((horizontal, vertical)) =
            pixel_gradient(candidate, width, candidate_x, candidate_y, lut)
        else {
            continue;
        };
        difference += (sample.horizontal - horizontal).abs() + (sample.vertical - vertical).abs();
        count += 1;
    }

    let required = MIN_VALID_SAMPLES.max(samples.len() / 10);
    (count >= required).then_some(difference / count as f32)
}

fn is_better(
    score: f32,
    translation: Translation,
    best_score: f32,
    best_translation: Translation,
) -> bool {
    const SCORE_EPSILON: f32 = 1.0e-7;
    if score + SCORE_EPSILON < best_score {
        return true;
    }
    if (score - best_score).abs() > SCORE_EPSILON {
        return false;
    }
    let displacement = translation.x * translation.x + translation.y * translation.y;
    let best_displacement =
        best_translation.x * best_translation.x + best_translation.y * best_translation.y;
    displacement < best_displacement
}

pub(crate) fn estimate_translation(
    width: usize,
    height: usize,
    reference: &[u8],
    candidate: &[u8],
) -> Translation {
    if width < 8
        || height < 8
        || reference.len() != width.saturating_mul(height).saturating_mul(3)
        || candidate.len() != reference.len()
    {
        return Translation::default();
    }

    let radius = maximum_translation(width, height);
    if radius == 0 {
        return Translation::default();
    }
    let lut = linear_lut();
    let reference_map = GradientMap::from_rgb(width, height, reference, &lut);
    let candidate_map = GradientMap::from_rgb(width, height, candidate, &lut);
    let coarse_radius = radius.div_ceil(reference_map.scale);
    let mut coarse_score = f32::INFINITY;
    let mut coarse = Translation::default();

    for y in -(coarse_radius as isize)..=coarse_radius as isize {
        for x in -(coarse_radius as isize)..=coarse_radius as isize {
            let translation = Translation { x, y };
            let Some(score) =
                score_gradient_maps(&reference_map, &candidate_map, translation, coarse_radius)
            else {
                continue;
            };
            if is_better(score, translation, coarse_score, coarse) {
                coarse_score = score;
                coarse = translation;
            }
        }
    }
    if !coarse_score.is_finite() {
        return Translation::default();
    }

    let coarse = Translation {
        x: coarse.x * reference_map.scale as isize,
        y: coarse.y * reference_map.scale as isize,
    };
    let samples = reference_samples(reference, width, height, radius, &lut);
    if samples.len() < MIN_VALID_SAMPLES {
        return Translation::default();
    }
    let Some(zero_score) = score_full_resolution(
        &samples,
        candidate,
        width,
        height,
        Translation::default(),
        &lut,
    ) else {
        return Translation::default();
    };

    let refinement = reference_map.scale.saturating_sub(1) as isize;
    let mut best_score = zero_score;
    let mut best = Translation::default();
    for y in coarse.y - refinement..=coarse.y + refinement {
        for x in coarse.x - refinement..=coarse.x + refinement {
            if x.unsigned_abs() > radius || y.unsigned_abs() > radius {
                continue;
            }
            let translation = Translation { x, y };
            let Some(score) =
                score_full_resolution(&samples, candidate, width, height, translation, &lut)
            else {
                continue;
            };
            if is_better(score, translation, best_score, best) {
                best_score = score;
                best = translation;
            }
        }
    }

    if best == Translation::default() || zero_score <= f32::EPSILON {
        return Translation::default();
    }
    let improvement = (zero_score - best_score) / zero_score;
    if improvement < MIN_RELATIVE_IMPROVEMENT {
        Translation::default()
    } else {
        best
    }
}

#[cfg(test)]
mod tests {
    use super::{Translation, estimate_translation};

    fn patterned_frame(width: usize, height: usize) -> Vec<u8> {
        let mut pixels = vec![0u8; width * height * 3];
        for y in 0..height {
            for x in 0..width {
                let offset = (y * width + x) * 3;
                let base = 36 + ((x * 17 + y * 29 + x * y * 3) % 170) as u8;
                pixels[offset] = base;
                pixels[offset + 1] = base.saturating_add(((x * 7 + y) % 31) as u8);
                pixels[offset + 2] = base.saturating_sub(((x + y * 5) % 23) as u8);
            }
        }
        pixels
    }

    fn translated_frame(
        reference: &[u8],
        width: usize,
        height: usize,
        translation: Translation,
        brightness_numerator: u16,
        brightness_denominator: u16,
    ) -> Vec<u8> {
        let mut result = vec![96u8; reference.len()];
        for y in 0..height {
            for x in 0..width {
                let Some(destination_x) = x.checked_add_signed(translation.x) else {
                    continue;
                };
                let Some(destination_y) = y.checked_add_signed(translation.y) else {
                    continue;
                };
                if destination_x >= width || destination_y >= height {
                    continue;
                }
                let source = (y * width + x) * 3;
                let destination = (destination_y * width + destination_x) * 3;
                for channel in 0..3 {
                    result[destination + channel] = ((u16::from(reference[source + channel])
                        * brightness_numerator
                        / brightness_denominator)
                        .min(250)) as u8;
                }
            }
        }
        result
    }

    #[test]
    fn finds_handheld_translation_across_exposure_change() {
        let (width, height) = (160, 120);
        let reference = patterned_frame(width, height);
        let expected = Translation { x: 5, y: -4 };
        let candidate = translated_frame(&reference, width, height, expected, 3, 5);
        assert_eq!(
            estimate_translation(width, height, &reference, &candidate),
            expected
        );
    }

    #[test]
    fn keeps_zero_for_static_frame() {
        let (width, height) = (128, 96);
        let reference = patterned_frame(width, height);
        assert_eq!(
            estimate_translation(width, height, &reference, &reference),
            Translation::default()
        );
    }

    #[test]
    fn rejects_ambiguous_textureless_frame() {
        let (width, height) = (128, 96);
        let reference = vec![96u8; width * height * 3];
        let candidate = vec![144u8; reference.len()];
        assert_eq!(
            estimate_translation(width, height, &reference, &candidate),
            Translation::default()
        );
    }

    #[test]
    fn rejects_invalid_buffers() {
        assert_eq!(
            estimate_translation(16, 16, &[0; 4], &[0; 4]),
            Translation::default()
        );
    }
}
