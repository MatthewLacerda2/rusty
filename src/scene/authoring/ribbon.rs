//! src/scene/authoring/ribbon.rs — Shared ribbon-style ops (#441).
//!
//! The ONE place a Trail's or a Line's [`RibbonStyle`] is written. The editor's
//! Trail and Line cards and the Lua `Trail.*` / `Line.*` style verbs all route
//! through these, so the bounds live once: widths are non-negative, curve and
//! gradient keys sit in `t ∈ [0, 1]`, colours are non-negative (HDR is allowed,
//! for additive tracers that bloom) and alpha stays in `[0, 1]`.
//!
//! Pure.

use crate::components::{ParticleBlend, RibbonStyle, TextureMode};
use crate::core::curve::{Curve, Gradient};

/// Set the width curve along the length; keys clamped to `t ∈ [0, 1]`, `≥ 0`.
pub fn set_width(s: &mut RibbonStyle, width: Curve) {
    let keys: Vec<(f32, f32)> = width
        .keys
        .iter()
        .map(|k| (unit(k.t), non_negative(k.value)))
        .collect();
    s.width = Curve::from_keys(&keys);
}

/// Set the colour gradient along the length; keys clamped to `t ∈ [0, 1]`,
/// channels `≥ 0`, alpha to `[0, 1]`.
pub fn set_color(s: &mut RibbonStyle, color: Gradient) {
    let mut g = color;
    for key in &mut g.color_keys {
        key.t = unit(key.t);
        key.color = key.color.map(non_negative);
    }
    for key in &mut g.alpha.keys {
        key.t = unit(key.t);
        key.value = unit(key.value);
    }
    g.sort();
    s.color = g;
}

/// Set (or, with an empty string, clear) the texture path.
pub fn set_texture(s: &mut RibbonStyle, texture: String) {
    s.texture = (!texture.is_empty()).then_some(texture);
}

/// Set how the texture maps along the length.
pub fn set_texture_mode(s: &mut RibbonStyle, mode: TextureMode) {
    s.texture_mode = mode;
}

/// Set the blend (alpha or additive).
pub fn set_blend(s: &mut RibbonStyle, blend: ParticleBlend) {
    s.blend = blend;
}

pub(super) fn non_negative(v: f32) -> f32 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

fn unit(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_ops_clamp() {
        let mut s = RibbonStyle::default();
        set_width(&mut s, Curve::from_keys(&[(-1.0, -2.0), (2.0, 0.5)]));
        assert_eq!(s.width, Curve::from_keys(&[(0.0, 0.0), (1.0, 0.5)]));
        set_color(&mut s, Gradient::linear([-1.0, 2.0, 0.5, 3.0], [0.0; 4]));
        assert_eq!(s.color.evaluate(0.0), [0.0, 2.0, 0.5, 1.0]);
        set_texture(&mut s, "beam.png".into());
        assert_eq!(s.texture.as_deref(), Some("beam.png"));
        set_texture(&mut s, String::new());
        assert_eq!(s.texture, None);
        set_texture_mode(&mut s, TextureMode::Tile);
        set_blend(&mut s, ParticleBlend::Additive);
        assert_eq!(
            (s.texture_mode, s.blend),
            (TextureMode::Tile, ParticleBlend::Additive)
        );
    }
}
