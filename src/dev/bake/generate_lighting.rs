//! src/dev/bake/generate_lighting.rs — Generate Lighting (#832): every bake a scene has,
//! in order, with the result written.
//!
//! Unity 5's one button: lightmaps first (`lightmap_bake`, #438), then light and
//! reflection probes (`lighting_bake`, #246), both with the scene's own
//! [`LightingSettings`](crate::scene::lighting::LightingSettings), then the
//! occlusion culling's PVS (`scene::lighting::occlusion`, #1714, which Unity bakes
//! from its Occlusion window). The lighting is written into the scene file
//! ([`write_lighting`], [`save_lighting`]) as soon as the probes commit, so nobody
//! has to remember to save and a PVS step dropped or failed loses none of it; the
//! PVS is written beside it when it ends ([`finish`]). The individual bakes
//! stay for scripts that want one of them.
//!
//! The editor runs the lightmap step on a worker (#808), then steps the probe bake
//! ([`start_probes`]) one frame at a time (#1602, Unity's `BakeAsync`), then bakes
//! the PVS on a worker (`occlusion::OcclusionBakeJob`) and calls [`finish`];
//! `Lighting.Generate()` calls [`generate_lighting`], which runs the same steps in
//! line (Unity's `Lightmapping.Bake`). Same path, so the two never drift.

use crate::dev::bake::lighting::{
    run_to_end, start_lighting, LightingBakeParams, LightingBakeReport,
};
use crate::dev::bake::lightmap::bake_scene_lightmaps_logged;
use crate::dev::bake::occlusion::{bake_occlusion_logged, BakedOcclusion};
use crate::dev::bake::progress::{BakeLog, Step};
use crate::dev::bake::steps::LightingBake;
use crate::navigation::NavigationGraph;
use crate::scene::lighting::occlusion::OcclusionBakeReport;
use crate::scene::{save_lighting, LightingSave, Scene, LIGHTING_IN_PLAY};

/// Why Generate Lighting cannot start: the bakes write beside the scene file.
pub const UNSAVED: &str = "save the scene before generating lighting";

/// Why Bake Occlusion cannot start: the PVS is written beside the scene file.
pub const UNSAVED_OCCLUSION: &str = "save the scene before baking occlusion";

/// What one Generate Lighting did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenerateReport {
    /// Lightmaps written (0 when no static mesh has a lightmap UV).
    pub lightmaps: usize,
    /// The probe step's placement and bake.
    pub probes: LightingBakeReport,
    /// The occlusion bake (#1714): its cells, rows and size; all zero when nothing
    /// occludes.
    pub occlusion: OcclusionBakeReport,
    /// Wall-clock milliseconds the occlusion bake took.
    pub occlusion_ms: u64,
    /// Whether the lighting reached the scene file or needs a full save.
    pub saved: LightingSave,
}

impl GenerateReport {
    /// One line for the console.
    pub fn summary(&self) -> String {
        let p = &self.probes;
        let mut line = format!(
            "Generated lighting: {} lightmap(s), {} light probe(s), {} reflection probe(s)",
            self.lightmaps, p.light_probes, p.reflection_probes
        );
        line.push_str(&occlusion_summary(&self.occlusion, self.occlusion_ms));
        line.push_str(match self.saved {
            LightingSave::Written => "; saved",
            LightingSave::NeedsSceneSave => "; save the scene to keep it",
        });
        line
    }
}

/// The occlusion bake's part of the summary line: empty when nothing occludes.
fn occlusion_summary(o: &OcclusionBakeReport, ms: u64) -> String {
    let never = match o.never_culled {
        0 => String::new(),
        n => format!(", {n} never culled (too large to sample)"),
    };
    if o.too_large {
        let max = crate::scene::lighting::occlusion::MAX_CELL;
        return format!(", no occlusion culling: the level needs cells over {max} m");
    }
    match o.cells {
        0 => String::new(),
        cells => format!(
            ", occlusion over {cells} cell(s) of {:.1} m, {} object(s){never} ({} KiB, {ms} ms)",
            o.cell_size,
            o.objects,
            o.bytes.div_ceil(1024),
        ),
    }
}

