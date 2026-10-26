//! The exposure as a post effect (#1444): the meter, a scene pass after motion blur,
//! and the composite's pre-grade scale, the fixed EV or the adapted one.

use super::super::{Composite, EffectCx, Owner, Pass, PassCx, Phase, PostEffect};
use super::ExposureParams;
use crate::components::{AutoExposure, ExposureMode};
use crate::render::motion::HistoryKey;
use crate::render::postfx::params::Gate;
use crate::render::postfx::params::PostSource;
use crate::render::{RenderView, Renderer};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "exposure",
    owner: Owner::Volume,
    // The player has no option for it: the exposure is the developer's look.
    gate: Gate::NONE,
    module: Some("postfx/effects/exposure.wgsl"),
    composite: &[Composite {
        phase: Phase::Hdr,
        order: 40,
        function: "hdr",
        slots: 1,
        textures: &[],
        pack,
    }],
    grade: &[],
    pass: Some(Pass::Scene {
        order: 30,
        run: meter,
    }),
};

/// x: the fixed EV, y: `1` in Automatic (the composite reads the adapted EV).
fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume() else { return };
    let auto = vc.exposure.mode == ExposureMode::Automatic;
    slots[0] = [vc.exposure.fixed_ev, if auto { 1.0 } else { 0.0 }, 0.0, 0.0];
}

/// `post`'s automatic-exposure knobs, when its volume meters the exposure.
fn automatic<'a>(post: PostSource<'a>) -> Option<&'a AutoExposure> {
    post.volume
        .filter(|vc| vc.exposure.mode == ExposureMode::Automatic)
        .map(|vc| &vc.exposure.automatic)
}

/// Meter this view's scene, after depth of field and motion blur, and adapt, on the
/// GPU, when its volume is `Automatic` and the view runs full post-FX; otherwise the
/// next metered frame snaps.
fn meter(r: &Renderer, view: &mut RenderView, cx: &PassCx) {
    let (meter, scene) = view.post_fx.exposure_and_scene();
    let Some(ae) = automatic(cx.stack.post).filter(|_| cx.effect.full) else {
        return meter.cut();
    };
    let key = HistoryKey::new(cx.scene, cx.stack.base_id);
    let step = meter.step(key, cx.stack.editor_mode, cx.scene.shader_time);
    let params = ExposureParams::new(ae, step);
    let gpu = (&r.device, &r.queue);
    meter.run(gpu, &r.post_passes.exposure, scene, &params);
}
