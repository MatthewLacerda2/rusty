//! A model's humanoid settings changed (#1747): the skinned meshes whose clips
//! that changes take their new clips at once.

use std::collections::BTreeMap;
use std::path::Path;

use crate::asset::{self, humanoid::HumanoidSettings, AssetRef};
use crate::scene::Scene;

/// Write `settings` as the humanoid settings of the model at `path` and give
/// every skinned mesh in `scene` whose clips it shapes (instanced from `path`,
/// or from a model borrowing clips from it) its re-imported clips. Its rig,
/// bones and geometry are left as they are: only clips change. A setting the
/// sidecar already holds changes nothing. The one verb `Assets.SetHumanoid` runs.
/// Returns how many meshes took new clips.
pub fn set_humanoid(
    scene: &mut Scene,
    path: &str,
    settings: HumanoidSettings,
) -> Result<usize, String> {
    let source = Path::new(path);
    let mut stored = asset::sidecar::load(source).map_err(|e| e.to_string())?;
    if stored.humanoid == settings {
        return Ok(0);
    }
    stored.humanoid = settings;
    asset::sidecar::save(source, &stored).map_err(|e| e.to_string())?;
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
        let hit = *shaped.entry(file.clone()).or_insert_with(|| {
            file == path
                || asset::sidecar::load(Path::new(&file))
                    .is_ok_and(|s| s.humanoid.clips_from.iter().any(|f| f == path))
        });
        if !hit {
            continue;
        }
        let Ok(import) = asset::imported(Path::new(&file)) else {
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
