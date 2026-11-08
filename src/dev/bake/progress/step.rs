//! The steps a bake runs, as the log names them, and a probe stage read as one.

use super::Progress;
use crate::dev::bake::steps::Stage;
use crate::render::ibl::probe::bounce::MAX_BOUNCES;

/// Which part of a bake is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Lightmaps,
    /// The lightmap pages being encoded after the trace (#1694): a line per page.
    LightmapPages,
    /// One pass of the light-probe solve, which stops at `max` or when it converges.
    LightProbes {
        bounce: u32,
        max: u32,
    },
    ReflectionProbes,
    /// The occlusion culling's PVS (#1714): cells baked of the grid's.
    Occlusion,
}

impl Step {
    /// Every step Generate Lighting runs, in order (#1897): its [`BakeLog::plan`](super::BakeLog::plan).
    pub const GENERATE: [Step; 5] = [
        Step::Lightmaps,
        Step::LightmapPages,
        Step::LightProbes { bounce: 1, max: 1 },
        Step::ReflectionProbes,
        Step::Occlusion,
    ];

    /// The step's name in a plan: every light-probe bounce is one entry.
    pub(super) fn name(self) -> &'static str {
        match self {
            Step::Lightmaps => "lightmaps",
            Step::LightmapPages => "lightmap pages",
            Step::LightProbes { .. } => "light probes",
            Step::ReflectionProbes => "reflection probes",
            Step::Occlusion => "occlusion",
        }
    }
}

/// Where a stepped probe bake is, as the log reads it; `None` once it is done, and for
/// a probe step with nothing to bake (no probes of its kind), which has nothing to say.
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
