//! The navmesh bake (#1887, Unity's `NavMeshSurface.BuildNavMesh`): the full bake
//! `Navigation.Bake()` and the editor's *Bake Navigation* button run, which keeps the
//! result on the scene and writes `<scene>.navmesh` beside a saved one. A save writes
//! it again (`scene::nav_settings::save_navmesh`), so a scene baked unsaved, or saved
//! under another name, carries its navmesh to its file. Independent of Generate
//! Lighting, as in Unity.

use crate::navigation::NavigationGraph;
use crate::scene::nav_settings::{navmesh_path, save_navmesh};
use crate::scene::Scene;

/// Bake `scene`'s navmesh in full onto `nav`. In edit mode the result is also kept on
/// the scene and written beside `path`: `Some` names the file written, `None` means
/// nothing was (an unsaved scene, whose first save writes it; or Play, where the bake
/// only refreshes the live navmesh, as Unity's runtime `BuildNavMesh` does).
pub fn bake_navmesh(
    scene: &mut Scene,
    nav: &mut NavigationGraph,
    path: Option<&str>,
) -> Result<Option<String>, String> {
    if scene.is_playing() {
        nav.bake(scene);
        return Ok(None);
    }
    scene.navmesh = Some(nav.bake_to_bytes(scene).into());
    let Some(path) = path else {
        return Ok(None);
    };
    save_navmesh(scene, path)?;
    Ok(Some(navmesh_path(path).to_string_lossy().into_owned()))
}
