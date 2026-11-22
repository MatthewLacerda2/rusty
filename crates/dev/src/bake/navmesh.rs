use super::report::{self, Stages};
use rusty_navigation::NavigationGraph;
use rusty_scene::nav_settings::{navmesh_path, save_navmesh};
use rusty_scene::Scene;

/// `None`: nothing written, for an unsaved scene or in Play, which refreshes the live navmesh.
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
    // Marked baked: the save writes it even over a newer file another process baked.
    scene.put_baked_navmesh(bytes.clone());
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
