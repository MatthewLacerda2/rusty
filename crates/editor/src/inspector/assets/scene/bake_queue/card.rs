//! The bake card's queue line: where a running queue is, or an unfinished one on disk
//! with its Resume button.

use std::time::{Duration, Instant};

use egui_phosphor::regular as icon;

use crate::EditorUi;
use rusty_dev::bake::queue::{read, Owner, Read, State};
use rusty_scene::Scene;

const LOOK_AGAIN: Duration = Duration::from_secs(1);

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, scene: &mut Scene) {
    if let Some(spot) = &scene.lighting_queue {
        ui.label(format!(
            "Bake queue: {}",
            spot.line(&scene.lighting_bake_status)
        ));
        return;
    }
    let Some(left) = unfinished(editor) else {
        return;
    };
    ui.horizontal(|ui| {
        ui.label(left);
        let live = !scene.bake_busy()
            && !scene.is_playing()
            && !editor.is_dirty
            && editor.current_scene_path().is_some();
        let resume = egui::Button::new(format!("{}  Resume", icon::PLAY));
        let clicked = ui
            .add_enabled(live, resume)
            .on_hover_text("Bake the queue's remaining scenes, one after another")
            .on_disabled_hover_text("Save the scene, stop Play and wait for the bake first")
            .clicked();
        if clicked {
            scene.lighting_queue_requested = true;
            editor.dev.bake_queue.seen = None;
        }
    });
}

/// `N of M scene(s) left` for a queue on disk no live process bakes, read once a second.
/// It reads the unsaved mark and never changes it: only a save does.
fn unfinished(editor: &mut EditorUi) -> Option<String> {
    let state = &mut editor.dev.bake_queue;
    let fresh = state
        .seen
        .as_ref()
        .filter(|(at, _)| at.elapsed() < LOOK_AGAIN);
    if let Some((_, line)) = fresh {
        return line.clone();
    }
    let line = match read(&state.root) {
        Read::Found(q) if q.unfinished() && !Owner::is_held(&state.root) => {
            let open = |s: &State| matches!(s, State::Pending | State::Running);
            let left = q.scenes.iter().filter(|e| open(&e.state)).count();
            Some(format!(
                "Unfinished bake queue: {left} of {} scene(s) left",
                q.scenes.len()
            ))
        }
        _ => None,
    };
    state.seen = Some((Instant::now(), line.clone()));
    line
}
