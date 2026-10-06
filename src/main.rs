//! src/main.rs — the `rusty` editor binary.
//!
//! A thin entry point: the runtime shell (`rusty::shell`) owns the window and frame
//! loop, and its editor frontend adds the egui dashboard. The standalone player is
//! `src/bin/player.rs`; both share the same shell (#431).
//!
//! ```text
//! cargo run                     # the project picker: recent projects, New, Open
//! cargo run -- --project <dir>  # open the game project at <dir> (created if missing)
//! ```
//!
//! Without `--project` the editor starts on the project picker (#854), as Unity starts
//! on the Hub; with it the picker is skipped, as Unity skips it on `-projectPath`.

use rusty::core::project::{self, Access};

fn main() {
    env_logger::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    match project::take_flag(&mut args) {
        Ok(Some(dir)) => {
            let opened = project::open(&dir, Access::Edit).unwrap_or_else(|e| fail(&e));
            rusty::shell::editor::launch(&opened);
        }
        Ok(None) => rusty::shell::editor::launch_picker(),
        Err(e) => fail(&e),
    }
}

fn fail(err: &str) -> ! {
    eprintln!("[Project] cannot open the project: {err}");
    std::process::exit(2);
}
