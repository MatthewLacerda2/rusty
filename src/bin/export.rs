//! src/bin/export.rs — ship a game project (dev-only, `--features dev`, #1267).
//!
//! ```text
//! Usage:
//!   cargo run --bin export --features dev -- --project <dir> --out <dir>
//!   make export PROJECT=<dir> OUT=<dir>
//! ```
//!
//! Brings every converted source of the project (`.fbx`, `.blend`) up to date,
//! then copies its runtime files and import cache into `<out>`, leaving those
//! sources behind (`rusty::dev::export`). The player runs the result
//! (`player --project <out>`) without converting anything or needing Blender.
//! A dev binary, not a player flag: exporting may run Blender, and a shipped
//! player carries neither the converters' authoring path nor `dev`.

use std::path::{absolute, PathBuf};
use std::process::exit;

use rusty::core::project::Access;

const USAGE: &str = "usage: export [--project <dir>] --out <dir>";

fn main() {
    rusty::logging::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let project = rusty::core::project::take_flag(&mut args);
    let (Ok(project), [flag, out]) = (project, args.as_slice()) else {
        eprintln!("{USAGE}");
        exit(2);
    };
    if flag != "--out" {
        eprintln!("{USAGE}");
        exit(2);
    }
    // Named from the launch directory; opening the project moves it.
    let out = absolute(out).unwrap_or_else(|_| PathBuf::from(out));
    // `Edit`, as the editor opens it: the export needs `project.rusty`, which makes
    // the import cache project-relative and carries the build settings.
    let opened = rusty::core::project::open(&rusty::core::project::locate(project), Access::Edit);
    if let Err(e) = opened {
        eprintln!("export: cannot open the project: {e}");
        exit(2);
    }
    match rusty::dev::export::export(&out) {
        Ok(done) => println!(
            "export: wrote {} files to {} ({} sources converted and left behind)",
            done.files.len(),
            out.display(),
            done.sources.len()
        ),
        Err(e) => {
            eprintln!("export: {e}");
            exit(1);
        }
    }
}
