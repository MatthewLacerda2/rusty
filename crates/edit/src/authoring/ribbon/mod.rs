pub mod line;
pub mod trail;

use rusty_asset::MapRef;
use rusty_components::{ParticleBlend, RibbonStyle, TextureMode};
use rusty_core::curve::{Curve, Gradient};
use rusty_scene::rules::sanitize::{finite_curve, finite_gradient, non_negative, unit};

/// Set the width curve along the length; keys clamped to `t ∈ [0, 1]`, `≥ 0`.
pub fn set_width(s: &mut RibbonStyle, width: Curve) {
    let keys: Vec<(f32, f32)> = finite_curve(width)
        .keys
        .iter()
        .map(|k| (unit(k.t), non_negative(k.value)))
        .collect();
    s.width = Curve::from_keys(&keys);
}

/// Set the colour gradient along the length; keys clamped to `t ∈ [0, 1]`,
/// channels `≥ 0` (HDR, so additive tracers bloom), alpha to `[0, 1]`.
pub fn set_color(s: &mut RibbonStyle, color: Gradient) {
    let mut g = finite_gradient(color);
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

pub fn set_texture(s: &mut RibbonStyle, texture: String) {
    s.texture = (!texture.is_empty()).then(|| MapRef::from(texture));
}

pub fn set_texture_mode(s: &mut RibbonStyle, mode: TextureMode) {
    s.texture_mode = mode;
}

pub fn set_blend(s: &mut RibbonStyle, blend: ParticleBlend) {
    s.blend = blend;
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
        assert_eq!(s.texture, Some("beam.png".into()));
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
