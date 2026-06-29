//! src/bin/session_mcp.rs — MCP stdio bridge for the EDIT-mode session (dev-only).
//!
//! "Blender-MCP, but for game development" (#288). Holds a live `GameWorld` in
//! **edit mode** and speaks the Model Context Protocol over stdio (newline-delimited
//! JSON-RPC 2.0), so Claude Code attaches to the engine natively and drives it
//! through the single existing evaluator — the very same control surface the
//! `session` bin and the in-editor console use. No second engine API, no subprocess,
//! no async, no extra dependency.
//!
//! Usage:
//!   cargo run --bin session-mcp --features dev               # boot the default scene
//!   cargo run --bin session-mcp --features dev -- <scene>    # boot a specific scene
//!   cargo run --bin session-mcp --features dev -- --empty    # start from empty scene
//!   cargo run --bin session-mcp --features dev -- --project <dir> [<scene>|--empty]
//!                                                            # chdir into a project first
//!
//! stdout carries ONLY JSON-RPC messages (one per line); engine logs go to stderr or
//! the in-memory console, never stdout. See `docs/mcp.md` for the tools/resources and
//! how to register it with Claude Code.

use std::io::{self, BufReader};
use std::process::exit;

use rusty::dev::mcp;
use rusty::dev::session::{self, Session};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // `--project <dir>` chdirs first; then `--empty` starts blank, an explicit path
    // boots that scene, and no argument boots (and seeds) the bundled default scene.
    let boot_scene = session::boot_scene_from_args(&args);

    let sess = match Session::new(&boot_scene) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("session-mcp: failed to start: {}", e);
            exit(2);
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    if let Err(e) = mcp::run(&sess, BufReader::new(stdin.lock()), stdout.lock()) {
        eprintln!("session-mcp: stdio error: {}", e);
        exit(2);
    }
}
