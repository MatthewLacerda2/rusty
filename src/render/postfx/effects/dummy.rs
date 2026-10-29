//! The test-only dummy effect (#1467): a scene pass that draws nothing, its doc
//! section, which no shipped doc holds, and the things an effect declares for
//! itself: its [`Gate`], its own per-view state (a slot of [`Targets`]) and a
//! composite function reading a texture of its own (a [`CompositeTexture`], #1473),
//! so neither `PostFx` nor the composite's shared bindings name it.

use super::{
    Composite, CompositeTexture, EffectCx, Owner, Pass, PassCx, Phase, PostEffect, Targets,
};
use crate::render::postfx::params::Gate;
use crate::render::{RenderView, Renderer};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "dummy",
    owner: Owner::Volume,
    gate: Gate::NONE,
    module: Some("postfx/effects/dummy.wgsl"),
    composite: &[Composite {
        phase: Phase::Hdr,
        order: 90,
        function: "hdr",
        slots: 0,
        // Never provided: the registry binds its black texel.
        textures: &[CompositeTexture {
            name: "probe",
            view: |_| None,
        }],
        pack: |_, _| {},
    }],
    grade: &[],
    pass: Some(Pass::Scene { order: 90, run }),
};

/// The dummy's per-view state: what a real effect's targets would be.
pub(crate) struct DummyState {
    /// The frames it ran since its slot was made.
    pub frames: u32,
}

fn run(_: &Renderer, view: &mut RenderView, cx: &PassCx) {
    track(&mut view.post_fx.targets, &cx.effect);
}

/// Count a frame in the dummy's slot while it runs; drop the slot when it doesn't.
pub(crate) fn track(targets: &mut Targets, cx: &EffectCx) {
    let on = cx.volume().is_some_and(|vc| vc.dummy.active);
    if cx.runs(EFFECT.gate, on) {
        targets.get_or(|| DummyState { frames: 0 }).frames += 1;
    } else {
        targets.remove::<DummyState>();
    }
}

/// Its section, as `docs/api/Graphics.md` would hold it.
pub(super) const DOC: &str = "<!-- effect: dummy -->
#### Dummy

Draws nothing. Tween paths: `Graphics.dummy.strength`.";

#[cfg(test)]
mod tests {
    use super::{track, DummyState};
    use crate::render::postfx::effects::{EffectCx, Targets};
    use crate::render::postfx::params::PostSource;
    use crate::scene::authoring::default_post_processing;

    #[test]
    fn the_dummy_keeps_its_own_slot_while_it_runs() {
        let (mut on, off) = (default_post_processing(), default_post_processing());
        on.dummy.active = true;
        let cx = |vc| EffectCx {
            post: PostSource {
                camera: None,
                volume: Some(vc),
            },
            graphics: Default::default(),
            full: true,
        };
        let mut targets = Targets::default();
        track(&mut targets, &cx(&on));
        track(&mut targets, &cx(&on));
        assert_eq!(targets.get::<DummyState>().map(|s| s.frames), Some(2));
        track(&mut targets, &cx(&off));
        assert!(targets.get::<DummyState>().is_none(), "off drops it");
    }
}
