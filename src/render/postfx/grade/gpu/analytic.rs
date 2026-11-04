//! The tonemappers on the CPU, for grey inputs (#1701): what
//! `postfx/effects/tonemapping.wgsl` computes per channel when all three are equal,
//! so a LUT test can tell a curve the table drew from one it clipped or bent.

use crate::components::Tonemap;

/// The composite's grade before the LUT (as it was at #1438): exposure, then
/// the tonemap, at neutral contrast, saturation and gamma, for the grey `x`.
pub(super) fn analytic(x: f32, ev: f32, tonemap: Tonemap) -> f32 {
    let h = x * ev.exp2();
    match tonemap {
        Tonemap::None => h,
        Tonemap::Reinhard => h / (h + 1.0),
        Tonemap::Aces => aces(h),
        Tonemap::Neutral => neutral(h),
        Tonemap::AgX => agx(h),
    }
}

fn aces(x: f32) -> f32 {
    let h = 0.6 * x;
    ((h * (2.51 * h + 0.03)) / (h * (2.43 * h + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// Khronos PBR Neutral on a grey: its rolloff, the desaturation a no-op.
fn neutral(x: f32) -> f32 {
    let start = 0.76;
    let offset = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
    let peak = x - offset;
    if peak < start {
        return peak.clamp(0.0, 1.0);
    }
    let d = 1.0 - start;
    1.0 - d * d / (peak + d - start)
}

/// AgX on a grey: every matrix it applies keeps a grey grey (each row sums to one,
/// to their printed digits), so only the log encoding, the sigmoid and the 2.2 act.
fn agx(x: f32) -> f32 {
    let (lo, hi) = (-12.47393f32, 4.026069f32);
    let t = ((x.max(1e-10).log2() - lo) / (hi - lo)).clamp(0.0, 1.0);
    let (t2, t4) = (t * t, t * t * t * t);
    let s =
        15.5 * t4 * t2 - 40.14 * t4 * t + 31.96 * t4 - 6.868 * t2 * t + 0.4298 * t2 + 0.1191 * t
            - 0.00232;
    s.max(0.0).powf(2.2).clamp(0.0, 1.0)
}
