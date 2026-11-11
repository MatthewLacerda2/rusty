//! src/core/project/agent/mod.rs — the files that make a project ready for an agent (#1313).
//!
//! A game is built by a Claude session started in its project folder, so an
//! edit-open writes two files at the root:
//!
//! * **`CLAUDE.md`** — the game's own instructions, from a short template
//!   (`game_claude.md`): what the folder is, where the engine's guide is, and that
//!   engine findings are filed, not fixed. It is the game's file: written once when
//!   absent, never rewritten.
//! * **`.mcp.json`** — registers the engine's `session-mcp` with Claude Code, in attach
//!   mode (`rusty`, the editor window) and embed mode (`rusty-headless`). The file is
//!   shared with any other server the game registers: the engine owns only those two
//!   entries, and a `dev` build rewrites them on every edit-open, so they name the
//!   checkout that last opened the project. An entry whose `command` already names a
//!   `session-mcp` under this checkout's `target/` keeps it (a release build the
//!   operator chose, #2024) and gets only its `args` refreshed; any other gets the
//!   running build's profile ([`session_binary`]). A build without `dev` has no
//!   `session-mcp` and never writes it.
//!
//! Both are skipped for a project inside the engine checkout (its own `./project`, a
//! worktree's), which is no game. A failed write is a warning, never a failed open.
//!
//! The engine's half of the instructions is not copied here: `session-mcp` serves it
//! from the checkout as `rusty://game-guide.md`, so it never goes stale in a game.
//!
//! stored-format: the engine's entries in a project's `.mcp.json`

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// The game's instructions for an agent, at the project root.
pub const CLAUDE_FILE: &str = "CLAUDE.md";
/// Claude Code's project MCP file, at the project root.
pub const MCP_FILE: &str = ".mcp.json";
/// The server entries under `mcpServers` the engine owns: attach and embed mode.
pub(super) const SERVERS: [&str; 2] = ["rusty", "rusty-headless"];
const CLAUDE_TEMPLATE: &str = include_str!("../game_claude.md");
/// The bridge binary's file name, as cargo builds it into `target/<profile>/`.
const SESSION_MCP: &str = "session-mcp";

/// The engine checkout this build was made from.
fn checkout() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    crate::core::paths::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

/// Write the agent files at `root` for an edit-open: `CLAUDE.md` when absent, and the
/// engine's `.mcp.json` entries when `mcp` (a `dev` build). Returns one line per file
/// created and one warning per file that could not be written.
pub(super) fn write(root: &Path, mcp: bool) -> Vec<String> {
    write_for(root, mcp, &checkout())
}

/// The `session-mcp` a fresh engine entry runs: the one beside the running binary when
/// that binary lies in `<engine>/target/<profile>/` and the file exists, else the first
/// of `target/debug` and `target/release` that has one, else `target/debug`'s (#2024).
/// Read from where the binary sits, never `cfg!(debug_assertions)`, which a custom
/// profile or a release build with assertions would get wrong. `exe` is the running
/// binary and `exists` the file check, both passed in so tests drive it with fakes.
pub(super) fn session_binary(
    engine: &Path,
    exe: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    let target = engine.join("target");
    let running = exe
        .and_then(Path::parent)
        .filter(|dir| dir.parent() == Some(target.as_path()));
    let (debug, release) = (target.join("debug"), target.join("release"));
    for dir in running
        .into_iter()
        .chain([debug.as_path(), release.as_path()])
    {
        let binary = dir.join(SESSION_MCP);
        if exists(&binary) {
            return binary;
        }
    }
    debug.join(SESSION_MCP)
}

/// [`session_binary`] for this process.
fn running_session_binary(engine: &Path) -> PathBuf {
    let exe = std::env::current_exe().ok();
    let exe = exe.map(|e| crate::core::paths::canonicalize(&e).unwrap_or(e));
    session_binary(engine, exe.as_deref(), Path::is_file)
}

/// Whether `command` names a `session-mcp` under `engine`'s `target/`, in any profile
/// directory: an entry the operator pointed at a build of this checkout, kept as set.
pub(super) fn is_checkout_binary(command: &str, engine: &Path) -> bool {
    let path = Path::new(command);
    path.starts_with(engine.join("target"))
        && path.file_stem().and_then(|s| s.to_str()) == Some(SESSION_MCP)
}

