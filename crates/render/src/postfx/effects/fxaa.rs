//! FXAA (#360): a **camera** effect, the chain's finishing LDR pass
//! (`postfx/fxaa.wgsl`), on the tonemapped image just before it reaches the target.

use super::{EffectCx, History, LdrRun, Owner, Pass, PostEffect};
use crate::postfx::params::Gate;
use crate::postfx::params::{effect_runs, PostSource};
use crate::postfx::PostFx;
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::GraphicsSettings;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "fxaa",
    owner: Owner::Camera,
    gate: Gate::option(GraphicsOption::Fxaa),
    history: History::None("a function of this frame's image"),
    module: None,
    composite: &[],
    grade: &[],
    passes: &[Pass::Ldr {
        order: 20,
        prepare,
        run: LdrRun::Finish(|pfx| &pfx.fxaa_pipeline),
    }],
};

/// Skipped on a frame TAA resolved (#1702): Unity's camera has one anti-aliasing
/// mode, and TAA's is the one that ran.
fn prepare(pfx: &mut PostFx, _: &wgpu::Device, cx: &EffectCx) -> bool {
    cx.full && fxaa_enabled(cx.post, cx.graphics) && !pfx.taa_ran()
}

/// Whether the FXAA pass runs this frame for `post`'s camera.
///
/// Deliberately *not* tied to the volume: FXAA lives on the camera, so tying it to
/// the volume would make anti-aliasing depend on the camera carrying a
/// `PostProcessing` — the opposite of a default-on effect.
///
/// No camera component at all gets `true`: default-on means a scene that has never
/// heard of the knob is still anti-aliased.
///
/// It has no quality knob: it is precisely the cheap anti-aliasing an iGPU wants;
/// the player's `fxaa` option can turn it off.
pub(crate) fn fxaa_enabled(post: PostSource, graphics: GraphicsSettings) -> bool {
    let on = post.camera.is_none_or(|c| c.fxaa_active);
    effect_runs(graphics, EFFECT.gate, on)
}
