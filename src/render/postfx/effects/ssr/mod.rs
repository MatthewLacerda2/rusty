//! Screen-space reflections: a composite function on the HDR image (High tier only;
//! on every tier the forward pass reflects the sky or a reflection probe, #718).
//!
//! It reads two per-view targets (#1526): the velocity target (#1435), for each
//! pixel's depth and which camera of the stack drew it, and the SSR surface target the
//! opaque pass writes while SSR runs ([`surface`]): the shading normal, smoothness and
//! environment-reflection weight. So only a smooth surface reflects, and an overlay
//! camera's pixels (a viewmodel) get none. The volume's `quality` sets the march
//! ([`march`], #1521).

pub(crate) mod surface;

use super::{Composite, CompositeTexture, EffectCx, Owner, Phase, PostEffect};
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::QualityPreset;
use crate::core::settings::GraphicsSettings;
use crate::render::postfx::params::{effect_runs, Gate, PostSource};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "ssr",
    owner: Owner::Volume,
    gate: Gate {
        option: Some(GraphicsOption::Ssr),
        tiers: QualityPreset::screen_space_ssr,
    },
    module: Some("postfx/effects/ssr.wgsl"),
    composite: &[Composite {
        phase: Phase::Hdr,
        order: 20,
        function: "hdr",
        slots: 1,
        textures: &[
            CompositeTexture {
                name: "velocity",
                view: |pfx| pfx.motion.active_target().map(|t| &t.view),
            },
            CompositeTexture {
                name: "surface",
                view: surface::view,
            },
        ],
        pack,
    }],
    grade: &[],
    pass: None,
};

/// One quality's ray march: how many steps, how long each (world units). Its reach is
/// their product.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct March {
    pub steps: u32,
    pub step: f32,
}

/// The march for a volume's `quality` name; an unknown name marches as `"High"`.
///
/// The step counts are HDRP's *Max Ray Steps* per quality level (Low 16, Medium 32,
/// High 64, its volume default), and Ultra doubles High. HDRP traces a depth pyramid
/// in screen space, so its reach is the screen; this march steps in world space, so
/// each level also reaches further: 8, 12, 16 and 25.6 m.
pub(crate) fn march(quality: &str) -> March {
    let (steps, step) = match quality {
        "Low" => (16, 0.5),
        "Medium" => (32, 0.375),
        "Ultra" => (128, 0.2),
        _ => (64, 0.25),
    };
    March { steps, step }
}

/// Whether a view whose base camera is `post`'s runs SSR this frame: its volume has
/// it on, the player's option allows it and the tier runs it. `full` is the stack's
/// `full_post_fx`. The opaque pass writes the surface target exactly then.
pub(crate) fn ssr_enabled(post: PostSource, graphics: GraphicsSettings, full: bool) -> bool {
    let on = post.volume.is_some_and(|vc| vc.ssr.active);
    full && effect_runs(graphics, EFFECT.gate, on)
}

/// x: the mode, `2` (screen-space) when it runs, else `0`; y and z the [`march`].
fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume() else { return };
    if cx.runs(EFFECT.gate, vc.ssr.active) {
        let m = march(&vc.ssr.quality);
        slots[0] = [2.0, m.steps as f32, m.step, 0.0];
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod gpu_fixture;

#[cfg(test)]
mod gpu_tests;
