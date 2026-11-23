//! Generate Lighting: lightmaps, probes, then the PVS. The editor steps these functions
//! across frames; `Lighting.Generate()` runs the same ones in line, so the two never drift.

use crate::bake::lighting::{run_to_end, start_lighting, LightingBakeParams, LightingBakeReport};
use crate::bake::lightmap::bake_scene_lightmaps_logged;
use crate::bake::occlusion::summary::occlusion_summary;
use crate::bake::occlusion::{
    bake_occlusion_logged, generate_occlusion_logged, reach, BakedOcclusion,
};
use crate::bake::progress::{BakeLog, Step};
use crate::bake::steps::LightingBake;
use rusty_edit::{save_lighting, LightingSave, LIGHTING_IN_PLAY};
use rusty_lighting::occlusion::OcclusionBakeReport;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;

pub const UNSAVED: &str = "save the scene before generating lighting";

pub const UNSAVED_OCCLUSION: &str = "save the scene before baking occlusion";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenerateReport {
    pub lightmaps: usize,
    pub probes: LightingBakeReport,
    pub occlusion: OcclusionBakeReport,
    // Wall-clock milliseconds.
    pub occlusion_ms: u64,
    pub occlusion_kept: bool,
    pub saved: LightingSave,
}

impl GenerateReport {
    pub fn summary(&self) -> String {
        let p = &self.probes;
        let mut line = format!(
            "Generated lighting: {} lightmap(s), {} light probe(s), {} reflection probe(s)",
            self.lightmaps, p.light_probes, p.reflection_probes
        );
        match self.occlusion_kept {
            true => line.push_str(", occlusion unchanged, kept"),
            false => line.push_str(&occlusion_summary(&self.occlusion, self.occlusion_ms)),
        }
        line.push_str(match self.saved {
            LightingSave::Written => "; saved",
            LightingSave::NeedsSceneSave => "; save the scene to keep it",
        });
        line
    }
}

pub fn generate_lighting(
    scene: &mut Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
    say: &mut dyn FnMut(String),
    log: &mut BakeLog,
) -> Result<GenerateReport, String> {
    refuse_in_play(scene)?;
    refuse_while_baking(scene)?;
    let path = scene_path.ok_or(UNSAVED)?;
    let settings = scene.lighting_settings.lightmaps;
    log.plan(&Step::GENERATE);
    // Past the lightmaps, a failure leaves applied lighting the file lacks, as the editor's
    // button marks it (`inspector::assets::scene::bake::poll`).
    scene.unrecorded(|scene| {
        let lightmaps = bake_scene_lightmaps_logged(scene, Some(path), &settings, log)?;
        let lit = light(scene, path, nav, say, log);
        let (probes, saved) = lit.inspect_err(|_| scene.mark_unsaved())?;
        let occlusion = generate_occlusion_logged(scene, reach(scene, nav), log);
        let unwritten = saved == LightingSave::NeedsSceneSave;
        let occlusion = occlusion.inspect_err(|_| mark_if(scene, unwritten))?;
        let lighting = (lightmaps, probes, saved);
        let report = finish(scene, path, lighting, occlusion);
        let report = report.inspect_err(|_| scene.mark_unsaved())?;
        log.finish();
        Ok(report)
    })
}

/// The probes placed, baked and committed, then the lighting written.
fn light(
    scene: &mut Scene,
    path: &str,
    nav: Option<&NavigationGraph>,
    say: &mut dyn FnMut(String),
    log: &mut BakeLog,
) -> Result<(LightingBakeReport, LightingSave), String> {
    log.enter(Step::ProbePlacement);
    let mut probes = start_probes(scene, path, nav)?;
    run_to_end(&mut probes, scene, say, log)?;
    log.enter(Step::Saving);
    probes.commit(scene)?;
    Ok((probes.report(), write_lighting(scene, path)?))
}

fn mark_if(scene: &mut Scene, unsaved: bool) {
    if unsaved {
        scene.mark_unsaved();
    }
}

pub fn start_probes(
    scene: &mut Scene,
    path: &str,
    nav: Option<&NavigationGraph>,
) -> Result<LightingBake, String> {
    refuse_in_play(scene)?;
    start_lighting(scene, Some(path), nav, LightingBakeParams::default())
}

