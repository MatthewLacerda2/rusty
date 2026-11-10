//! src/shell/editor/sync.rs — the settings the editor's menus share with the game:
//! the quality preset, the speaker mode and the Scene view's gizmos, each read and
//! written both ways.

use super::EditorFrontend;
use crate::app::GameWorld;
use crate::shell::Shell;

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
        crate::shell::settings::apply_graphics(shell, game);
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

    /// The Scene view's gizmos (#1909): this window has a Scene view, so a script's
    /// `Editor.SetGizmo` is welcome; take what it asked, then mirror the toggles back
    /// for `Editor.GetGizmo`.
    fn sync_gizmos(&mut self, game: &GameWorld) {
        let mut app = game.resources.application.borrow_mut();
        app.mark_gizmo_viewer();
        // Its inspector steps Generate Lighting's job, so `Lighting.GenerateAsync()`
        // is welcome here (#1999).
        app.mark_bake_host();
        app.sync_gizmos(&mut self.editor_ui.gizmos);
    }
}

/// The frame that would start a script's `Lighting.GenerateAsync()` refuses it
/// while Play is asked for but not yet taken (#1999, Unity's
/// `isPlayingOrWillChangePlaymode`): the request was made of edit mode. Runs before
/// the editor draws, where the inspector takes requests.
pub(super) fn refuse_bake_request_on_play(game: &GameWorld) {
    #[cfg(feature = "dev")]
    if game.resources.application.borrow().play_heading() {
        game.world
            .scene
            .borrow_mut()
            .drop_bake_request("Play was requested");
    }
    #[cfg(not(feature = "dev"))]
    let _ = game;
}

#[cfg(all(test, feature = "dev"))]
mod tests {
    /// The host's own pending Play (the transport) refuses a request at the draw.
    #[test]
    fn a_pending_host_play_refuses_a_request_before_the_draw() {
        let s = crate::dev::session::Session::new("").expect("an empty session boots");
        let game = s.world();
        game.world.scene.borrow_mut().lighting_bake_requested = true;
        super::refuse_bake_request_on_play(game);
        assert!(
            game.world.scene.borrow().lighting_bake_requested,
            "no Play asked"
        );
        game.resources.application.borrow_mut().set_host_play(true);
        super::refuse_bake_request_on_play(game);
        let scene = game.world.scene.borrow();
        assert!(!scene.lighting_bake_requested);
        assert_eq!(scene.lighting_bake_status.state(), "refused");
    }
}
