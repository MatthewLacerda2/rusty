//! FXAA (#360): a **camera** effect, the chain's finishing LDR pass
//! (`postfx/fxaa.wgsl`), on the tonemapped image just before it reaches the target.

use super::{EffectCx, LdrRun, Owner, Pass, PostEffect};
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::GraphicsSettings;
use crate::render::postfx::params::Gate;
use crate::render::postfx::params::{effect_runs, PostSource};
use crate::render::postfx::PostFx;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "fxaa",
    owner: Owner::Camera,
    gate: Gate::option(GraphicsOption::Fxaa),
    module: None,
    composite: &[],
    grade: &[],
    pass: Some(Pass::Ldr {
        order: 20,
        prepare,
        run: LdrRun::Finish(|pfx| &pfx.fxaa_pipeline),
    }),
};

fn prepare(_: &mut PostFx, _: &wgpu::Device, cx: &EffectCx) -> bool {
    cx.full && fxaa_enabled(cx.post, cx.graphics)
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
