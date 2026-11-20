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
            scene.lighting_bake_running = true; // a script's bake waits from now on
            scene.lighting_bake_status = BakeStatus::Running(BakeProgress::starting("lighting"));
        }
        Err(e) => not_started(scene, console, format!("Generate Lighting failed: {e}")),
    }
}

/// Say a bake never started because `line`, in the console and as the scene's
/// status (`Lighting.GetBakeStatus`, #1999).
fn not_started(scene: &mut Scene, console: &mut ConsoleLogs, line: String) {
    console.error(line.clone());
    scene.lighting_bake_status = BakeStatus::Refused(line);
}

/// A script's `Lighting.GenerateAsync()` (#1999): press the button for it, once.
/// The verb refused what the button refuses when it was called, Play is refused
/// while it waits (`Scene::play_refusal`, #2042), and File ▸ New and a load drop
/// it (`Scene::drop_bake_request`); a button pressed since refuses it here.
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

/// What the Scene view's camera sees, for the bake's Prioritize View (#1970). The
/// lens at a 16:9 aspect: a little wider or narrower than the panel only moves
/// which texels go first.
fn scene_view(editor: &EditorUi) -> BakeView {
    let camera = editor.scene_view.camera.camera();
    BakeView {
        eye: camera.position,
        view_proj: camera.projection_matrix(16.0 / 9.0) * camera.view_matrix(),
    }
}

/// Start Bake Occlusion (#1958) of the live scene on a worker: only Generate
/// Lighting's last step, written beside the open scene's file. It samples the cells
/// a camera can be in over `nav` (`occlusion::reach`, #2328).
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

/// Say when the scene file lacks a material the PVS will read (#1958): the next
/// load of the file would read the bake as stale until the scene is saved.
fn warn_unsaved(scene: &Scene, path: &str, console: &mut ConsoleLogs) {
    if let Some(line) = unsaved_materials_warning(scene, path) {
        console.warn(line);
    }
}
