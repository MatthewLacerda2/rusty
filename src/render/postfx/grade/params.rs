//! The grading LUT's bake inputs (#1440), packed from a view's volume: what the
//! `Grade` uniform in `postfx/grade.wgsl` holds. White balance and Lift / Gamma /
//! Gain are prepared here exactly as Unity's `ColorUtils` prepares them, so the
//! shader only applies them.

use glam::{Mat3, Vec3};

use crate::components::{LiftGammaGain, PostProcessingComponent, Tonemap, Wheel, WhiteBalance};
use crate::core::color::luminance;

/// CPU mirror of `Grade` in `postfx/grade.wgsl`. A new input joins as a new field
/// here and in the shader; the dirty check compares the bytes, so it is covered
/// with no further change.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GradeParams {
    /// LMS white-point coefficients (xyz); w: `1` when white balance runs.
    pub white_balance: [f32; 4],
    /// Lift, added after gain (xyz); w unused.
    pub lift: [f32; 4],
    /// The per-channel power of the gamma wheel, `1 / gamma` (xyz); w unused.
    pub inv_gamma: [f32; 4],
    /// Gain, the per-channel scale (xyz); w unused.
    pub gain: [f32; 4],
    /// x: hue shift in turns; yzw unused.
    pub hue: [f32; 4],
    /// x: tonemap index, y: contrast, z: saturation, w: display gamma.
    pub tone: [f32; 4],
    /// The user LUT's input range start (xyz); w: its contribution, `0` when no
    /// LUT applies (#1441).
    pub lookup_min: [f32; 4],
    /// The user LUT's input range end (xyz); w unused.
    pub lookup_max: [f32; 4],
}

impl GradeParams {
    /// No grade and no tonemap (clamp only): a view without an active volume, and
    /// the display-transform-only path.
    pub const NEUTRAL: Self = Self {
        white_balance: [1.0, 1.0, 1.0, 0.0],
        lift: [0.0; 4],
        inv_gamma: [1.0, 1.0, 1.0, 0.0],
        gain: [1.0, 1.0, 1.0, 0.0],
        hue: [0.0; 4],
        tone: [Tonemap::None.to_index() as f32, 1.0, 1.0, 1.0],
        lookup_min: [0.0; 4],
        lookup_max: [1.0, 1.0, 1.0, 0.0],
    };

    /// The bake inputs of `volume` (`None`: [`GradeParams::NEUTRAL`]).
    pub fn from_volume(volume: Option<&PostProcessingComponent>) -> Self {
        let Some(vc) = volume else {
            return Self::NEUTRAL;
        };
        // A loaded file bypasses the ops' clamps (serde writes the fields directly),
        // so the knobs are held to the ops' ranges again here.
        let r = WhiteBalance::RANGE;
        let balance = WhiteBalance {
            temperature: bounded(vc.white_balance.temperature, r),
            tint: bounded(vc.white_balance.tint, r),
        };
        let mut lgg = vc.lift_gamma_gain;
        for which in Wheel::ALL {
            let w = lgg.wheel_mut(which);
            w.color = w.color.map(|c| bounded(c - 0.5, 0.5) + 0.5);
            w.offset = bounded(w.offset, 1.0);
        }
        let wb = white_balance(balance);
        let (lift, inv_gamma, gain) = lift_gamma_gain(&lgg);
        let on = if balance.is_neutral() { 0.0 } else { 1.0 };
        Self {
            white_balance: wb.extend(on).to_array(),
            lift: lift.extend(0.0).to_array(),
            inv_gamma: inv_gamma.extend(0.0).to_array(),
            gain: gain.extend(0.0).to_array(),
            hue: [bounded(vc.hue_shift, 180.0) / 360.0, 0.0, 0.0, 0.0],
            tone: [
                vc.tonemap.to_index() as f32,
                vc.contrast,
                vc.saturation,
                vc.gamma.max(0.01),
            ],
            ..Self::NEUTRAL
        }
    }

    /// These inputs with a user LUT spanning `domain` applied at `contribution`,
    /// held to `0..=1` (a NaN applies none).
    pub fn with_lookup(self, domain: [[f32; 3]; 2], contribution: f32) -> Self {
        let k = if contribution.is_nan() {
            0.0
        } else {
            contribution.clamp(0.0, 1.0)
        };
        Self {
            lookup_min: Vec3::from_array(domain[0]).extend(k).to_array(),
            lookup_max: Vec3::from_array(domain[1]).extend(0.0).to_array(),
            ..self
        }
    }
}

/// `v` clamped to `-reach..=reach`, an infinity saturating; `0` (neutral) when NaN.
fn bounded(v: f32, reach: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(-reach, reach)
    }
}

/// Unity's `ColorUtils.ColorBalanceToLMSCoeffs`: the per-channel LMS scale that moves
/// D65 to the white point `temperature` and `tint` name.
pub(super) fn white_balance(wb: WhiteBalance) -> Vec3 {
    if wb.is_neutral() {
        return Vec3::ONE;
    }
    let t1 = wb.temperature / 65.0;
    let t2 = wb.tint / 65.0;
    // The reference white's CIE xy: D65's x, moved along the daylight locus.
    let x = 0.31271 - t1 * if t1 < 0.0 { 0.1 } else { 0.05 };
    let y = 2.87 * x - 3.0 * x * x - 0.275_095_07 + t2 * 0.05;
    let d65 = Vec3::new(0.949_237, 1.035_42, 1.087_28);
    d65 / xy_to_lms(x, y)
}

/// CIE xy (at `Y = 1`) to LMS, through CIE XYZ.
fn xy_to_lms(x: f32, y: f32) -> Vec3 {
    let xyz = Vec3::new(x / y, 1.0, (1.0 - x - y) / y);
    let to_lms = Mat3::from_cols(
        Vec3::new(0.7328, -0.7036, 0.0030),
        Vec3::new(0.4296, 1.6975, 0.0136),
        Vec3::new(-0.1624, 0.0061, 0.9834),
    );
    to_lms * xyz
}

/// Unity's `ColorUtils.PrepareLiftGammaGain`: each wheel's colour minus its own
/// luminance (so a grey wheel tints nothing) plus its offset. Returns `(lift,
/// 1 / gamma, gain)`; gamma and gain are centred on `1`. The luminance is
/// `core::color`'s Rec. 709, Unity's weights, which `postfx/grade.wgsl` mirrors.
pub(super) fn lift_gamma_gain(lgg: &LiftGammaGain) -> (Vec3, Vec3, Vec3) {
    let centred = |w: &crate::components::ColorWheel| {
        let c = Vec3::from_array(w.color);
        c - luminance(c) + w.offset
    };
    let lift = centred(&lgg.lift);
    let gamma = centred(&lgg.gamma) + 1.0;
    let gain = centred(&lgg.gain) + 1.0;
    (lift, 1.0 / gamma.max(Vec3::splat(1e-3)), gain)
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
