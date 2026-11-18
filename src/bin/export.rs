//! src/bin/export.rs — ship a game project (dev-only, `--features dev`, #1267).
//!
//! ```text
//! Usage:
//!   cargo run --bin export --features dev -- --project <dir> --out <dir>
//!   make export PROJECT=<dir> OUT=<dir>
//!   cargo run --bin export --features dev -- --project <dir> --import-only
//!   make import PROJECT=<dir>
//! ```
//!
//! Brings every converted source of the project (`.fbx`, `.blend`) up to date,
//! then copies its runtime files and import cache into `<out>`, leaving those
//! sources behind (`rusty::dev::export`). The player runs the result
//! (`player --project <out>`) without converting anything or needing Blender.
//! A dev binary, not a player flag: exporting may run Blender, and a shipped
//! player carries neither the converters' authoring path nor `dev`.
//!
//! `--import-only` stops after the first step: it brings the project's import
//! cache up to date in place (every converted model and every image's compressed
//! copy, `rusty::dev::export::import`) and copies nothing. A headless run (`play`,
//! `bench-scenes`) never compresses a texture, so this is how it draws a project
//! the editor never opened as the shipped game does (#2319).

use std::path::{absolute, PathBuf};
use std::process::exit;

use rusty::core::project::Access;

const USAGE: &str = "usage: export [--project <dir>] (--out <dir> | --import-only)";

fn main() {
    rusty::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    rusty::core::project::answer_help(&args, USAGE);
    let project = rusty::core::project::take_flag(&mut args);
    let Ok(project) = project else {
        eprintln!("{USAGE}");
        exit(2);
    };
    if args == ["--import-only"] {
        import_only(project);
    }
    let [flag, out] = args.as_slice() else {
        eprintln!("{USAGE}");
        exit(2);
    };
    if flag != "--out" {
        eprintln!("{USAGE}");
        exit(2);
    }
    // Named from the launch directory; opening the project moves it.
    let out = absolute(out).unwrap_or_else(|_| PathBuf::from(out));
    // Checked before opening: `open` would fill a stray folder with a skeleton.
    let dir = rusty::core::project::locate(project);
    if let Err(e) = rusty::dev::export::check(&dir, &out) {
        eprintln!("export: {e}");
        exit(1);
    }
    // `Edit`, as the editor opens it: the export needs `project.rusty`, which makes
    // the import cache project-relative and carries the build settings.
    let opened = rusty::core::project::open(&dir, Access::Edit);
    if let Err(e) = opened {
        eprintln!("export: cannot open the project: {e}");
        exit(2);
    }
    match rusty::dev::export::export(&out) {
        Ok(done) => {
            let sources = match done.sources.len() {
                1 => "1 source".to_string(),
                n => format!("{n} sources"),
            };
            println!(
                "export: wrote {} files to {}; converted and left behind {sources}",
                done.files.len(),
                out.display(),
            );
        }
        Err(e) => {
            eprintln!("export: {e}");
            exit(1);
        }
    }
}

/// `--import-only`: the project's import cache brought up to date in place.
fn import_only(project: Option<PathBuf>) -> ! {
    let dir = rusty::core::project::locate(project);
    if !rusty::core::project::is_project(&dir) {
        eprintln!("export: {} is not a game project", dir.display());
        exit(2);
    }
    // `Run`: the import cache is the only thing written, no `project.rusty`,
    // `CLAUDE.md` or `.mcp.json`; it lands where a run of the project reads it.
    if let Err(e) = rusty::core::project::open(&dir, Access::Run) {
        eprintln!("export: cannot open the project: {e}");
        exit(2);
    }
    eprintln!("{}", rusty::asset::import::notice(&dir));
    match rusty::dev::export::import(std::path::Path::new(rusty::core::project::ASSETS_DIR)) {
        Ok(done) => {
            let t = &done.textures;
            println!(
                "import: {} model sources up to date; {} images compressed, {} failed \
                 (they draw uncompressed)",
                done.sources.len(),
                t.encoded,
                t.failed.len(),
            );
            exit(0);
        }
        Err(e) => {
            eprintln!("export: {e}");
            exit(1);
        }
    }
}
