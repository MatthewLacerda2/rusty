mod cache;
mod chain;
#[cfg(test)]
mod chain_gpu_tests;
pub(crate) mod dirt;
#[cfg(test)]
mod dirt_tests;
#[cfg(test)]
mod gpu_tests;
mod passes;
#[cfg(test)]
mod prefilter_gpu_tests;

use crate::gpu::commands::Commands;
use crate::gpu::ops::CountedQueue as _;
use chain::BloomParams;
pub(crate) use chain::BloomTargets;
pub(crate) use passes::BloomPasses;

use super::{
    Composite, CompositeTexture, EffectCx, History, Owner, Pass, PassCx, Phase, PostEffect,
};
use crate::postfx::params::Gate;
use crate::{RenderView, Renderer};
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::quality::MAX_RESOLUTION_DIVISOR;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "bloom",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::Bloom),
    history: History::None("its mip chain is rebuilt from this frame's scene"),
    module: Some("postfx/effects/bloom.wgsl"),
    composite: &[Composite {
        phase: Phase::Hdr,
        order: 30,
        function: "hdr",
        slots: 1,
        textures: &[CompositeTexture {
            name: "dirt",
            view: |pfx| pfx.bloom_dirt.as_ref().map(|t| &t.view),
        }],
        pack,
    }],
    grade: &[],
    passes: &[Pass::Scene { order: 40, run }],
};

fn blooms(cx: &EffectCx) -> bool {
    cx.volume()
        .is_some_and(|vc| cx.runs(EFFECT.gate, vc.bloom.active))
}

/// x: the intensity, y: the dirt's, scaled by the intensity as Unity's `UberPost`
/// scales it (`0` without a dirt texture: the composite then skips the dirt), w:
/// `1` while bloom runs (the composite skips the add otherwise).
fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume() else { return };
    let on = if blooms(cx) { 1.0 } else { 0.0 };
    let b = &vc.bloom;
    let dirt = if b.dirt_texture.is_some() {
        b.intensity * b.dirt_intensity
    } else {
        0.0
    };
    slots[0] = [b.intensity, dirt, 0.0, on];
}

fn run(r: &Renderer, view: &mut RenderView, cx: &PassCx, commands: &mut Commands) {
    let pfx = &mut view.post_fx;
    let Some(vc) = cx.effect.volume().filter(|_| blooms(&cx.effect)) else {
        pfx.bloom_dirt = None;
        pfx.targets.remove::<BloomTargets>();
        return;
    };
    let levels = &cx.effect.graphics.quality.bloom_resolution_divisor;
    let divisor = vc.bloom.quality.pick(levels, vc.bloom.resolution_divisor);
    let divisor = divisor.clamp(1, MAX_RESOLUTION_DIVISOR);
    let (device, passes) = (&r.device, &r.post_passes.bloom);
    let (scene, sampler) = (pfx.chain_size, &pfx.sampler);
    let make = || BloomTargets::new(device, passes, sampler, scene, divisor);
    let t = pfx.targets.get_or(make);
    if t.made_for != (scene, divisor) {
        *t = make();
    }
    pfx.bloom_dirt = r.lens_dirt(vc);
    let pfx = &view.post_fx;
    let t = pfx.targets.get::<BloomTargets>().expect("made above");
    let params = BloomParams::new(&vc.bloom, t);
    r.queue
        .counted_write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
    let input = (pfx.scene_color(), &pfx.sampler);
    passes.encode(device, commands.encoder(), t, input, &r.gpu_timer);
}
