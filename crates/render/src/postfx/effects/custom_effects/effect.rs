use crate::postfx::params::Gate;
use std::collections::BTreeMap;

use super::super::{EffectCx, Gpu, History, LdrRun, Owner, Pass, PostEffect};
use super::CustomChain;
use crate::postfx::{PostFx, PostFxContext};
use crate::timing::GpuPass;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "custom_effects",
    owner: Owner::Volume,
    gate: Gate::NONE,
    history: History::Reads(|cx| reads_history(&cx.effect)),
    module: None,
    composite: &[],
    grade: &[],
    passes: &[Pass::Ldr {
        order: 10,
        prepare,
        run: LdrRun::InPlace(run),
    }],
};

static NO_PARAMS: BTreeMap<String, Vec<f32>> = BTreeMap::new();

fn chain<'a>(cx: &EffectCx<'a>) -> (&'a [String], &'a BTreeMap<String, Vec<f32>>) {
    match cx.volume() {
        Some(vc) => (&vc.custom_effects.names, &vc.custom_effects.params),
        None => (&[], &NO_PARAMS),
    }
}

fn reads_history(cx: &EffectCx) -> bool {
    !chain(cx).0.is_empty()
}

fn prepare(pfx: &mut PostFx, device: &wgpu::Device, cx: &EffectCx) -> bool {
    let (names, _) = chain(cx);
    pfx.custom.prepare(device, names);
    !pfx.custom.loaded(names).is_empty()
}

fn run(
    pfx: &mut PostFx,
    (device, queue): Gpu,
    encoder: &mut wgpu::CommandEncoder,
    ctx: &PostFxContext,
    cx: &EffectCx,
) {
    let (names, params) = chain(cx);
    let effects = pfx.custom.loaded(names);
    CustomChain::write_params(queue, &effects, params);
    let (mut src, mut dst) = (&pfx.ldr, &pfx.custom.ldr_b);
    for effect in &effects {
        let bg = pfx.io_bind_group(
            device,
            &src.view,
            &pfx.custom.history.view,
            ctx.depth_view,
            ctx.skybox,
        );
        let pass = (&effect.pipeline, ctx.timer.writes(GpuPass::PostFx));
        PostFx::fullscreen(encoder, pass, &[&bg, &effect.params], &dst.view);
        (src, dst) = (dst, src);
    }
    encoder.copy_texture_to_texture(
        src.texture.as_image_copy(),
        pfx.custom.history.texture.as_image_copy(),
        src.texture.size(),
    );
    if effects.len() % 2 == 1 {
        std::mem::swap(&mut pfx.ldr, &mut pfx.custom.ldr_b);
    }
}
