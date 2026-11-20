//! `make bench-scenes`' editor round (#2314): the editor's own costs on a
//! project scene, which the game rounds never draw.
//!
//! The scene is opened as the editor opens it, in edit mode and not standalone
//! ([`editor_world`]), so Play takes its edit snapshot and Stop restores it. Each
//! run draws [`Round::warmup`] frames, then [`Round::frames`] measured ones, of the
//! editor at the bench's size: the Scene tab, its default gizmos, the scene's first
//! camera selected, the Scene view posed at the scene's first game viewpoint
//! (`dev::bench::scenes::views::pick`: its first camera, or the fallbacks after
//! it), with its pitch; the Scene view keeps its own lens, as the editor's does).
//! A frame is the windowed editor's outside Play: the world's edit frame
//! (`GameWorld::edit_frame`, timed as `edit_ms`), then the capture's
//! ([`super::capture`]): `EditorUi::draw` and egui's tessellation (timed as
//! `ui_ms`), the viewport's render and egui's paint (the renderer's stages and GPU
//! passes), waited on. Then one Play entry, one Stop (to the first edit frame
//! drawn, a frame that probes nothing) and one save each run, into the meter
//! (`dev::bench::scenes::editor::Meter`), which returns plain rows.
//!
//! **The project is read-only.** The save goes to `Round::save_dir` and is
//! deleted after; the world's store is pathless, so Stop's flush writes nothing;
//! the process is marked standalone for the asset gate (as the bench's harness
//! is), so Play's texture refresh compresses nothing into the project, which
//! leaves that refresh out of `play_enter_ms`.
//!
//! **The save is the editor's Ctrl+S of the loaded scene**, not a Save As: the
//! scene's bake files (`<scene>.navmesh`, `<scene>.occlusion`) are copied beside
//! the save target first and the world's record of them handed to it
//! (`BakeFiles::copied`), so the save keeps them, as a save over the loaded scene
//! does, instead of rewriting every byte a Save As to a new path writes.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use rusty_app::{GameWorld, ProjectSource};
use rusty_core::input::InputState;
use rusty_core::project::{BootPlan, ProjectSettings};
use rusty_dev::bench::scenes::editor::{EditorRound, Meter};
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

mod editor;

use editor::Editor;

/// What the round measures, and where its save goes.
#[derive(Clone, Debug)]
pub struct Round {
    pub size: (u32, u32),
    pub warmup: u32,
    pub frames: u32,
    pub runs: u32,
    /// Build every lazy pipeline (the editor's overlays too) first, off the
    /// clock, as the editor's window does.
    pub warm: bool,
    /// The folder the save writes into, outside the project; removed after.
    pub save_dir: PathBuf,
}

/// The scene at `scene_path` (under the working directory, the project's root)
/// in an edit-mode editor world on `settings`, unbound: what the editor opens,
/// minus the window, the project's store and the demo recording. `Err` with the
/// console's lines when the scene does not load.
pub fn editor_world(scene_path: &str, settings: ProjectSettings) -> Result<GameWorld, String> {
    rusty_asset::convert::mark_standalone();
    let (mut scene, mut console) = (Scene::new(), ConsoleLogs::new());
    let plan = BootPlan::of(scene_path);
    let Some(loaded) = rusty_app::load_boot_scene(&mut scene, &mut console, &plan, false) else {
        let said: Vec<&str> = console.since(0).map(|(m, _)| m.as_str()).collect();
        return Err(format!("{scene_path} did not load: {}", said.join("; ")));
    };
    let nav = NavigationGraph::from_scene(&scene);
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(console)),
    );
    rusty_dev::stats::install(&mut game);
    rusty_dev::lua_surface::install_api();
    *game.script_manager().scene_path_cell().borrow_mut() = Some(loaded);
    let init = game.init_edit_runtime();
    game.report_lua_init(init);
    game.open_project(ProjectSource::ReadOnly(settings));
    Ok(game)
}

