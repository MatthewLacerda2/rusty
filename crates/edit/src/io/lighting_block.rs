use serde_json::Value;

use super::parse_scene_text;
use rusty_scene::serialize::SceneData;
use rusty_scene::Scene;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightingSave {
    Written,
    /// The file could not be rewritten field by field (a hand-edited document loading would
    /// change); save the whole scene to keep the lighting.
    NeedsSceneSave,
}

/// Writes only the lighting fields and SH sidecar, as Unity's Generate Lighting: unsaved
/// edits stay unsaved.
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
    rusty_core::fs::write_atomic(std::path::Path::new(path), json.as_bytes())
        .map_err(|e| format!("Failed to write scene file: {e}"))?;
    // An outside edit the follow has yet to apply is left off the disk-text record: the follow
    // reloads it, and no Save writes over it.
    if scene.is_disk_text(path, &text) {
        scene.record_disk_text(path, json);
    } else {
        // The recorded disk text takes the lighting too: the reload finds no unsaved edit.
        add_lighting_to_record(scene, path);
    }
    rusty_scene::lighting::io::save_sh_sidecar(scene, path)?;
    Ok(LightingSave::Written)
}

fn add_lighting_to_record(scene: &Scene, path: &str) {
    let Some(mut data) = scene
        .disk_text_at(path)
        .and_then(|old| parse_scene_text(&old, path).ok())
    else {
        return;
    };
    copy_lighting(scene, &mut data);
    if let Ok(json) = serde_json::to_string_pretty(&data) {
        scene.record_disk_text(path, json);
    }
}

fn copy_lighting(scene: &Scene, data: &mut SceneData) {
    data.probes = scene.probes.clone();
    data.reflection_probes = scene.reflection_probes.clone();
    data.lightmaps = scene.lightmaps.saved().clone();
    data.lighting_settings = scene.lighting_settings;
}

fn parse(json: &str) -> Result<Value, String> {
    serde_json::from_str(json).map_err(|e| format!("Failed to parse scene file: {e}"))
}

/// Through the text, not `serde_json::to_value`: an `f32` written as its shortest decimal
/// reads back as another `f64` than the `f32` widened.
fn to_value(data: &SceneData) -> Result<Value, String> {
    let json =
        serde_json::to_string(data).map_err(|e| format!("Failed to serialize scene: {e}"))?;
    parse(&json)
}

#[cfg(test)]
#[path = "lighting_block_tests.rs"]
mod tests;
