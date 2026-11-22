use super::progress::{stage_progress, BakeLog};
use super::report::{self, probes::Timeline};
use super::steps::Stage;
use rusty_lighting::lightmap::probe_direct;
use rusty_render::{Renderer, DEFAULT_BAKE_RESOLUTION};
use rusty_scene::Scene;

pub fn bake_scene_probes(scene: &mut Scene, log: &mut BakeLog) -> Result<bool, String> {
    bake_scene_probes_at(scene, None, log)
}

pub fn bake_scene_probes_at(
    scene: &mut Scene,
    scene_path: Option<&str>,
    log: &mut BakeLog,
) -> Result<bool, String> {
    if scene.probes.is_empty() {
        return Ok(true);
    }
    let res = DEFAULT_BAKE_RESOLUTION;
    let mut renderer = match pollster::block_on(Renderer::new_headless(res, res)) {
        Some(r) => r,
        None => {
            log::warn!("[ProbeBake] no GPU/software adapter available — skipping bake");
            return Ok(false);
        }
    };
    let mut times = Timeline::default();
    renderer.bake_probes_observed(scene, res, probe_direct, &mut |bounce, done, total| {
        let stage = Stage::Light {
            bounce,
            done,
            total,
        };
        times.note(stage);
        if let Some(p) = stage_progress(stage) {
            log.observe(p);
        }
    });
    times.note(Stage::Done);
    log.finish();
    if let Some(path) = scene_path {
        let print = report::probes::scene_fingerprint(scene);
        report::keep(
            path,
            report::probes::report(scene, &times.stages().0, print),
        );
    }
    Ok(true)
}
