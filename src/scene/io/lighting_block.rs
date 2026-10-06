//! Write a scene's baked lighting into its file without saving the rest (#832).
//!
//! Unity's Generate Lighting writes its result to a lighting data asset at once, so
//! the bake is kept without the user re-saving the scene, and unsaved edits stay
//! unsaved. rusty's baked lighting lives in the scene document (probe positions,
//! reflection-probe cubemap paths, the lightmap set, the bake settings) plus the SH
//! sidecar, so the equivalent is to rewrite exactly those fields of the file on disk
//! and leave every other byte's meaning alone.
//!
//! The file is re-read as a typed `SceneData`, so it is written back in the layout
//! [`save_to_file`](super::save_to_file) uses. That is only safe when the document
//! survives the round trip unchanged (a legacy scene that loading migrates would
//! not), so the round trip is checked first; when it fails nothing is written and
//! the caller is told to save the whole scene instead.

use serde_json::Value;

use super::read_scene_file;
use crate::scene::serialize::SceneData;
use crate::scene::Scene;

/// What [`save_lighting`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightingSave {
    /// The scene file now carries the lighting; nothing else in it changed.
    Written,
    /// The file could not be rewritten field by field (it is a legacy or hand-edited
    /// document loading would migrate); save the whole scene to keep the lighting.
    NeedsSceneSave,
}

/// Write `scene`'s lighting fields into the scene file at `path`, and its baked SH
/// into the sidecar beside it. Errors when the file cannot be read or written.
pub fn save_lighting(scene: &Scene, path: &str) -> Result<LightingSave, String> {
    let original: Value = parse(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)?;
    let mut data = read_scene_file(path)?;
    if to_value(&data)? != original {
        return Ok(LightingSave::NeedsSceneSave);
    }
    copy_lighting(scene, &mut data);
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("Failed to write scene file: {e}"))?;
    crate::scene::lighting::io::save_lighting_sidecar(scene, path)?;
    Ok(LightingSave::Written)
}

/// The fields a lighting bake writes: probe layout, reflection probes, lightmaps and
/// the settings they were baked with.
fn copy_lighting(scene: &Scene, data: &mut SceneData) {
    data.probes = scene.probes.clone();
    data.reflection_probes = scene.reflection_probes.clone();
    data.lightmaps = scene.lightmaps.clone();
    data.lighting_settings = scene.lighting_settings;
}

fn parse(json: &str) -> Result<Value, String> {
    serde_json::from_str(json).map_err(|e| format!("Failed to parse scene file: {e}"))
}

fn to_value(data: &SceneData) -> Result<Value, String> {
    serde_json::to_value(data).map_err(|e| format!("Failed to serialize scene: {e}"))
}

#[cfg(test)]
#[path = "lighting_block_tests.rs"]
mod tests;
