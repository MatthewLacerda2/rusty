//! What an editor lightmap bake measures for its report (#1980), kept beside
//! the job ([`super::job`]) so the job stays about running the bake: the
//! settings, the gather's time, each mesh's worker time and what OIDN took.

use std::sync::Arc;
use std::time::Duration;

use super::{keep_report, lend_denoiser, EncodedPages};
use crate::bake::denoise::Oidn;
use crate::bake::progress::MeshClocks;
use crate::bake::report::{lightmaps::Measured, Stages};
use rusty_scene::lighting::lightmap::{BakeProgress, BakeScene, BakeSettings};
use rusty_scene::Scene;

/// A bake's clocks, from its start to its report.
pub(super) struct Measure {
    settings: BakeSettings,
    gathered: Duration,
    clocks: Arc<MeshClocks>,
    oidn: Option<Arc<Oidn>>,
}

impl Measure {
    /// The progress a bake with `settings` counts into, timing each mesh and
    /// lent OIDN when the settings pick it, and its clocks; the gather took
    /// `gathered`.
    pub fn start(settings: &BakeSettings, gathered: Duration) -> (BakeProgress, Self) {
        let clocks = Arc::new(MeshClocks::default());
        let (progress, oidn) = lend_denoiser(BakeProgress::observed(clocks.clone()), settings);
        let measure = Self {
            settings: *settings,
            gathered,
            clocks,
            oidn,
        };
        (progress, measure)
    }

    /// Write the report of the bake that read `input`, traced for `trace`,
    /// encoded `pages` for `encode` and wrote them for `write`.
    pub fn keep(
        &self,
        (scene, scene_path): (&Scene, &str),
        input: &BakeScene,
        pages: &EncodedPages,
        [trace, encode, write]: [Duration; 3],
    ) {
        let mut stages = Stages::start();
        stages.push("gather", self.gathered);
        stages.push("trace", trace);
        stages.push("encode", encode);
        stages.push("write", write);
        let measured = Measured {
            times: self.clocks.times(),
            denoised: self.oidn.as_ref().map(|o| o.spent()),
        };
        let read = (input, &self.settings);
        keep_report(scene, scene_path, read, (pages, &measured), &stages);
    }
}
