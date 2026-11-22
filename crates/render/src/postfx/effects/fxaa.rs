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

/// Skipped on a frame TAA resolved: Unity's camera has one anti-aliasing mode.
fn prepare(pfx: &mut PostFx, _: &wgpu::Device, cx: &EffectCx) -> bool {
    cx.full && fxaa_enabled(cx.post, cx.graphics) && !pfx.taa_ran()
}

/// The camera's, never the volume's: default-on, so a camera without a volume, or no camera,
/// is anti-aliased.
pub(crate) fn fxaa_enabled(post: PostSource, graphics: GraphicsSettings) -> bool {
    let on = post.camera.is_none_or(|c| c.fxaa_active);
    effect_runs(graphics, EFFECT.gate, on)
}
