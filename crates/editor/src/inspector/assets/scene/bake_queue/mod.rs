//! The editor's bake queue: each scene opened in turn and Generate Lighting's own job run
//! on it, then the first scene opened again.

use std::path::PathBuf;
use std::time::Instant;

use crate::EditorUi;
use rusty_dev::bake::navmesh::bake_navmesh;
use rusty_dev::bake::queue::live::UNSAVED;
use rusty_dev::bake::queue::scene::{at, has_navigation};
use rusty_dev::bake::queue::{Outcome, Resumed, Runner};
use rusty_navigation::NavigationGraph;
use rusty_scene::lighting::bake_status::{BakeStatus, QueueSpot};
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

mod card;
pub use card::draw;

pub struct EditorQueue {
    runner: Runner,
    origin: String,
    current: Option<(usize, String)>,
}

#[derive(Default)]
pub struct QueueState {
    /// Empty: the working directory.
    pub root: PathBuf,
    pub running: Option<EditorQueue>,
    pub(super) seen: Option<(Instant, Option<String>)>,
}

type Ctx<'a> = (&'a mut Scene, &'a mut ConsoleLogs, &'a mut NavigationGraph);

/// Before the bake's poll, which starts what it asks for.
pub fn step(
    editor: &mut EditorUi,
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    nav: &mut NavigationGraph,
) {
    take_request(editor, scene, console);
    if editor.dev.bake_queue.running.is_none() || scene.is_playing() {
        return;
    }
    if editor.dev.lightmap_bake.is_none() {
        match settle(editor, (scene, console, nav)) {
            Ok(true) => {}
            Ok(false) => return end(editor, scene, console, None),
            Err(e) => return end(editor, scene, console, Some(e)),
        }
    }
    put_spot(editor, scene);
}

/// After the bake's poll: with no job asking for frames, an idle window still settles
/// the scene in hand.
pub fn wake(editor: &EditorUi, ctx: &egui::Context, scene: &Scene) {
    let queue = &editor.dev.bake_queue.running;
    if queue.is_some() && editor.dev.lightmap_bake.is_none() && !scene.is_playing() {
        ctx.request_repaint();
        rusty_core::wake::frame();
    }
}

fn take_request(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs) {
    if !std::mem::take(&mut scene.lighting_queue_requested) {
        return;
    }
    let refuse = |scene: &mut Scene, console: &mut ConsoleLogs, why: String| {
        let line = format!("Bake queue not started: {why}");
        console.error(line.clone());
        scene.lighting_bake_status = BakeStatus::Refused(line);
    };
    if editor.dev.bake_queue.running.is_some() || editor.dev.lightmap_bake.is_some() {
        return refuse(scene, console, "a bake is already running".into());
    }
    let Some(origin) = editor.current_scene_path() else {
        return refuse(scene, console, "save the scene first".into());
    };
    if editor.unsaved_against_file(scene) {
        return refuse(scene, console, UNSAVED.into());
    }
    let runner = match Runner::resume(&editor.dev.bake_queue.root) {
        Ok(Resumed::Runner(runner)) => runner,
        Ok(Resumed::Nothing(why)) => return refuse(scene, console, why),
        Err(why) => return refuse(scene, console, why),
    };
    let n = runner.queue().scenes.len();
    console.info(format!("Baking a queue of {n} scene(s) in the background…"));
    editor.dev.bake_queue.running = Some(EditorQueue {
        runner,
        origin,
        current: None,
    });
}

