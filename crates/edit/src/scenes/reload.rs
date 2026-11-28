//! The editor's follow with several scenes loaded: one file changed on disk. The World is built
//! again in load order, the changed scene from its file and every other from its live document,
//! so their unsaved edits stay (and stay unsaved). The active scene stays the active one.

use crate::io::LoadError;
use rusty_scene::rehydrate::RehydratedScene;
use rusty_scene::serialize::SceneData;
use rusty_scene::Scene;

use crate::load::prepare::{PreparedScene, SceneSidecars};

struct Part {
    path: Option<String>,
    data: SceneData,
    /// What the file held when the engine last read or wrote it.
    text: Option<String>,
}

/// Each loaded scene's document, taken before the World is built again.
fn parts(scene: &Scene, handles: &[u32], changed: &str, fresh: (String, SceneData)) -> Vec<Part> {
    let mut fresh = Some(fresh);
    let mut parts = Vec::with_capacity(handles.len());
    for &handle in handles {
        let path = scene.loaded.path_of(handle).map(str::to_string);
        if path.as_deref() == Some(changed) {
            if let Some((text, data)) = fresh.take() {
                let text = Some(text);
                parts.push(Part { path, data, text });
                continue;
            }
        }
        let Some(document) = super::document::document(scene, handle) else {
            continue;
        };
        let mut data = document.data;
        crate::io::relative_scripts(&mut data);
        let text = path.as_deref().and_then(|p| scene.disk_text_at(p));
        parts.push(Part { path, data, text });
    }
    parts
}

/// `changed` is a loaded scene's path, `text` and `data` what its file now holds.
pub fn reload_loaded(
    scene: &mut Scene,
    changed: &str,
    text: String,
    data: SceneData,
) -> Result<(), LoadError> {
    let handles = scene.loaded.handles();
    let active = scene.loaded.active();
    let active_at = handles.iter().position(|&h| h == active).unwrap_or(0);
    let parts = parts(scene, &handles, changed, (text, data));
    let mut parts = parts.into_iter();
    let Some(first) = parts.next() else {
        return Ok(());
    };
    let base = first.path.clone();
    let loaded = match &first.path {
        Some(path) => {
            let text = first.text.unwrap_or_default();
            crate::io::load_rehydrated(scene, path, text, RehydratedScene::new(first.data))
        }
        None => {
            crate::serialize::apply_scene_data(scene, first.data);
            Ok(())
        }
    };
    let mut joined = vec![0];
    for part in parts {
        let Some(path) = part.path else { continue };
        let document = RehydratedScene::new(part.data);
        let prepared = PreparedScene::new(path.clone(), document, SceneSidecars::read(&path));
        let (handle, _) = super::join_additive(scene, base.clone(), prepared);
        joined.push(handle);
        if let Some(text) = part.text {
            scene.record_disk_text_also(&path, text);
        }
    }
    if let Some(&handle) = joined.get(active_at) {
        scene.set_active_scene(handle);
    }
    loaded
}
