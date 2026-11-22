use std::sync::Arc;
use std::time::Duration;

use super::{keep_report, lend_denoiser, EncodedPages};
use crate::bake::denoise::Oidn;
use crate::bake::progress::MeshClocks;
use crate::bake::report::{lightmaps::Measured, Stages};
use rusty_lighting::lightmap::{BakeProgress, BakeScene};
use rusty_scene::lighting::settings::BakeSettings;
use rusty_scene::Scene;

pub(super) struct Measure {
    settings: BakeSettings,
    gathered: Duration,
    clocks: Arc<MeshClocks>,
    oidn: Option<Arc<Oidn>>,
}

impl Measure {
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
            peak_memory: crate::memory::rss_peak_bytes(),
        };
        let read = (input, &self.settings);
        keep_report(scene, scene_path, read, (pages, &measured), &stages);
    }
}
