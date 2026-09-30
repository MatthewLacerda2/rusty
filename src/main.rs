//! src/main.rs — the `rusty` editor binary.
//!
//! A thin entry point: the runtime shell (`rusty::shell`) owns the window and frame
//! loop, and its editor frontend adds the egui dashboard. The standalone player is
//! `src/bin/player.rs`; both share the same shell (#431).

fn main() {
    env_logger::init();
    rusty::shell::editor::launch();
}
