use glam::{Vec3, Vec4};

use super::super::trace::TexelLight;
use rusty_core::color::luminance;

/// Lamp share at which the stored direction is fully the lamps' mean; below it
/// it blends from the mean of all light, so a fading lamp leaves no seam.
pub(in super::super) const LAMP_RAMP: f32 = 0.25;

pub(super) fn direction_texel(indirect: Vec3, indirect_dir: Vec3, light: &TexelLight) -> Vec4 {
    let weight = luminance(indirect + light.direct);
    if weight <= 0.0 {
        return Vec4::ZERO;
    }
    let all = ((indirect_dir + light.direct_dir) / weight).clamp_length_max(1.0);
    let share = (light.direct_dir.length() / weight).min(1.0);
    let mean = all.normalize_or_zero();
    let toward = match light.direct_dir.try_normalize() {
        Some(lamp) => {
            let t = (share / LAMP_RAMP).min(1.0);
            mean.lerp(lamp, t).try_normalize().unwrap_or(lamp)
        }
        None => mean,
    };
    (toward * all.length()).extend(share)
}
