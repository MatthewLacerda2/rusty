//! A directional bake's texel (#810, #1695): where the light comes from, and how much
//! of it is `Baked` lamps' direct light, the part the forward shader turns into one
//! specular lobe (`forward::lightmap::baked_specular`).
//!
//! The tracer keeps a texel's lamp light (`TexelLight::direct`, `Baked` lights only)
//! apart from its bounce and sky until here, so the lobe can come from lamps alone: an
//! even sky over a dark ground, or a `Mixed` spot's bounce off a small patch, makes a
//! texel directional with no lamp anywhere, and must give no highlight.

use glam::{Vec3, Vec4};

use super::super::trace::TexelLight;
use rusty_core::color::luminance;

/// The lamp share over which the stored direction turns from the mean of all the
/// texel's light to the lamps' own: a quarter, so a lamp supplying most of a texel
/// aims its lobe at itself, while one fading out at the edge of its range hands the
/// direction back smoothly instead of leaving a seam on a normal-mapped surface.
pub(in super::super) const LAMP_RAMP: f32 = 0.25;

/// One texel's direction page value: `xyz` its dominant incoming direction scaled
/// by its directionality (the luminance-weighted mean of where its smoothed bounce
/// and its direct light arrived from), `w` its lamp share, `|direct_dir| /
/// luminance(direct + indirect)`: 1 for one point, spot or directional lamp alone,
/// 0 with none, weakened by two opposed lamps, and by a `Baked` Area light whose
/// panel fills the texel's view (its arrivals spread, so `direct_dir` shortens). The direction is the lamps' mean once the share reaches
/// [`LAMP_RAMP`], blended from the mean of everything below it. Black texels have none.
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
