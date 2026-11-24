//! One queued scene's bake: navmesh, Generate Lighting, save, on a scene read beside the
//! live one, so the caller's World and its running script are never replaced.

use std::path::Path;

use super::run::Outcome;
use crate::bake::generate_lighting::generate_lighting;
use crate::bake::navmesh::bake_navmesh;
use crate::bake::progress::BakeLog;
use rusty_edit::LightingSave;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;

pub fn bake_scene(
    root: &Path,
    path: &str,
    say: &mut dyn FnMut(String),
    log: &mut BakeLog,
) -> Outcome {
    match bake(&at(root, path), say, log) {
        Ok(summary) => Outcome::Done(summary),
        Err(why) => Outcome::Failed(why),
    }
}

pub fn at(root: &Path, path: &str) -> String {
    match root == Path::new(".") {
        true => path.to_string(),
        false => root.join(path).to_string_lossy().into_owned(),
    }
}

/// It keeps a navmesh, or an agent walks it.
pub fn has_navigation(scene: &Scene) -> bool {
    scene.navmesh.bytes.is_some() || !scene.world.ids_with_nav_agent().is_empty()
}

fn bake(file: &str, say: &mut dyn FnMut(String), log: &mut BakeLog) -> Result<String, String> {
    if !Path::new(file).is_file() {
        return Err(format!("no scene file at '{file}'"));
    }
    let mut scene = Scene::new();
    match rusty_edit::load_from_file(&mut scene, file) {
        Ok(()) => {}
        Err(e) if e.replaced_world() => say(format!("{file}: {e}")),
        Err(e) => return Err(e.to_string()),
    }
    // First: the probes are placed on its floor.
    let mut nav = NavigationGraph::from_scene(&scene);
    let navmesh = match has_navigation(&scene) {
        true => bake_navmesh(&mut scene, &mut nav, Some(file))?.is_some(),
        false => false,
    };
    let report = generate_lighting(&mut scene, Some(file), Some(&nav), say, log)?;
    if report.saved == LightingSave::NeedsSceneSave {
        rusty_edit::save_to_file(&scene, file)?;
    }
    let lit = report.summary();
    Ok(match navmesh {
        true => format!("navmesh baked; {lit}"),
        false => lit,
    })
}
