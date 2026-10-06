//! src/dev/generate_lighting.rs — Generate Lighting (#832): every bake a scene has,
//! in order, with the result written.
//!
//! Unity 5's one button: lightmaps first (`lightmap_bake`, #438), then light and
//! reflection probes (`lighting_bake`, #246), both with the scene's own
//! [`LightingSettings`](crate::scene::lighting::LightingSettings), and finally the
//! lighting written into the scene file ([`save_lighting`]) so nobody has to
//! remember to save. The individual bakes stay for scripts that want one of them.
//!
//! The editor runs the lightmap step on a worker (#808) and calls [`finish`] once
//! it lands; `Lighting.Generate()` calls [`generate_lighting`], which runs the same
//! two steps in line. Same path, so the two never drift.

use super::lighting_bake::{bake_lighting, LightingBakeParams, LightingBakeReport};
use super::lightmap_bake::bake_scene_lightmaps;
use crate::navigation::NavigationGraph;
use crate::scene::{save_lighting, LightingSave, Scene};

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
/// Errors when the scene was never saved or a bake's own contract fails.
pub fn generate_lighting(
    scene: &mut Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
) -> Result<GenerateReport, String> {
    let path = scene_path.ok_or(UNSAVED)?;
    let settings = scene.lighting_settings.lightmaps;
    let lightmaps = bake_scene_lightmaps(scene, Some(path), &settings)?;
    finish(scene, path, nav, lightmaps)
}

/// The steps after the lightmaps (already applied to `scene`, `lightmaps` of them):
/// the probe bake, then writing the lighting into the file at `path`.
pub fn finish(
    scene: &mut Scene,
    path: &str,
    nav: Option<&NavigationGraph>,
    lightmaps: usize,
) -> Result<GenerateReport, String> {
    let probes = bake_lighting(scene, Some(path), nav, LightingBakeParams::default())?;
    let saved = save_lighting(scene, path)?;
    Ok(GenerateReport {
        lightmaps,
        probes,
        saved,
    })
}

#[cfg(test)]
#[path = "generate_lighting_tests.rs"]
mod tests;
