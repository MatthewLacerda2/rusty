//! Starting the scene inspector's bakes: Generate Lighting's lightmap step (the
//! button, or a script's `Lighting.GenerateAsync()` pressing it) and Bake
//! Occlusion's PVS on its worker, each said in the console and as the scene's
//! bake status when it cannot start.

use super::{ActiveBake, Step};
use crate::EditorUi;
use rusty_dev::bake::lightmap::LightmapBakeJob;
use rusty_dev::bake::occlusion::{reach, unsaved_materials_warning, OcclusionBakeJob};
use rusty_lighting::lightmap::BakeView;
use rusty_navigation::NavigationGraph;
use rusty_scene::lighting::bake_status::{BakeProgress, BakeStatus};
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

/// Start the lightmap step of the live scene in the background. It writes to the
/// open scene's file, never to whichever scene asset the card shows.
pub(super) fn start(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    let Some(path) = editor.current_scene_path() else {
        return not_started(
            scene,
            console,
            "Generate Lighting failed: save the scene first".into(),
        );
    };
    let settings = scene.lighting_settings.lightmaps;
    match LightmapBakeJob::start(scene, Some(&path), settings, Some(scene_view(editor))) {
        Ok(job) => {
            console.info("Generating lighting in the background…".into());
            warn_unsaved(scene, &path, console);
            editor.dev.lightmap_bake = Some(ActiveBake {
                step: Step::Lightmaps(job),
                scene: scene.id(),
                path: Some(path),
            });
            scene.lighting_bake_running = true;
            scene.lighting_bake_status = BakeStatus::Running(BakeProgress::starting("lighting"));
        }
        Err(e) => not_started(scene, console, format!("Generate Lighting failed: {e}")),
    }
}

fn not_started(scene: &mut Scene, console: &mut ConsoleLogs, line: String) {
    console.error(line.clone());
    scene.lighting_bake_status = BakeStatus::Refused(line);
}

/// A script's `Lighting.GenerateAsync()`: press the button for it, once.
pub(super) fn take_request(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    if !std::mem::take(&mut scene.lighting_bake_requested) {
        return;
    }
    if editor.dev.lightmap_bake.is_none() {
        return start(editor, scene, console);
    }
    let line = "Lighting.GenerateAsync: a bake was already running".to_string();
    console.error(line.clone());
    scene.lighting_bake_status = BakeStatus::Refused(line);
}

fn scene_view(editor: &EditorUi) -> BakeView {
    let camera = editor.scene_view.camera.camera();
    BakeView {
        eye: camera.position,
        // A 16:9 lens whatever the panel's shape: the view only orders which texels go first.
        view_proj: camera.projection_matrix(16.0 / 9.0) * camera.view_matrix(),
    }
}

pub(super) fn start_occlusion(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    nav: &NavigationGraph,
) {
    let Some(path) = editor.current_scene_path() else {
        return not_started(
            scene,
            console,
            "Bake Occlusion failed: save the scene first".into(),
        );
    };
    match OcclusionBakeJob::start(scene, reach(scene, Some(nav))) {
        Ok(job) => {
            console.info("Baking occlusion culling in the background…".into());
            warn_unsaved(scene, &path, console);
            editor.dev.lightmap_bake = Some(ActiveBake {
                step: Step::OcclusionOnly {
                    job,
                    path: path.clone(),
                },
                scene: scene.id(),
                path: Some(path),
            });
            scene.lighting_bake_running = true;
            scene.lighting_bake_status = BakeStatus::Running(BakeProgress::starting("occlusion"));
        }
        Err(e) => not_started(scene, console, format!("Bake Occlusion failed: {e}")),
    }
}

fn warn_unsaved(scene: &Scene, path: &str, console: &mut ConsoleLogs) {
    if let Some(line) = unsaved_materials_warning(scene, path) {
        console.warn(line);
    }
}