/// Bake lightmaps then probes with the scene's settings, and write the result.
/// Errors when the scene was never saved, during Play (the bakes write the scene
/// file, as a save does, #1340), or when a bake's own contract fails. The probes'
/// progress lines go to `say`; where every step is, and the total time, to `log`
/// (#1667).
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
    // A bake is no edit (#1631): the console line's Undo step leaves it out.
    log.plan(&Step::GENERATE);
    scene.unrecorded(|scene| {
        let lightmaps = bake_scene_lightmaps_logged(scene, Some(path), &settings, log)?;
        // Every second has a step (#1891): placing probes and saving are their own.
        log.enter(Step::ProbePlacement);
        let mut probes = start_probes(scene, path, nav)?;
        run_to_end(&mut probes, scene, say, log)?;
        log.enter(Step::Saving);
        probes.commit(scene)?;
        let saved = write_lighting(scene, path)?;
        // The PVS file `finish` writes is occlusion's own output, and counts there.
        let occlusion = bake_occlusion_logged(scene, log)?;
        let lighting = (lightmaps, probes.report(), saved);
        let report = finish(scene, path, lighting, occlusion)?;
        log.finish();
        Ok(report)
    })
}

/// The probe step after the lightmaps, as a job to step ([`LightingBake::step`]):
/// [`generate_lighting`] runs it to the end, the editor one step a frame. Refused in
/// Play like [`generate_lighting`]; the editor's background bake never reaches that,
/// since `bake::poll` holds a bake while the scene plays.
pub fn start_probes(
    scene: &mut Scene,
    path: &str,
    nav: Option<&NavigationGraph>,
) -> Result<LightingBake, String> {
    refuse_in_play(scene)?;
    start_lighting(scene, Some(path), nav, LightingBakeParams::default())
}

/// Write the applied lightmaps and the committed probes ([`LightingBake::commit`])
/// into the file at `path`, before the PVS bakes: what Generate Lighting baked is
/// on disk however its last step ends. The PVS file is not touched: [`finish`]
/// writes it once the PVS is baked, so a PVS step that never ends leaves the file
/// as it was, a newer one another process baked included (#1982).
pub fn write_lighting(scene: &Scene, path: &str) -> Result<LightingSave, String> {
    refuse_in_play(scene)?;
    save_lighting(scene, path)
}

/// The last step, once the lighting is written ([`write_lighting`]: `lightmaps` of
/// them, the probes' report, and whether it reached the file) and the occlusion
/// culling's PVS is baked (#1714): apply the PVS and write it beside the scene. A
/// scene file that could not take the lighting alone waits for a full save, which
/// writes the PVS too.
pub fn finish(
    scene: &mut Scene,
    path: &str,
    (lightmaps, probes, saved): (usize, LightingBakeReport, LightingSave),
    mut occlusion: BakedOcclusion,
) -> Result<GenerateReport, String> {
    refuse_in_play(scene)?;
    let (report, ms) = match saved {
        LightingSave::Written => {
            let o = apply_occlusion(scene, path, occlusion)?;
            (o.occlusion, o.ms)
        }
        LightingSave::NeedsSceneSave => {
            let (report, ms) = (occlusion.report, occlusion.ms);
            let cells = std::mem::take(&mut occlusion.cells);
            put_occlusion(scene, occlusion);
            // Nothing is written until the scene saves; the bake is reported now.
            super::report::occlusion::keep(scene, path, (&report, ms, &cells), None);
            (report, ms)
        }
    };
    Ok(GenerateReport {
        lightmaps,
        probes,
        occlusion: report,
        occlusion_ms: ms,
        saved,
    })
}