/// [`write`] for the engine checkout `engine`.
pub(super) fn write_for(root: &Path, mcp: bool, engine: &Path) -> Vec<String> {
    if root.starts_with(engine) {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let claude = root.join(CLAUDE_FILE);
    if !claude.exists() {
        lines.push(match put(&claude, &claude_md(root, engine)) {
            Ok(()) => format!("created {CLAUDE_FILE}"),
            Err(e) => warning(CLAUDE_FILE, &e),
        });
    }
    if mcp {
        lines.extend(write_mcp(root, engine));
    }
    lines
}

/// Bring the engine's entries in `root`'s `.mcp.json` up to date, keeping the rest of
/// the file. `Some` line when it was created or could not be written.
fn write_mcp(root: &Path, engine: &Path) -> Option<String> {
    let Some(root_text) = root.to_str() else {
        return Some(warning(MCP_FILE, "the project path is not UTF-8"));
    };
    let path = root.join(MCP_FILE);
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Some(warning(MCP_FILE, &e.to_string())),
    };
    let mut config = match existing.as_deref().map(serde_json::from_str::<Value>) {
        None => json!({}),
        Some(Ok(value)) => value,
        Some(Err(e)) => return Some(warning(MCP_FILE, &format!("not valid JSON ({e})"))),
    };
    if let Err(e) = merge(
        &mut config,
        root_text,
        engine,
        &running_session_binary(engine),
    ) {
        return Some(warning(MCP_FILE, e));
    }
    let mut text = serde_json::to_string_pretty(&config).unwrap_or_default();
    text.push('\n');
    if existing.as_deref() == Some(text.as_str()) {
        return None;
    }
    match put(&path, &text) {
        Err(e) => Some(warning(MCP_FILE, &e)),
        Ok(()) => existing.is_none().then(|| format!("created {MCP_FILE}")),
    }
}

/// Set the engine's two server entries in `config`, leaving every other key alone. An
/// entry already running a `session-mcp` of `engine` keeps its `command`; any other
/// runs `binary`.
pub(super) fn merge(
    config: &mut Value,
    root: &str,
    engine: &Path,
    binary: &Path,
) -> Result<(), &'static str> {
    let servers = config
        .as_object_mut()
        .ok_or("not a JSON object")?
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("`mcpServers` is not an object")?;
    let fresh = binary.to_string_lossy();
    let [attach, headless] = SERVERS;
    for (name, args) in [
        (attach, vec!["--attach"]),
        (headless, vec!["--project", root]),
    ] {
        let kept = servers
            .get(name)
            .and_then(|entry| entry["command"].as_str())
            .filter(|command| is_checkout_binary(command, engine))
            .map(str::to_owned);
        let command = kept.unwrap_or_else(|| fresh.to_string());
        servers.insert(name.into(), server(&command, &args));
    }
    Ok(())
}

/// One server entry: a built `session-mcp` of the checkout, run directly. `cargo run`
/// from a game folder would ignore the checkout's toolchain and config and cold-build
/// the engine past Claude Code's startup timeout.
fn server(command: &str, args: &[&str]) -> Value {
    json!({ "command": command, "args": args })
}

fn put(path: &Path, text: &str) -> Result<(), String> {
    // lint: allow(asset-write) project settings, not read by a path-keyed cache
    crate::core::fs::write_atomic(path, text.as_bytes()).map_err(|e| e.to_string())
}

fn warning(file: &str, why: &str) -> String {
    format!("warning: {file} not written: {why}")
}

/// The game's `CLAUDE.md`: the template, with the project's name and the checkout.
pub(super) fn claude_md(root: &Path, engine: &Path) -> String {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    CLAUDE_TEMPLATE
        .replace("{name}", &name)
        .replace("{engine}", &engine.to_string_lossy())
}

#[cfg(test)]
mod binary_tests;
#[cfg(test)]
mod mcp_tests;
#[cfg(test)]
mod tests;
