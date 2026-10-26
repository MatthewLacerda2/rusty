//! Film grain (#1439): the composite's last function, on the graded image, seeded by
//! the game clock (`PostParams::camera_pos.w`).

use super::super::{Composite, EffectCx, Owner, Phase, PostEffect};
use crate::core::settings::graphics_options::GraphicsOption;
use crate::render::postfx::params::Gate;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "film_grain",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::FilmGrain),
    module: Some("postfx/effects/film_grain.wgsl"),
    composite: &[Composite {
        phase: Phase::Display,
        order: 20,
        function: "display",
        slots: 1,
        textures: &[],
        pack,
    }],
    grade: &[],
    pass: None,
};

/// x: the intensity (`0` when it doesn't run), y: the response, both clamped.
fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume() else { return };
    let grain = &vc.film_grain;
    if cx.runs(EFFECT.gate, grain.active) {
        slots[0][0] = grain.intensity.clamp(0.0, 1.0);
    }
    slots[0][1] = grain.response.clamp(0.0, 1.0);
}
