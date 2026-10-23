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
//!   mode (`rusty`, the editor window) and embed mode (`rusty-headless`). It is
//!   engine-owned like `project.rusty`: rewritten on every edit-open by a `dev`
//!   build, so it names the checkout that last opened the project. A build without
//!   `dev` has no `session-mcp` and never writes it.
//!
//! The engine's half of the instructions is not copied here: `session-mcp` serves it
//! from the checkout as `rusty://game-guide.md`, so it never goes stale in a game.

use std::path::Path;

use serde_json::json;

/// The game's instructions for an agent, at the project root.
pub const CLAUDE_FILE: &str = "CLAUDE.md";
/// Claude Code's project MCP file, at the project root.
pub const MCP_FILE: &str = ".mcp.json";
const CLAUDE_TEMPLATE: &str = include_str!("game_claude.md");

/// The engine checkout this build was made from.
fn checkout() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

/// Write the agent files at `root` for an edit-open: `CLAUDE.md` when absent, and
/// `.mcp.json` when `mcp` (a `dev` build). Returns one line per file created.
pub(super) fn write(root: &Path, mcp: bool) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let claude = root.join(CLAUDE_FILE);
    if !claude.exists() {
        put(&claude, &claude_md(root))?;
        lines.push(format!("created {CLAUDE_FILE}"));
    }
    let config = root.join(MCP_FILE);
    if mcp {
        let created = !config.exists();
        let text = mcp_json(root);
        if std::fs::read_to_string(&config).ok().as_deref() != Some(text.as_str()) {
            put(&config, &text)?;
        }
        if created {
            lines.push(format!("created {MCP_FILE}"));
        }
    }
    Ok(lines)
}

fn put(path: &Path, text: &str) -> Result<(), String> {
    // lint: allow(asset-write) project settings, not read by a path-keyed cache
    std::fs::write(path, text).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// The game's `CLAUDE.md`: the template, with the project's name and the checkout.
pub(super) fn claude_md(root: &Path) -> String {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    CLAUDE_TEMPLATE
        .replace("{name}", &name)
        .replace("{engine}", checkout())
}

/// The `.mcp.json` text: both servers, run through `cargo run` against the checkout so
/// they work even when `session-mcp` was never built.
pub(super) fn mcp_json(root: &Path) -> String {
    let manifest = Path::new(checkout()).join("Cargo.toml");
    let server = |mode: &[&str]| {
        let mut args = vec!["run".to_string(), "--manifest-path".to_string()];
        args.push(manifest.to_string_lossy().into_owned());
        let run = ["--quiet", "--bin", "session-mcp", "--features", "dev", "--"];
        args.extend(run.iter().chain(mode).map(ToString::to_string));
        json!({ "command": "cargo", "args": args })
    };
    let root = root.to_string_lossy();
    let config = json!({
        "mcpServers": {
            "rusty": server(&["--attach"]),
            "rusty-headless": server(&["--project", &root]),
        }
    });
    let mut text = serde_json::to_string_pretty(&config).unwrap_or_default();
    text.push('\n');
    text
}