/// `Ok(false)` once no scene is left.
fn settle(editor: &mut EditorUi, (scene, console, nav): Ctx) -> Result<bool, String> {
    if let Some((index, file)) = in_hand(editor) {
        let outcome = match &scene.lighting_bake_status {
            BakeStatus::Requested | BakeStatus::Running(_) => return Ok(true),
            BakeStatus::Done(m) => Outcome::Done(m.clone()),
            BakeStatus::Failed(m) | BakeStatus::Refused(m) => Outcome::Failed(m.clone()),
            BakeStatus::Cancelled(_) => Outcome::Cancelled,
            BakeStatus::Idle => Outcome::Failed("the bake never started".into()),
        };
        keep(editor, scene, console, &file);
        let queue = running(editor)?;
        queue.current = None;
        queue.runner.finish(index, outcome)?;
    }
    if editor.unsaved_against_file(scene) {
        return Err(UNSAVED_STOP.to_string());
    }
    let queue = running(editor)?;
    let Some((index, path)) = queue.runner.start_next()? else {
        return Ok(false);
    };
    let total = queue.runner.queue().scenes.len();
    let file = at(queue.runner.root(), &path);
    console.info(format!(
        "Bake queue: scene {} of {total}: {path}",
        index + 1
    ));
    match open(editor, (scene, console, nav), &file) {
        Ok(()) => {
            scene.lighting_bake_requested = true;
            scene.lighting_bake_status = BakeStatus::Requested;
            running(editor)?.current = Some((index, file));
        }
        Err(why) => running(editor)?
            .runner
            .finish(index, Outcome::Failed(why))?,
    }
    Ok(true)
}

fn in_hand(editor: &EditorUi) -> Option<(usize, String)> {
    editor.dev.bake_queue.running.as_ref()?.current.clone()
}

fn running(editor: &mut EditorUi) -> Result<&mut EditorQueue, String> {
    let running = editor.dev.bake_queue.running.as_mut();
    running.ok_or_else(|| "the bake queue was dropped".to_string())
}

const UNSAVED_STOP: &str = "the open scene has unsaved edits the queue did not make; \
save it and Resume";

/// Any outcome: what only a full save keeps, or an edit made during the bake, into its
/// own file.
fn keep(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs, file: &str) {
    if editor.unsaved_against_file(scene) && editor.current_scene_path().as_deref() == Some(file) {
        editor.save_scene_to(scene, console, file);
    }
}

/// The navmesh first: probes are placed on its floor.
fn open(editor: &mut EditorUi, (scene, console, nav): Ctx, file: &str) -> Result<(), String> {
    if !editor.open_scene(scene, console, file, false) {
        return Err(format!("could not open '{file}' (the console says why)"));
    }
    editor.follow_edits(scene);
    editor.is_dirty = false;
    *nav = NavigationGraph::from_scene(scene);
    if has_navigation(scene) {
        bake_navmesh(scene, nav, Some(file))?;
    }
    Ok(())
}

fn put_spot(editor: &EditorUi, scene: &mut Scene) {
    let Some(queue) = &editor.dev.bake_queue.running else {
        return;
    };
    let total = queue.runner.queue().scenes.len();
    let (index, file) = match &queue.current {
        Some((index, file)) => (index + 1, file.clone()),
        None if scene.lighting_queue.is_some() => return,
        None => (0, String::new()),
    };
    scene.lighting_queue = Some(QueueSpot {
        index,
        total,
        scene: file,
    });
}

fn end(editor: &mut EditorUi, scene: &mut Scene, console: &mut ConsoleLogs, error: Option<String>) {
    let Some(queue) = editor.dev.bake_queue.running.take() else {
        return;
    };
    scene.lighting_queue = None;
    let elsewhere = editor.current_scene_path().as_deref() != Some(&queue.origin);
    if !editor.unsaved_against_file(scene) && (elsewhere || baked(&queue)) {
        editor.open_scene(scene, console, &queue.origin, false);
    }
    let summary = queue.runner.summary();
    let error_stop = error.is_some();
    let line = match error {
        Some(e) => format!("Bake queue stopped: {e}\n{summary}"),
        None => summary,
    };
    console.info(line.clone());
    scene.lighting_bake_status = match () {
        _ if queue.runner.was_cancelled() => BakeStatus::Cancelled(line),
        _ if error_stop || queue.runner.queue().failed() > 0 => BakeStatus::Failed(line),
        _ => BakeStatus::Done(line),
    };
}

fn baked(queue: &EditorQueue) -> bool {
    let root = queue.runner.root();
    let entries = queue.runner.queue().scenes.iter();
    let ran =
        |e: &&rusty_dev::bake::queue::Entry| e.state != rusty_dev::bake::queue::State::Pending;
    entries
        .filter(ran)
        .any(|e| rusty_dev::bake::queue::live::same_file(root, &e.path, &queue.origin))
}

#[cfg(test)]
mod tests;
