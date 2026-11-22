//! stored-format: the engine's entries in a project's `.mcp.json`
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

pub const CLAUDE_FILE: &str = "CLAUDE.md";
pub const MCP_FILE: &str = ".mcp.json";
pub(super) const SERVERS: [&str; 2] = ["rusty", "rusty-headless"];
const CLAUDE_TEMPLATE: &str = include_str!("../game_claude.md");
const SESSION_MCP: &str = "session-mcp";

fn checkout() -> PathBuf {
    let dir = super::checkout();
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

pub(super) fn write(root: &Path, mcp: bool) -> Vec<String> {
    write_for(root, mcp, &checkout())
}

/// Read from where `exe` sits, never `cfg!(debug_assertions)`, which a custom profile
/// or a release build with assertions gets wrong.
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

fn running_session_binary(engine: &Path) -> PathBuf {
    let exe = std::env::current_exe().ok();
    let exe = exe.map(|e| std::fs::canonicalize(&e).unwrap_or(e));
    session_binary(engine, exe.as_deref(), Path::is_file)
}

pub(super) fn is_checkout_binary(command: &str, engine: &Path) -> bool {
    let path = Path::new(command);
    path.starts_with(engine.join("target"))
        && path.file_stem().and_then(|s| s.to_str()) == Some(SESSION_MCP)
}

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

/// A built `session-mcp`, run directly: `cargo run` from a game folder would skip the checkout's
/// toolchain and config and cold-build past Claude Code's startup timeout.
fn server(command: &str, args: &[&str]) -> Value {
    json!({ "command": command, "args": args })
}

fn put(path: &Path, text: &str) -> Result<(), String> {
    // lint: allow(asset-write) project settings, not read by a path-keyed cache
    crate::fs::write_atomic(path, text.as_bytes()).map_err(|e| e.to_string())
}

fn warning(file: &str, why: &str) -> String {
    format!("warning: {file} not written: {why}")
}

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