/// Bake Occlusion (#1958): only Generate Lighting's last step, as Unity bakes
/// occlusion from its own window. Bakes the open scene's PVS in line, applies it
/// and writes it beside the scene at `scene_path`, touching no lightmap or probe.
/// Refused as Generate Lighting is: during Play, while the editor's bake runs, and
/// for a scene never saved.
pub fn bake_occlusion_only(
    scene: &mut Scene,
    scene_path: Option<&str>,
    log: &mut BakeLog,
) -> Result<OcclusionOnlyReport, String> {
    refuse_in_play(scene)?;
    refuse_while_baking(scene)?;
    let path = scene_path.ok_or(UNSAVED_OCCLUSION)?;
    log.plan(&[Step::Occlusion]);
    let baked = bake_occlusion_logged(scene, log)?;
    let report = scene.unrecorded(|scene| apply_occlusion(scene, path, baked))?;
    log.finish();
    Ok(report)
}

/// What one Bake Occlusion did: the bake's report and its wall-clock milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcclusionOnlyReport {
    pub occlusion: OcclusionBakeReport,
    pub ms: u64,
}

impl OcclusionOnlyReport {
    /// One line for the console.
    pub fn summary(&self) -> String {
        match occlusion_summary(&self.occlusion, self.ms) {
            line if line.is_empty() => "Baked occlusion: nothing occludes, so nothing is \
                 occlusion-culled"
                .to_string(),
            // No PVS was kept, so there is nothing saved to speak of.
            line if self.occlusion.too_large => format!("Baked occlusion{line}"),
            line => format!("Baked occlusion{line}; saved"),
        }
    }
}

/// Apply a finished occlusion bake to `scene` and write it beside the scene file
/// at `path`: Bake Occlusion's last step, the script's and the editor's worker's.
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
    let cells = std::mem::take(&mut baked.cells);
    put_occlusion(scene, baked);
    let saving = std::time::Instant::now();
    crate::scene::lighting::occlusion::save_sidecar(scene, path)?;
    let saved = Some(saving.elapsed());
    let measured = (&report.occlusion, report.ms, &cells[..]);
    super::report::occlusion::keep(scene, path, measured, saved);
    Ok(report)
}

/// Put a finished bake's PVS on `scene`: the next save writes it
/// ([`Scene::put_baked_occlusion`]).
fn put_occlusion(scene: &mut Scene, baked: BakedOcclusion) {
    scene.put_baked_occlusion(baked.data);
}

/// The warning a bake gives when the scene file at `path` lacks, or holds
/// otherwise, a material the baked meshes draw (#1958,
/// [`unsaved_materials`](crate::scene::lighting::occlusion::unsaved_materials)):
/// the PVS fingerprints the live library, so the next load of the file would read
/// it as stale. `None` when the file agrees, or cannot be read.
pub fn unsaved_materials_warning(scene: &Scene, path: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let saved = crate::scene::parse_scene_text(&text, path).ok()?.materials;
    let keys = crate::scene::lighting::occlusion::unsaved_materials(scene, &saved);
    if keys.is_empty() {
        return None;
    }
    let keys: Vec<_> = keys.iter().map(|k| format!("{k:?}")).collect();
    Some(format!(
        "[Occlusion] material(s) {} are defined in memory but not as the scene file holds \
         them: save the scene so its materials are written, or the bake won't match the \
         next load",
        keys.join(", ")
    ))
}

/// Why a script's bake is refused while the editor's runs: the two would bake the same
/// probes and write the same files. The editor's bake is either of its buttons.
pub const BAKE_RUNNING: &str =
    "the editor's Generate Lighting or Bake Occlusion is already running; wait for it to finish";

/// A bake run in one call is refused while the editor's stepped one runs (#1602).
pub fn refuse_while_baking(scene: &Scene) -> Result<(), String> {
    match scene.lighting_bake_running {
        true => Err(BAKE_RUNNING.to_string()),
        false => Ok(()),
    }
}

/// Generate Lighting writes the scene file, so Play refuses it as it refuses Save.
fn refuse_in_play(scene: &Scene) -> Result<(), String> {
    match scene.is_playing() {
        true => Err(LIGHTING_IN_PLAY.to_string()),
        false => Ok(()),
    }
}

#[cfg(test)]
#[path = "pvs_only_tests.rs"]
mod pvs_only_tests;
#[cfg(test)]
#[path = "generate_lighting_tests.rs"]
mod tests;
