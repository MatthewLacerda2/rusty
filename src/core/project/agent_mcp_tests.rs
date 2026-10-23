//! The `.mcp.json` an edit-open keeps up to date (#1313): the engine's two entries,
//! and nothing else of the file.

use serde_json::{json, Value};

use super::agent::{merge, write_for, SERVERS};
use super::agent_tests::fresh;
use super::*;

const ENGINE: &str = "/engine";

fn read(root: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root.join(MCP_FILE)).unwrap()).unwrap()
}

#[test]
fn the_servers_run_the_checkouts_built_binary_in_attach_and_embed_mode() {
    let mut config = json!({});
    merge(&mut config, "/games/horde", Path::new(ENGINE)).unwrap();
    let servers = &config["mcpServers"];
    let binary = "/engine/target/debug/session-mcp";
    assert_eq!(
        servers[SERVERS[0]],
        json!({ "command": binary, "args": ["--attach"] })
    );
    let headless = json!({ "command": binary, "args": ["--project", "/games/horde"] });
    assert_eq!(servers[SERVERS[1]], headless);
}

#[test]
fn an_edit_open_keeps_the_games_own_servers_and_keys() {
    let root = fresh("shared");
    let blender = json!({ "command": "uvx", "args": ["blender-mcp"] });
    let own = json!({
        "mcpServers": { "blender": blender, "rusty": { "command": "stale" } },
        "note": "ours",
    });
    std::fs::write(root.join(MCP_FILE), own.to_string()).unwrap();
    let lines = write_for(&root, true, Path::new(ENGINE));
    assert!(
        !lines.iter().any(|l| l.contains(MCP_FILE)),
        "rewritten: {lines:?}"
    );
    let config = read(&root);
    assert_eq!(config["mcpServers"]["blender"], blender);
    assert_eq!(config["note"], "ours");
    assert_ne!(config["mcpServers"]["rusty"]["command"], "stale");
    let before = std::fs::read_to_string(root.join(MCP_FILE)).unwrap();
    write_for(&root, true, Path::new(ENGINE));
    let after = std::fs::read_to_string(root.join(MCP_FILE)).unwrap();
    assert_eq!(before, after, "a second open changes nothing");
}

#[test]
fn a_new_mcp_json_is_reported_created() {
    let root = fresh("created");
    let lines = write_for(&root, true, Path::new(ENGINE));
    assert!(lines.contains(&format!("created {MCP_FILE}")), "{lines:?}");
    assert_eq!(read(&root)["mcpServers"].as_object().unwrap().len(), 2);
}

#[test]
fn an_unparseable_mcp_json_is_left_alone_with_a_warning() {
    for (name, text) in [("broken", "{ not json"), ("array", "[1]")] {
        let root = fresh(name);
        std::fs::write(root.join(MCP_FILE), text).unwrap();
        let lines = write_for(&root, true, Path::new(ENGINE));
        let warned = format!("warning: {MCP_FILE} not written");
        assert!(lines.iter().any(|l| l.starts_with(&warned)), "{lines:?}");
        assert_eq!(std::fs::read_to_string(root.join(MCP_FILE)).unwrap(), text);
    }
}

#[test]
fn without_dev_no_mcp_json_is_written() {
    let root = fresh("player");
    assert_eq!(
        write_for(&root, false, Path::new(ENGINE)),
        [format!("created {CLAUDE_FILE}")]
    );
    assert!(!root.join(MCP_FILE).exists());
}
