//! Bloom: a scene pass, before the lens flare's, which reads its result, and the
//! composite's add of that result on the HDR image (`postfx/effects/bloom.wgsl`). The
//! pass is a mip chain (#1700, `postfx/bloom.wgsl`, [`chain`]), after Jimenez's
//! Call of Duty bloom with URP's knobs: a 13-tap prefilter with a Karis average and
//! a soft-knee threshold into the first mip, at its quality's reduced resolution
//! (#1522), 13-tap downsamples to a few texels across, and 3×3 tent upsamples
//! blended back up by the scatter. The prefilter reads the scene after depth of field
//! and motion blur, so a moving highlight blooms along its streak, as in URP and HDRP
//! (#1436). The add lights the lens dirt too (`dirt`, #1442), a composite texture of
//! bloom's own (#1473).

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

use chain::BloomParams;
pub(crate) use chain::BloomTargets;
pub(crate) use passes::BloomPasses;

use super::{Composite, CompositeTexture, EffectCx, Owner, Pass, PassCx, Phase, PostEffect};
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::MAX_RESOLUTION_DIVISOR;
use crate::render::postfx::params::Gate;
use crate::render::{RenderView, Renderer};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "bloom",
    owner: Owner::Volume,
    gate: Gate::option(GraphicsOption::Bloom),
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
    pass: Some(Pass::Scene { order: 40, run }),
};

/// Whether `cx`'s volume blooms this frame.
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

/// The chain, when the view blooms, at its quality's divisor (made the first frame
/// it does, dropped when it stops): the prefilter (scene -> `down[0]`), each
/// downsample, then each upsample from the bottom (`BloomPasses::encode`); with the
/// lens dirt its volume names for the composite.
fn run(r: &Renderer, view: &mut RenderView, cx: &PassCx) {
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
    let (scene, sampler) = (pfx.full_size, &pfx.sampler);
    let make = || BloomTargets::new(device, passes, sampler, scene, divisor);
    let t = pfx.targets.get_or(&make);
    if t.divisor != divisor {
        *t = make();
    }
    pfx.bloom_dirt = r.lens_dirt(vc);
    let pfx = &view.post_fx;
    let t = pfx.targets.get::<BloomTargets>().expect("made above");
    let params = BloomParams::new(&vc.bloom, t);
    r.queue
        .write_buffer(&t.params, 0, bytemuck::bytes_of(&params));
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Bloom Encoder"),
    });
    let input = (pfx.scene_color(), &pfx.sampler);
    passes.encode(device, &mut encoder, t, input, &r.gpu_timer);
    r.queue.submit(std::iter::once(encoder.finish()));
}
