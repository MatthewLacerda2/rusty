//! The editor playing a demo on its timeline (#1598): `Editor.OpenDemo(path)` takes
//! the editor's world, replays the demo on it, and the Game view's timeline plays,
//! pauses and scrubs it (`dev::demo::seek`). Closing, by the timeline's button, by
//! Stop or by quitting, puts back the edit scene as the same scene, with no input
//! held (`dev::demo::Origin`, shared with a seek back), the recorder, and what the
//! replay overwrote: the save data, settings, seed and clock (`dev::demo::lend`),
//! so nothing of the demo reaches `saved/`.
//!
//! Playback is read-only: the replay writes the recorded input over the live one
//! every tick, and the Game view hears none of the editor's (`game_has_input`). The
//! Game view keeps the size the demo recorded, drawn letterboxed in the panel.

use crate::app::{GameWorld, PlayTransition};
use crate::core::application::DemoRequest;
use crate::dev::demo::{self, Demo, Origin, Replayer};
use crate::editor::demo::DemoView;
use crate::editor::EditorUi;

/// One frame with the demo timeline in charge: the requests a script made, the
/// timeline's own, then the replay's ticks, spent against their recorded `dt`.
/// `None` when no demo is open.
pub fn advance(ui: &mut EditorUi, game: &mut GameWorld, dt: f32) -> Option<PlayTransition> {
    // This window has a timeline: a script's `Editor.OpenDemo` is welcome.
    game.resources.application.borrow_mut().mark_demo_viewer();
    let requests = game.resources.application.borrow_mut().take_demo_requests();
    for request in requests {
        on_request(ui, game, request);
    }
    let ended = !game.resources.application.borrow().play_heading();
    let view = ui.dev.demo.as_mut()?;
    if view.close || ended {
        return Some(close(ui, game));
    }
    let mut seek = view.seek.take();
    if seek.is_none() && !view.paused {
        let owed = view.pace.spend(view.current, dt);
        // The tick after the one it stands on; the end holds still.
        let last = view.ticks.saturating_sub(1);
        seek = (owed > 0).then(|| (view.current + owed).min(last));
    }
    let mut transition = PlayTransition::None;
    match seek {
        Some(tick) => {
            if let Err(err) = demo::seek(game, tick) {
                game.console()
                    .borrow_mut()
                    .error(format!("Editor demo: {err}"));
            }
            game.take_quit_request();
        }
        _ => transition = game.poll_transition(),
    }
    sync(ui, game);
    Some(transition)
}

/// Where the replay stands now, for the timeline and `Editor.DemoTick`.
fn sync(ui: &mut EditorUi, game: &mut GameWorld) {
    let done = game.tap_mut::<Replayer>().map(|r| r.verdict().ticks);
    let Some(view) = ui.dev.demo.as_mut() else {
        return;
    };
    view.current = done.unwrap_or(0).saturating_sub(1);
    let video = *game.script_manager().video_cell().borrow();
    let size = game.resources.screen.borrow().pixels(&video);
    view.size = (size.x as u32, size.y as u32);
    if view.at_end() {
        view.paused = true;
    }
    let place = (view.current, view.ticks);
    game.resources
        .application
        .borrow_mut()
        .set_demo_tick(Some(place));
}

fn on_request(ui: &mut EditorUi, game: &mut GameWorld, request: DemoRequest) {
    match request {
        DemoRequest::Open(path) => {
            if let Err(err) = open(ui, game, &path) {
                game.console()
                    .borrow_mut()
                    .error(format!("Editor.OpenDemo: {err}"));
            }
        }
        DemoRequest::Seek(tick) => match ui.dev.demo.as_mut() {
            Some(view) if tick < view.ticks => view.seek = Some(tick),
            Some(view) => game.console().borrow_mut().error(format!(
                "Editor.SeekDemo: tick {tick} is past the demo's end: it holds {} ticks",
                view.ticks
            )),
            None => game
                .console()
                .borrow_mut()
                .warn("Editor.SeekDemo: no demo is open".into()),
        },
        DemoRequest::Pause(paused) => match ui.dev.demo.as_mut() {
            Some(view) => view.set_paused(paused),
            None => game
                .console()
                .borrow_mut()
                .warn("Editor.PauseDemo: no demo is open".into()),
        },
    }
}

/// Take the world for the demo at `path`: its scene loaded, what the replay
/// overwrites lent (`demo::lend`), the replay prepared, Play entered and stopped
/// on tick 0.
fn open(ui: &mut EditorUi, game: &mut GameWorld, path: &str) -> Result<(), String> {
    if game.world.scene.borrow().is_playing() || game.play_requested() {
        return Err("stop Play first".into());
    }
    let demo = Demo::read(std::path::Path::new(path))?;
    if demo.ticks == 0 {
        return Err(format!("{path} holds no ticks"));
    }
    let scene_file = demo.header.scene.clone();
    let return_to = (Origin::of(game), ui.current_scene_path());
    let mut console = game.resources.console.borrow_mut();
    let loaded = ui.open_scene(
        &mut game.world.scene.borrow_mut(),
        &mut console,
        &scene_file,
        false,
    );
    drop(console);
    if !loaded {
        put_back(ui, game, return_to);
        return Err(format!("the demo's scene {scene_file} did not load"));
    }
    let view = DemoView::new(&demo, return_to, demo::lend(game));
    if let Err(err) = demo::prepare(game, demo) {
        view.lent.give_back(game);
        put_back(ui, game, view.return_to);
        return Err(err);
    }
    game.set_playing(true);
    ui.dev.demo = Some(view);
    ui.viewport_tab = crate::editor::ViewportTab::Game;
    if let Err(err) = demo::seek(game, 0) {
        close(ui, game);
        return Err(format!("the replay did not start: {err}"));
    }
    game.take_quit_request();
    sync(ui, game);
    Ok(())
}

/// Leave Play, drop the replay, and give the editor back its scene, its recorder
/// and everything the replay overwrote. The quit boundary calls it too, so a
/// demo open at quit flushes the editor's own store, not the demo's.
pub fn close(ui: &mut EditorUi, game: &mut GameWorld) -> PlayTransition {
    let Some(view) = ui.dev.demo.take() else {
        return PlayTransition::None;
    };
    game.set_playing(false);
    let transition = game.poll_transition();
    game.take_tap();
    demo::record_project(game);
    put_back(ui, game, view.return_to);
    view.lent.give_back(game);
    game.resources.application.borrow_mut().set_demo_tick(None);
    transition
}

/// The edit scene the demo found, back as it was and as the same scene.
fn put_back(ui: &mut EditorUi, game: &mut GameWorld, (origin, path): (Origin, Option<String>)) {
    origin.restore(game);
    ui.set_current_scene_path(path);
    ui.selected_entity_id = None;
    ui.selected_asset_path = None;
}
