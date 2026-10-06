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

use rusty::core::project::Access;

fn main() {
    env_logger::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let opened = rusty::core::project::open_from_args(&mut args, Access::Edit);
    rusty::shell::editor::launch(&opened);
}
