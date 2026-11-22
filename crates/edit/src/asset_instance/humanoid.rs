use std::collections::BTreeMap;
use std::path::Path;

use rusty_asset::{humanoid::HumanoidSettings, AssetRef};
use rusty_core::fs::written::Stale;
use rusty_scene::Scene;

pub fn set_humanoid(
    scene: &mut Scene,
    path: &str,
    settings: HumanoidSettings,
) -> Result<usize, String> {
    let source = Path::new(path);
    let mut stored = rusty_asset::sidecar::load(source).map_err(|e| e.to_string())?;
    if stored.humanoid == settings {
        return Ok(0);
    }
    stored.humanoid = settings;
    rusty_asset::sidecar::save(source, &stored).map_err(|e| e.to_string())?;
    let changed = Stale::new(vec![path.to_string()]);
    let mut shaped: BTreeMap<String, bool> = BTreeMap::new();
    let mut count = 0;
    for id in scene.world.ids_with_mesh() {
        let Some(reference) = scene.world.mesh(id).and_then(|m| {
            m.skin.as_ref()?;
            AssetRef::parse(m.asset_ref.as_deref()?).ok()
        }) else {
            continue;
        };
        let file = reference.path.clone();
        let hit = *(shaped.entry(file.clone()))
            .or_insert_with_key(|file| rusty_asset::humanoid::clips_follow(file, &changed));
        if !hit {
            continue;
        }
        let Ok(import) = rusty_asset::imported(Path::new(&file)) else {
            continue;
        };
        let Some(sub) = import.sub_mesh(&reference.sub_object) else {
            continue;
        };
        if let Some(mut mesh) = scene.world.mesh_mut(id) {
            mesh.clips = sub.clips.clone();
            count += 1;
        }
    }
    Ok(count)
}