/// Called before the PVS bakes, so a PVS step that fails or never ends loses no lighting.
pub fn write_lighting(scene: &Scene, path: &str) -> Result<LightingSave, String> {
    refuse_in_play(scene)?;
    save_lighting(scene, path)
}

pub fn finish(
    scene: &mut Scene,
    path: &str,
    (lightmaps, probes, saved): (usize, LightingBakeReport, LightingSave),
    mut occlusion: BakedOcclusion,
) -> Result<GenerateReport, String> {
    refuse_in_play(scene)?;
    let occlusion_kept = occlusion.kept;
    let (report, ms) = match saved {
        LightingSave::Written => {
            let o = apply_occlusion(scene, path, occlusion)?;
            (o.occlusion, o.ms)
        }
        LightingSave::NeedsSceneSave => {
            // Applied, never written: only a full save keeps it, so the scene reads unsaved.
            scene.mark_unsaved();
            let (report, ms) = (occlusion.report, occlusion.ms);
            let (kept, cells) = (occlusion.kept, std::mem::take(&mut occlusion.cells));
            let standable = occlusion.standable.take();
            occlusion.put_on(scene);
            let measured = (&report, ms, (&cells[..], standable.as_deref()));
            let written = (None, kept);
            super::report::occlusion::keep(scene, path, measured, written);
            (report, ms)
        }
    };
    Ok(GenerateReport {
        lightmaps,
        probes,
        occlusion: report,
        occlusion_ms: ms,
        occlusion_kept,
        saved,
    })
}

pub fn bake_occlusion_only(
    scene: &mut Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
    log: &mut BakeLog,
) -> Result<OcclusionOnlyReport, String> {
    refuse_in_play(scene)?;
    refuse_while_baking(scene)?;
    let path = scene_path.ok_or(UNSAVED_OCCLUSION)?;
    log.plan(&[Step::Occlusion]);
    let baked = bake_occlusion_logged(scene, reach(scene, nav), log)?;
    let report = scene.unrecorded(|scene| apply_occlusion(scene, path, baked))?;
    log.finish();
    Ok(report)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcclusionOnlyReport {
    pub occlusion: OcclusionBakeReport,
    pub ms: u64,
}

impl OcclusionOnlyReport {
    pub fn summary(&self) -> String {
        match occlusion_summary(&self.occlusion, self.ms) {
            line if line.is_empty() => "Baked occlusion: nothing occludes, so nothing is \
                 occlusion-culled"
                .to_string(),
            line if self.occlusion.too_large => format!("Baked occlusion{line}"),
            line => format!("Baked occlusion{line}; saved"),
        }
    }
}

pub fn apply_occlusion(
    scene: &mut Scene,
    path: &str,
    mut baked: BakedOcclusion,
) -> Result<OcclusionOnlyReport, String> {
    refuse_in_play(scene)?;
    let report = OcclusionOnlyReport {
        occlusion: baked.report,
        ms: baked.ms,
    };
    let (kept, cells) = (baked.kept, std::mem::take(&mut baked.cells));
    let standable = baked.standable.take();
    baked.put_on(scene);
    let saving = std::time::Instant::now();
    rusty_scene::lighting::occlusion::save_sidecar(scene, path)
        .inspect_err(|_| scene.mark_unsaved())?;
    let saved = Some(saving.elapsed());
    let measured = (
        &report.occlusion,
        report.ms,
        (&cells[..], standable.as_deref()),
    );
    super::report::occlusion::keep(scene, path, measured, (saved, kept));
    Ok(report)
}

pub const BAKE_RUNNING: &str =
    "the editor's Generate Lighting or Bake Occlusion is already running; wait for it to finish";

pub fn refuse_while_baking(scene: &Scene) -> Result<(), String> {
    match scene.bake_busy() {
        true => Err(BAKE_RUNNING.to_string()),
        false => Ok(()),
    }
}

fn refuse_in_play(scene: &Scene) -> Result<(), String> {
    match scene.is_playing() {
        true => Err(LIGHTING_IN_PLAY.to_string()),
        false => Ok(()),
    }
}

#[cfg(test)]
#[path = "generate_kept_tests.rs"]
mod kept_tests;
#[cfg(test)]
#[path = "pvs_only_tests.rs"]
mod pvs_only_tests;
#[cfg(test)]
#[path = "generate_lighting_tests.rs"]
mod tests;
