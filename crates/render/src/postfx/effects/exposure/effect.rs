use super::super::{Composite, EffectCx, History, Owner, Pass, PassCx, Phase, PostEffect};
use super::ExposureParams;
use crate::gpu::commands::Commands;
use crate::motion::HistoryKey;
use crate::postfx::params::Gate;
use crate::postfx::params::PostSource;
use crate::{RenderView, Renderer};
use rusty_components::{AutoExposure, ExposureMode};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "exposure",
    owner: Owner::Volume,
    // The player has no option for it: the exposure is the developer's look.
    gate: Gate::NONE,
    history: History::None(
        "it adapts on game time, which a redraw with no sim step does not advance",
    ),
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
    passes: &[Pass::Scene {
        order: 30,
        run: meter,
    }],
};

/// x: the fixed EV, y: `1` in Automatic (the composite reads the adapted EV).
fn pack(cx: &EffectCx, slots: &mut [[f32; 4]]) {
    let Some(vc) = cx.volume() else { return };
    let auto = vc.exposure.mode == ExposureMode::Automatic;
    slots[0] = [vc.exposure.fixed_ev, if auto { 1.0 } else { 0.0 }, 0.0, 0.0];
}

fn automatic<'a>(post: PostSource<'a>) -> Option<&'a AutoExposure> {
    post.volume
        .filter(|vc| vc.exposure.mode == ExposureMode::Automatic)
        .map(|vc| &vc.exposure.automatic)
}

fn meter(r: &Renderer, view: &mut RenderView, cx: &PassCx, commands: &mut Commands) {
    let (meter, scene) = view.post_fx.exposure_and_scene();
    let Some(ae) = automatic(cx.stack.post).filter(|_| cx.effect.full) else {
        return meter.cut();
    };
    let key = HistoryKey::new(cx.scene, cx.stack.base_id);
    let step = meter.step(key, cx.stack.editor_mode, cx.scene.shader_time);
    let params = ExposureParams::new(ae, step);
    let gpu = (&r.device, &r.queue);
    let passes = (&r.post_passes.exposure, &r.gpu_timer);
    meter.run(gpu, commands, passes, scene, &params);
}

impl crate::RenderView {
    pub fn exposure_readout(&self) -> Option<f32> {
        self.post_fx.exposure.readout()
    }

    pub fn settled_exposure(&mut self, device: &wgpu::Device) -> Option<f32> {
        self.post_fx.exposure.settle(device);
        self.exposure_readout()
    }
}
