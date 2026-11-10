//! The navmesh bake (#1887, Unity's `NavMeshSurface.BuildNavMesh`): the full bake
//! `Navigation.Bake()` and the editor's *Bake Navigation* button run, which keeps the
//! result on the scene and writes `<scene>.navmesh` beside a saved one. A save writes
//! it when the file lacks it (`scene::nav_settings::save_navmesh`), so a scene baked
//! unsaved, or saved under another name, carries its navmesh to its file. Independent
//! of Generate Lighting, as in Unity. A bake that writes the file writes its report
//! beside it too (`report::navmesh`, #1980).

use super::report::{self, Stages};
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
    let mut stages = Stages::start();
    let bytes: std::sync::Arc<[u8]> = nav.bake_to_bytes(scene).into();
    stages.lap("bake");
    scene.navmesh.bytes = Some(bytes.clone());
    let Some(path) = path else {
        return Ok(None);
    };
    save_navmesh(scene, path)?;
    stages.lap("write");
    let file = navmesh_path(path);
    let written = report::bytes_under(&file);
    report::keep(
        path,
        report::navmesh::report(scene, nav, &stages, &bytes, written),
    );
    Ok(Some(file.to_string_lossy().into_owned()))
}
