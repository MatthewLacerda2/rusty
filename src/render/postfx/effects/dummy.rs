//! The test-only dummy effect (#1467): a scene pass that draws nothing, its doc
//! section, which no shipped doc holds, and the two things an effect declares for
//! itself: the tiers it runs on (its [`Gate`]) and its own per-view state (a slot of
//! [`Targets`]), so neither `QualityPreset` nor `PostFx` names it.

use super::{EffectCx, Owner, Pass, PassCx, PostEffect, Targets};
use crate::core::settings::quality::QualityPreset;
use crate::render::postfx::params::Gate;
use crate::render::{RenderView, Renderer};

pub(crate) const EFFECT: PostEffect = PostEffect {
    name: "dummy",
    owner: Owner::Volume,
    gate: Gate {
        option: None,
        tiers: |q| q == QualityPreset::High,
    },
    module: None,
    composite: &[],
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
    use crate::core::settings::quality::QualityPreset;
    use crate::render::postfx::effects::{EffectCx, Targets};
    use crate::render::postfx::params::PostSource;
    use crate::scene::authoring::default_post_processing;

    #[test]
    fn the_dummy_keeps_its_own_slot_on_its_own_tiers() {
        let mut vc = default_post_processing();
        vc.dummy.active = true;
        let cx = |tier: QualityPreset| EffectCx {
            post: PostSource {
                camera: None,
                volume: Some(&vc),
            },
            graphics: tier.into(),
            full: true,
        };
        let mut targets = Targets::default();
        track(&mut targets, &cx(QualityPreset::High));
        track(&mut targets, &cx(QualityPreset::High));
        assert_eq!(targets.get::<DummyState>().map(|s| s.frames), Some(2));
        track(&mut targets, &cx(QualityPreset::Medium));
        assert!(
            targets.get::<DummyState>().is_none(),
            "Medium doesn't run it"
        );
    }
}
