//! crates/shell/src/editor/sync.rs — the settings the editor's menus share with the game:
//! the quality preset, the speaker mode, the Scene view's gizmos and the Game view's
//! size, each read and written both ways, and the frame stats the Game view's Stats
//! overlay shows.

use super::EditorFrontend;
use crate::Shell;
use rusty_app::GameWorld;
use rusty_core::application::Application;
use rusty_editor::viewport::stats::{Clock, Readout};
use rusty_editor::EditorUi;

/// What the editor window can do for a script, marked once when it is built, before
/// its first frame drains a command (drawn or not, a locked screen's included): it
/// has a Scene view, so `Editor.SetGizmo` is welcome (#1909), a Game view whose size
/// `Editor.SetGameViewSize` sets (#2207), and its frames step Generate Lighting's
/// job, drawn or not (`EditorUi::frame_work`), so `Lighting.GenerateAsync()` is
/// (#1999). Each only ever turns on.
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

    /// The Config menu and `Graphics.SetQuality` (script) select through one
    /// `Application`: select a preset picked in the menu, mirror the presets and the
    /// running one back into it, then hand the shared cell to the renderer
    /// (`settings::apply_graphics`), the player's graphics options beside the
    /// preset's values (#1438, #1522).
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

    /// The Scene view's gizmos (#1909): take what a script's `Editor.SetGizmo`
    /// asked (welcome here, `mark_host`), then mirror the toggles back for
    /// `Editor.GetGizmo`.
    fn sync_gizmos(&mut self, game: &GameWorld) {
        let mut app = game.resources.application.borrow_mut();
        app.sync_gizmos(&mut self.editor_ui.gizmos);
        // The Game view's size (#2207), the same way: a script's set, then mirrored.
        app.sync_game_view(&mut self.editor_ui.game_view);
    }
}

/// The Game view's **Stats** overlay (#2205) reads the frame stats over the last
/// second, the table `Debug.Stats{ window = 60 }` returns, and only on a frame it
/// shows (the toggle on, the Game tab open): otherwise it folds nothing. Without
/// the `dev` layer nothing records them, and it shows the window's `clock` alone.
/// `timestamps`: whether the renderer times the GPU (`Renderer::times_gpu`).
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
