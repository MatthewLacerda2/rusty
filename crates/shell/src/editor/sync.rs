use super::EditorFrontend;
use crate::Shell;
use rusty_app::GameWorld;
use rusty_core::application::Application;
use rusty_editor::viewport::stats::{Clock, Readout};
use rusty_editor::EditorUi;

/// What scripts may ask of this window, marked before its first frame drains a command,
/// drawn or not. Each mark only turns on.
pub(super) fn mark_host(app: &mut Application) {
    app.mark_gizmo_viewer();
    app.mark_game_view_host();
    app.mark_bake_host();
}

impl EditorFrontend {
    /// Each frame, after the editor drew: every setting it shares with the game.
    pub(super) fn sync_shared(&mut self, shell: &mut Shell, game: &GameWorld) {
        self.sync_quality(shell, game);
        self.sync_speaker_mode(game);
        self.sync_gizmos(game);
    }

    fn sync_quality(&mut self, shell: &mut Shell, game: &GameWorld) {
        let mut app = game.resources.application.borrow_mut();
        if let Some(name) = self.editor_ui.quality_request.take() {
            if let Err(err) = app.select_quality(&name) {
                log::warn!("[Quality] {err}");
            }
            game.script_manager().graphics_cell().borrow_mut().quality = app.quality_values();
        }
        let names = app.quality_presets().names();
        self.editor_ui.quality_presets = names.into_iter().map(str::to_string).collect();
        self.editor_ui.quality_preset = app.quality().map(str::to_string);
        drop(app);
        crate::settings::apply_graphics(shell, game);
    }

    /// Apply a speaker mode picked in Config, then mirror the maestro's mode (which
    /// `Audio.SetSpeakerMode` may also have changed) back into the menu.
    fn sync_speaker_mode(&mut self, game: &GameWorld) {
        let mut audio = game.resources.audio.borrow_mut();
        if let Some(mode) = self.editor_ui.speaker_mode_request.take() {
            audio.set_speaker_mode(mode);
        }
        self.editor_ui.speaker_mode = audio.speaker_mode();
    }

    /// The Scene view's gizmos: take what a script's `Editor.SetGizmo` asked, then mirror
    /// the toggles back for `Editor.GetGizmo`.
    fn sync_gizmos(&mut self, game: &GameWorld) {
        let mut app = game.resources.application.borrow_mut();
        app.sync_gizmos(&mut self.editor_ui.gizmos);
        // The Game view's size, the same way: a script's set, then mirrored.
        app.sync_game_view(&mut self.editor_ui.game_view);
    }
}

pub(super) fn sync_stats(ui: &mut EditorUi, game: &GameWorld, clock: Clock, timestamps: bool) {
    let shown = ui.viewport_tab == rusty_editor::ViewportTab::Game;
    let on = shown && ui.gizmos.on(rusty_core::application::Gizmo::Stats);
    ui.stats = on.then(|| Readout {
        table: windowed_stats(game),
        clock,
        timestamps,
    });
}

#[cfg(feature = "dev")]
fn windowed_stats(game: &GameWorld) -> Option<rusty_core::frame_stats::FrameStats> {
    let stats = game.script_manager().stats_cell();
    let window = stats
        .borrow()
        .window(rusty_editor::viewport::stats::STATS_WINDOW);
    Some(window)
}

// lint: allow(gpu-default-only) ship stub: frame stats are dev-only; nothing is drawn
#[cfg(not(feature = "dev"))]
fn windowed_stats(_game: &GameWorld) -> Option<rusty_core::frame_stats::FrameStats> {
    None
}

#[cfg(all(test, feature = "dev"))]
#[path = "sync_tests.rs"]
mod tests;
