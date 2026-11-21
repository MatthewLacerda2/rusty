//! src/main.rs — the `rusty` editor binary.
//!
//! A thin entry point: the runtime shell (`rusty::shell`) owns the window and frame
//! loop, and its editor frontend adds the egui dashboard. The standalone player is
//! `src/bin/player.rs`; both share the same shell (#431).
//!
//! ```text
//! cargo run --bin rusty                     # the project picker: recent projects, New, Open
//! cargo run --bin rusty -- --project <dir>  # open the game project at <dir> (created if missing)
//! ```
//!
//! `-h` / `--help` prints the options and exits; an argument it does not know prints
//! them and exits 2. Neither opens a window.
//!
//! Without `--project` the editor starts on the project picker (#854), as Unity starts
//! on the Hub; with it the picker is skipped, as Unity skips it on `-projectPath`.

use rusty::core::project::{self, Access};

const USAGE: &str = "usage: rusty [--project <dir>]
  (no option)      start on the project picker: recent projects, New, Open
  --project <dir>  open the game project at <dir> (created if missing)
  -h, --help       print this and exit";

fn main() {
    rusty::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    project::answer_help(&args, USAGE);
    let named = project::take_flag(&mut args).unwrap_or_else(|e| usage(&e));
    if let Some(unknown) = args.first() {
        usage(&format!("unknown argument {unknown}"));
    }
    match named {
        Some(dir) => {
            let opened = project::open(&dir, Access::Edit).unwrap_or_else(|e| fail(&e));
            rusty::shell::editor::launch(&opened);
        }
        None => rusty::shell::editor::launch_picker(),
    }
}

/// A command line it cannot read: print `error` and the usage, exit 2, before any
/// window opens.
fn usage(error: &str) -> ! {
    eprintln!("rusty: {error}\n{USAGE}");
    std::process::exit(2);
}

fn fail(err: &str) -> ! {
    eprintln!("[Project] cannot open the project: {err}");
    std::process::exit(2);
}
