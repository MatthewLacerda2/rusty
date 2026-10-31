//! src/dev/bake/generate_lighting.rs — Generate Lighting (#832): every bake a scene has,
//! in order, with the result written.
//!
//! Unity 5's one button: lightmaps first (`lightmap_bake`, #438), then light and
//! reflection probes (`lighting_bake`, #246), both with the scene's own
//! [`LightingSettings`](crate::scene::lighting::LightingSettings), and finally the
//! lighting written into the scene file ([`save_lighting`]) so nobody has to
//! remember to save. The individual bakes stay for scripts that want one of them.
//!
//! The editor runs the lightmap step on a worker (#808), then steps the probe bake
//! ([`start_probes`]) one frame at a time (#1602, Unity's `BakeAsync`) and calls
//! [`finish`]; `Lighting.Generate()` calls [`generate_lighting`], which runs the same
//! steps in line (Unity's `Lightmapping.Bake`). Same path, so the two never drift.

use crate::dev::bake::lighting::{start_lighting, LightingBakeParams, LightingBakeReport};
use crate::dev::bake::lightmap::bake_scene_lightmaps;
use crate::dev::bake::steps::LightingBake;
use crate::navigation::NavigationGraph;
use crate::scene::{save_lighting, LightingSave, Scene, LIGHTING_IN_PLAY};

/// Why Generate Lighting cannot start: the bakes write beside the scene file.
pub const UNSAVED: &str = "save the scene before generating lighting";

/// What one Generate Lighting did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenerateReport {
    /// Lightmaps written (0 when no static mesh has a lightmap UV).
    pub lightmaps: usize,
    /// The probe step's placement and bake.
    pub probes: LightingBakeReport,
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
        line.push_str(match self.saved {
            LightingSave::Written => "; saved",
            LightingSave::NeedsSceneSave => "; save the scene to keep it",
        });
        line
    }
}

/// Bake lightmaps then probes with the scene's settings, and write the result.
/// Errors when the scene was never saved, during Play (the bakes write the scene
/// file, as a save does, #1340), or when a bake's own contract fails. The probes'
/// progress lines go to `say`.
pub fn generate_lighting(
    scene: &mut Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
    say: &mut dyn FnMut(String),
) -> Result<GenerateReport, String> {
    refuse_in_play(scene)?;
    refuse_while_baking(scene)?;
    let path = scene_path.ok_or(UNSAVED)?;
    let settings = scene.lighting_settings.lightmaps;
    // A bake is no edit (#1631): the console line's Undo step leaves it out.
    scene.unrecorded(|scene| {
        let lightmaps = bake_scene_lightmaps(scene, Some(path), &settings)?;
        let mut probes = start_probes(scene, path, nav)?;
        while !probes.step(scene, say)? {}
        probes.commit(scene)?;
        finish(scene, path, lightmaps, probes.report())
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

/// The last step, once lightmaps (`lightmaps` of them, already applied to `scene`)
/// and probes are baked and committed ([`LightingBake::commit`]): write the lighting
/// into the file at `path`.
pub fn finish(
    scene: &mut Scene,
    path: &str,
    lightmaps: usize,
    probes: LightingBakeReport,
) -> Result<GenerateReport, String> {
    refuse_in_play(scene)?;
    let saved = save_lighting(scene, path)?;
    Ok(GenerateReport {
        lightmaps,
        probes,
        saved,
    })
}

/// Why a script's bake is refused while the editor's runs: the two would bake the same
/// probes and write the same files.
pub const BAKE_RUNNING: &str =
    "Generate Lighting is already running in the editor; wait for it to finish";

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
#[path = "generate_lighting_tests.rs"]
mod tests;
