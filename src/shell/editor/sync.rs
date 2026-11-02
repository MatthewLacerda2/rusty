//! src/shell/editor/sync.rs — the settings the editor's menus share with the game:
//! the quality preset and the speaker mode, each read and written both ways.

use super::EditorFrontend;
use crate::app::GameWorld;
use crate::shell::Shell;

impl EditorFrontend {
    /// The Config menu and `Graphics.SetQuality` (script) select through one
    /// `Application`: select a preset picked in the menu, mirror the presets and the
    /// running one back into it, then hand the shared cell to the renderer
    /// (`settings::apply_graphics`), the player's graphics options beside the
    /// preset's values (#1438, #1522).
    pub(super) fn sync_quality(&mut self, shell: &mut Shell, game: &GameWorld) {
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
    pub(super) fn sync_speaker_mode(&mut self, game: &GameWorld) {
        let mut audio = game.resources.audio.borrow_mut();
        if let Some(mode) = self.editor_ui.speaker_mode_request.take() {
            audio.set_speaker_mode(mode);
        }
        self.editor_ui.speaker_mode = audio.speaker_mode();
    }
}
