//! src/main.rs — the `rusty` editor binary.
//!
//! A thin entry point: the runtime shell (`rusty::shell`) owns the window and frame
//! loop, and its editor frontend adds the egui dashboard. The standalone player is
//! `src/bin/player.rs`; both share the same shell (#431).
//!
//! ```text
//! cargo run -- --project <dir>
//! ```
//!
//! opens the game project at `<dir>` (default `./project`, created if missing; #829).

fn main() {
    env_logger::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    rusty::core::project::open_from_args(&mut args);
    rusty::shell::editor::launch();
}