/// Run the round over `game` (an [`editor_world`]): `Ok(None)` with no adapter
/// (no rows are made up), `Err` when a save fails.
pub fn round(game: &mut GameWorld, opts: &Round) -> Result<Option<EditorRound>, String> {
    let Some(mut editor) = Editor::new(game, opts) else {
        rusty_dev::capture::no_adapter("bench the editor")?;
        return Ok(None);
    };
    let mut meter = Meter::new();
    let _ = std::fs::create_dir_all(&opts.save_dir);
    let saved = (1..=opts.runs.max(1)).try_for_each(|_| {
        for _ in 0..opts.warmup {
            editor.frame(game);
        }
        for _ in 0..opts.frames {
            let (ui, rendered, gpu) = editor.frame(game);
            meter.frame(ui, rendered.as_ref(), gpu);
        }
        play_stop_save(&mut editor, game, &mut meter, opts)?;
        meter.end_run();
        Ok::<_, String>(())
    });
    let _ = std::fs::remove_dir_all(&opts.save_dir);
    saved?;
    let mut round = meter.finish();
    if !round.rows.iter().any(|r| r.metric == "editor.renderer_ms") {
        let (w, h) = opts.size;
        round.warnings.push(format!(
            "editor round: the Scene view drew nothing at {w}x{h} (the editor's panels \
             leave it no room): no editor.render or editor.gpu rows; bench at a larger RES"
        ));
    }
    Ok(Some(round))
}

/// One Play entry, one Stop and one save, each a single sample of the run.
/// `play_rss_mb` is the first run's alone: the pages a Stop frees stay resident,
/// and the next run's Play re-uses them.
fn play_stop_save(
    editor: &mut Editor,
    game: &mut GameWorld,
    meter: &mut Meter,
    opts: &Round,
) -> Result<(), String> {
    let dt = rusty_time::FIXED_DELTA_TIME;
    let rss_before = rusty_dev::memory::rss_now_bytes();
    let start = Instant::now();
    game.set_playing(true);
    game.tick(dt); // the transition: the snapshot, `enter_play`, then the first FixedUpdate
    meter.single("play_enter_ms", ms_since(start));
    let rss_after = rusty_dev::memory::rss_now_bytes();
    if let (Some(before), Some(after), true) = (rss_before, rss_after, meter.first_run()) {
        let grew = after.saturating_sub(before) as f64 / (1024.0 * 1024.0);
        meter.single("play_rss_mb", (grew * 10.0).round() / 10.0);
    }
    let start = Instant::now();
    game.set_playing(false);
    game.tick(dt); // `exit_play`: the restore and the rehydrate
    editor.draw(game);
    meter.single("play_stop_ms", ms_since(start));
    let path = opts.save_dir.join("editor-save.scene");
    let path = path.to_string_lossy();
    let loaded = game.script_manager().scene_path_cell().borrow().clone();
    let scene = game.scene().borrow();
    if let Some(loaded) = loaded {
        stage_save(&scene, &loaded, &path)?;
    }
    let start = Instant::now();
    rusty_edit::save_to_file(&scene, &path)?;
    meter.single("save_ms", ms_since(start));
    Ok(())
}

/// Make a save to `save` the save of the scene loaded from `loaded`: its bake
/// files copied beside `save` (one it has none of removed there), each with the
/// world's record of it ([`rusty_scene::io::BakeFiles::copied`]).
pub(super) fn stage_save(scene: &Scene, loaded: &str, save: &str) -> Result<(), String> {
    let beside = [
        rusty_scene::nav_settings::navmesh_path,
        rusty_scene::lighting::occlusion::occlusion_path,
    ];
    for sidecar in beside {
        let (from, to) = (sidecar(loaded), sidecar(save));
        let copied = match from.exists() {
            true => std::fs::copy(&from, &to).map(|_| ()),
            false if to.exists() => std::fs::remove_file(&to),
            false => Ok(()),
        };
        copied.map_err(|e| format!("{}: {e}", to.display()))?;
        scene.bake_files().copied(&from, &to);
    }
    Ok(())
}

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "view_tests.rs"]
mod view_tests;
