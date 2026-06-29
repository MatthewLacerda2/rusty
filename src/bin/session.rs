//! src/bin/session.rs — headless EDIT-mode session (dev-only, `--features dev`).
//!
//! The keystone (#177): a long-lived engine process an external agent can talk to.
//!
//! Usage:
//!   cargo run --bin session --features dev               # boot the default scene
//!   cargo run --bin session --features dev -- <scene>    # boot a specific scene
//!   cargo run --bin session --features dev -- --empty    # start from empty scene
//!   cargo run --bin session --features dev -- --project <dir> [<scene>|--empty]
//!                                                        # chdir into a project first
//!
//! Holds a live `GameWorld` in **edit mode** (not forced play) and reads commands
//! from stdin, one Lua line per command, evaluating each against the live world
//! through the single existing evaluator and writing one JSON response line per
//! command to stdout (`{"ok":true,"result":"…"}` / `{"ok":false,"error":"…"}`).
//! State persists across commands for the life of the process; a bad command is
//! reported and the session stays up. The loop ends at EOF.

use std::io::{self, BufReader};
use std::process::exit;

use rusty::dev::session::{self, Session};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // `--project <dir>` chdirs first; then `--empty` starts blank, an explicit path
    // boots that scene, and no argument boots (and seeds) the bundled default scene.
    let boot_scene = session::boot_scene_from_args(&args);

    let sess = match Session::new(&boot_scene) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("session: failed to start: {}", e);
            exit(2);
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    if let Err(e) = session::run(&sess, BufReader::new(stdin.lock()), stdout.lock()) {
        eprintln!("session: channel error: {}", e);
        exit(2);
    }
}
