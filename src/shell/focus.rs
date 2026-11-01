//! src/shell/focus.rs — what the window gaining or losing focus means.
//!
//! Gaining it: the clipboard may hold something copied in another app (#612), and in
//! the editor, files may have been dropped into the project from outside, so the
//! assets are refreshed (Unity's auto-refresh; today MP3 → WAV, #385). Losing it:
//! key-ups never arrive for keys released while unfocused, so every key is released.

use super::frame::Host;
use super::{boot, capture_clipboard, Shell};
use crate::app::GameWorld;

/// Handle a `WindowEvent::Focused(focused)` in the frontend `host`.
pub(super) fn changed(shell: &mut Shell, game: &mut GameWorld, focused: bool, host: Host) {
    shell.window_focused = focused;
    if focused {
        capture_clipboard(shell, game);
        refresh_assets(game, host);
    } else {
        game.input().borrow_mut().release_all();
    }
}

/// The editor's refresh, logged to the console so the swap of an `.mp3` for its
/// `.wav` is never silent. A no-op in the player, whatever features the binary
/// was built with (`player` builds with the editor on by default): it boots
/// read-only (#1653), and a shipped game has nothing arriving.
pub(super) fn refresh_assets(game: &GameWorld, host: Host) {
    if host != Host::Editor {
        return;
    }
    let mut console = game.console().borrow_mut();
    for (warning, line) in boot::refresh_assets().lines() {
        match warning {
            true => console.warn(line),
            false => console.info(line),
        }
    }
}
