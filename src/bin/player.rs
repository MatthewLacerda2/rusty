//! src/bin/player.rs — the standalone player: run a rusty game without the editor.
//!
//! Boots the startup scene named in `project/build_settings.json` straight into Play,
//! renders it full-window, and exits when the game calls `Application.Quit()` or the
//! window closes. Ship it built without the editor:
//!
//!   cargo build --release --bin player --no-default-features
//!
//! It builds with the editor feature on too (it just doesn't use it), so a plain
//! `cargo run --bin player` works while developing.

fn main() {
    env_logger::init();
    rusty::shell::player::launch();
}
