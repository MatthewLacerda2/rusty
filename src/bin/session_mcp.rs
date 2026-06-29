//! src/bin/session_mcp.rs — MCP stdio bridge for the EDIT-mode session (dev-only).
//!
//! "Blender-MCP, but for game development" (#288). Speaks the Model Context Protocol
//! over stdio (newline-delimited JSON-RPC 2.0) so Claude Code attaches to the engine
//! natively and drives it through the single existing evaluator — the very same
//! control surface the `session` bin and the in-editor console use. No second engine
//! API, no subprocess, no async, no extra dependency.
//!
//! ## Two modes
//! - **embed** (default): boot an in-process headless `Session` and evaluate against
//!   it. Great for an agent working solo; there is no window.
//! - **attach** (`--attach [<addr>]`, #307): forward to a *running window's* command
//!   socket (#282) instead, so the agent drives the live, rendered world you are
//!   watching on screen. Holds no connection when idle and reconnects per call, so a
//!   window that closes and reopens is picked up transparently.
//!
//! Usage:
//!   cargo run --bin session-mcp --features dev               # embed: default scene
//!   cargo run --bin session-mcp --features dev -- <scene>    # embed: a specific scene
//!   cargo run --bin session-mcp --features dev -- --empty    # embed: an empty scene
//!   cargo run --bin session-mcp --features dev -- --project <dir> [<scene>|--empty]
//!                                                            # embed: chdir into a project
//!   cargo run --bin session-mcp --features dev -- --attach   # attach: default socket
//!   cargo run --bin session-mcp --features dev -- --attach <addr>
//!                                                            # attach: explicit socket
//!
//! stdout carries ONLY JSON-RPC messages (one per line); engine logs go to stderr or
//! the in-memory console, never stdout. See `docs/mcp.md` for the tools/resources and
//! how to register it with Claude Code.

use std::io::{self, BufReader};
use std::process::exit;

use rusty::dev::mcp::{self, attach::SocketClient, EvalBackend};
use rusty::dev::session::{self, Session};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Pick the backend: `--attach [<addr>]` forwards to a running window's socket;
    // otherwise boot an in-process headless world (honouring `--project` / scene args).
    let backend: Box<dyn EvalBackend> = match attach_address(&args) {
        Some(addr) => Box::new(SocketClient::new(addr)),
        None => Box::new(boot_embedded(&args)),
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    if let Err(e) = mcp::run(
        backend.as_ref(),
        BufReader::new(stdin.lock()),
        stdout.lock(),
    ) {
        eprintln!("session-mcp: stdio error: {}", e);
        exit(2);
    }
}

/// Parse the `--attach [<addr>]` flag. `None` means embed mode; `Some(None)` is
/// `--attach` with no address (use the default socket); `Some(Some(addr))` is an
/// explicit override. A token starting with `--` after `--attach` is the next flag,
/// not the address.
fn attach_address(args: &[String]) -> Option<Option<String>> {
    let pos = args.iter().position(|a| a == "--attach")?;
    let addr = args.get(pos + 1).filter(|a| !a.starts_with("--")).cloned();
    Some(addr)
}

/// Boot the in-process embed-mode session, exiting the process on failure.
fn boot_embedded(args: &[String]) -> Session {
    // `--project <dir>` chdirs first; then `--empty` starts blank, an explicit path
    // boots that scene, and no argument boots (and seeds) the bundled default scene.
    let boot_scene = session::boot_scene_from_args(args);
    match Session::new(&boot_scene) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("session-mcp: failed to start: {}", e);
            exit(2);
        }
    }
}
