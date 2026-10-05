//! src/bin/player.rs — the standalone player: run a rusty game without the editor.
//!
//! Boots the startup scene named in `build_settings.json` straight into Play,
//! renders it full-window, and exits when the game calls `Application.Quit()` or the
//! window closes. Ship it built without the editor:
//!
//!   cargo build --release --bin player --no-default-features
//!
//! The project it runs is `--project <dir>`, else a `project/` folder shipped beside
//! the executable (macOS: in the bundle's `Contents/Resources/`), else `./project`
//! (#829). The engine's own content travels as `engine/` in the same place.
//!
//! It builds with the editor feature on too (it just doesn't use it), so a plain
//! `cargo run --bin player` works while developing.

fn main() {
    env_logger::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    rusty::core::project::open_from_args(&mut args);
    rusty::shell::player::launch();
}
