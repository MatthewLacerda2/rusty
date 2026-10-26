//! The authored effects as a post effect (#397): an in-place LDR pass after the
//! composite and before FXAA, each effect one fullscreen pass ping-ponging between the
//! chain's two LDR targets, the result kept as next frame's history.

use std::collections::BTreeMap;

use super::super::{EffectCx, Gpu, LdrRun, Owner, Pass, PostEffect};
use super::CustomChain;
use crate::render::postfx::{PostFx, PostFxContext};
use crate::render::timing::GpuPass;

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "custom_effects",
    owner: Owner::Volume,
    option: None,
    module: None,
    composite: &[],
    grade: &[],
    pass: Some(Pass::Ldr {
        order: 10,
        prepare,
        run: LdrRun::InPlace(run),
    }),
};

/// No authored params: a view without a volume.
static NO_PARAMS: BTreeMap<String, Vec<f32>> = BTreeMap::new();

/// The volume's module names and their params (none without a volume, or on a view
/// that runs no post-FX).
fn chain<'a>(cx: &EffectCx<'a>) -> (&'a [String], &'a BTreeMap<String, Vec<f32>>) {
    match cx.volume() {
        Some(vc) => (&vc.custom_effects.names, &vc.custom_effects.params),
        None => (&[], &NO_PARAMS),
    }
}

/// Compile any module not cached yet; whether one of them loaded.
fn prepare(pfx: &mut PostFx, device: &wgpu::Device, cx: &EffectCx) -> bool {
    let (names, _) = chain(cx);
    pfx.custom.prepare(device, names);
    !pfx.custom.loaded(names).is_empty()
}

/// Run the loaded effects over [`PostFx::ldr`], keep the result as next frame's
/// history, and leave it in `ldr`.
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
        // The result is in the second target: make it the chain's `ldr`.
        std::mem::swap(&mut pfx.ldr, &mut pfx.custom.ldr_b);
    }
}
