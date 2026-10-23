//! src/core/project/agent.rs — the files that make a project ready for an agent (#1313).
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
//!   checkout that last opened the project. A build without `dev` has no
//!   `session-mcp` and never writes it.
//!
//! Both are skipped for a project inside the engine checkout (its own `./project`, a
//! worktree's), which is no game. A failed write is a warning, never a failed open.
//!
//! The engine's half of the instructions is not copied here: `session-mcp` serves it
//! from the checkout as `rusty://game-guide.md`, so it never goes stale in a game.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// The game's instructions for an agent, at the project root.
pub const CLAUDE_FILE: &str = "CLAUDE.md";
/// Claude Code's project MCP file, at the project root.
pub const MCP_FILE: &str = ".mcp.json";
/// The server entries under `mcpServers` the engine owns: attach and embed mode.
pub(super) const SERVERS: [&str; 2] = ["rusty", "rusty-headless"];
const CLAUDE_TEMPLATE: &str = include_str!("game_claude.md");

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
    if let Err(e) = merge(&mut config, root_text, engine) {
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

/// Set the engine's two server entries in `config`, leaving every other key alone.
pub(super) fn merge(config: &mut Value, root: &str, engine: &Path) -> Result<(), &'static str> {
    let servers = config
        .as_object_mut()
        .ok_or("not a JSON object")?
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("`mcpServers` is not an object")?;
    let [attach, headless] = SERVERS;
    servers.insert(attach.into(), server(engine, &["--attach"]));
    servers.insert(headless.into(), server(engine, &["--project", root]));
    Ok(())
}

/// One server entry: the checkout's built `session-mcp`, run directly. `cargo run`
/// from a game folder would ignore the checkout's toolchain and config and cold-build
/// the engine past Claude Code's startup timeout.
fn server(engine: &Path, args: &[&str]) -> Value {
    let binary = engine.join("target").join("debug").join("session-mcp");
    json!({ "command": binary.to_string_lossy(), "args": args })
}

fn put(path: &Path, text: &str) -> Result<(), String> {
    // lint: allow(asset-write) project settings, not read by a path-keyed cache
    std::fs::write(path, text).map_err(|e| e.to_string())
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
