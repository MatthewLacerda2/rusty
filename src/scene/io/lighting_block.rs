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
//! survives the round trip unchanged (a hand-edited one, with unknown keys or
//! out-of-range values loading drops or fixes, would not), so the round trip is checked first; when it fails nothing is written and
//! the caller is told to save the whole scene instead.

use serde_json::Value;

use super::parse_scene_text;
use crate::scene::serialize::SceneData;
use crate::scene::Scene;

/// What [`save_lighting`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightingSave {
    /// The scene file now carries the lighting; nothing else in it changed.
    Written,
    /// The file could not be rewritten field by field (it is a hand-edited document
    /// loading would change); save the whole scene to keep the lighting.
    NeedsSceneSave,
}

/// Write `scene`'s lighting fields into the scene file at `path`, and its baked SH
/// into the sidecar beside it. Errors when the file cannot be read or written, and
/// during Play, when the World holds the play state (#1340).
pub fn save_lighting(scene: &Scene, path: &str) -> Result<LightingSave, String> {
    if scene.is_playing() {
        return Err(super::LIGHTING_IN_PLAY.to_string());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut data = parse_scene_text(&text, path)?;
    if to_value(&data)? != parse(&text)? {
        return Ok(LightingSave::NeedsSceneSave);
    }
    copy_lighting(scene, &mut data);
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    // lint: allow(asset-write) a scene, prefab or lighting document, read whole on load
    std::fs::write(path, &json).map_err(|e| format!("Failed to write scene file: {e}"))?;
    // Only when the file held the engine's own text: an outside edit the follow has
    // not applied yet stays unrecorded, so the follow reloads it (edit + lighting)
    // instead of the next Save writing over it (#1374). The old record takes the
    // lighting instead, so that reload does not count the bake as an unsaved edit.
    if scene.is_disk_text(path, &text) {
        scene.disk_text.record(path, json);
    } else {
        add_lighting_to_record(scene, path);
    }
    crate::scene::lighting::io::save_lighting_sidecar(scene, path)?;
    Ok(LightingSave::Written)
}

/// Write `scene`'s lighting into the engine's record of `path`, when it has one.
fn add_lighting_to_record(scene: &Scene, path: &str) {
    let Some(mut data) = scene
        .disk_text
        .text(path)
        .and_then(|old| parse_scene_text(&old, path).ok())
    else {
        return;
    };
    copy_lighting(scene, &mut data);
    if let Ok(json) = serde_json::to_string_pretty(&data) {
        scene.disk_text.record(path, json);
    }
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

/// `data` as the file would hold it. Through the text, not `serde_json::to_value`:
/// an `f32` is written as its shortest decimal, which reads back as a different `f64`
/// than the `f32` widened directly.
fn to_value(data: &SceneData) -> Result<Value, String> {
    let json =
        serde_json::to_string(data).map_err(|e| format!("Failed to serialize scene: {e}"))?;
    parse(&json)
}

#[cfg(test)]
#[path = "lighting_block_tests.rs"]
mod tests;
