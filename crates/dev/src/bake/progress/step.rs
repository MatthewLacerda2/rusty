use super::Progress;
use crate::bake::steps::Stage;
use rusty_render::ibl::probe::bounce::MAX_BOUNCES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Lightmaps,
    LightmapPages,
    LightProbes {
        bounce: u32,
        // The solve stops at `max` or when it converges.
        max: u32,
    },
    ReflectionProbes,
    Occlusion,
    WritingLightmaps,
    ProbePlacement,
    Saving,
}

impl Step {
    pub const GENERATE: [Step; 5] = [
        Step::Lightmaps,
        Step::LightmapPages,
        Step::LightProbes { bounce: 1, max: 1 },
        Step::ReflectionProbes,
        Step::Occlusion,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Step::Lightmaps => "lightmaps",
            Step::LightmapPages => "lightmap pages",
            Step::LightProbes { .. } => "light probes",
            Step::ReflectionProbes => "reflection probes",
            Step::Occlusion => "occlusion",
            Step::WritingLightmaps => "writing lightmaps",
            Step::ProbePlacement => "probe placement",
            Step::Saving => "saving",
        }
    }
}

pub fn stage_progress(stage: Stage) -> Option<Progress> {
    let (step, done, total) = match stage {
        Stage::Light {
            bounce,
            done,
            total,
        } => (
            Step::LightProbes {
                bounce,
                max: MAX_BOUNCES,
            },
            done,
            total,
        ),
        Stage::Reflection { done, total } => (Step::ReflectionProbes, done, total),
        Stage::Done => return None,
    };
    if total == 0 {
        return None;
    }
    Some(Progress {
        step,
        done: done as u64,
        total: total as u64,
        meshes: None,
    })
}

impl super::BakeLog<'_> {
    pub fn kept(&mut self, step: Step) {
        self.enter(step);
        let (Some(started), Some((clock, out))) = (self.started, self.on.as_mut()) else {
            return;
        };
        let at = super::format::clock(clock().saturating_sub(started));
        out(super::Line {
            text: format!("[Lighting] {at} {} unchanged, kept", step.name()),
            milestone: true,
        });
    }
}
