//! src/dev/mcp/resources.rs — the MCP `resources/list` and `resources/read` handlers.
//!
//! Two resources are exposed, each read from the engine checkout on each request
//! (#931), so it is always the doc the checkout holds and a doc edit rebuilds
//! nothing:
//!
//! * the engine's Lua scripting-API reference — `docs/api/`, assembled into one
//!   document (#569) — which the agent reads to learn what it can drive through the
//!   `eval` tool;
//! * the game guide, `docs/game-agent.md` (#1313): the engine's rules for an agent
//!   building a game, which a game project's `CLAUDE.md` points at.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// The canonical URI for the scripting-API reference resource.
pub const SCRIPTING_API_URI: &str = "rusty://scripting-api.md";
/// The canonical URI for the game guide resource.
pub const GAME_GUIDE_URI: &str = "rusty://game-guide.md";

/// The game guide in this build's engine checkout.
fn game_guide_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("game-agent.md")
}

/// The `resources/list` result: the game guide and the scripting-API reference.
pub fn resources_list() -> Value {
    json!({
        "resources": [{
            "uri": GAME_GUIDE_URI,
            "name": "rusty game guide",
            "description": "The engine's rules for building a game on rusty. Read it first.",
            "mimeType": "text/markdown",
        }, {
            "uri": SCRIPTING_API_URI,
            "name": "rusty scripting API reference",
            "description": "The full Lua API surface the engine is driven through.",
            "mimeType": "text/markdown",
        }]
    })
}

/// Handle `resources/read`. `Ok` is the MCP result with the resource contents; `Err`
/// is a JSON-RPC protocol error `(code, message)` for an unknown URI, or an internal
/// error when the checkout's doc can't be read.
pub fn resources_read(params: &Value) -> Result<Value, (i64, String)> {
    read_from(
        params,
        &crate::scripting::api_doc::api_doc_dir(),
        &game_guide_path(),
    )
}

/// [`resources_read`] with the reference assembled from `doc_dir` and the guide read
/// from `guide`.
fn read_from(params: &Value, doc_dir: &Path, guide: &Path) -> Result<Value, (i64, String)> {
    let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
    let text = match uri {
        SCRIPTING_API_URI => crate::scripting::api_doc::assemble(doc_dir),
        GAME_GUIDE_URI => std::fs::read_to_string(guide)
            .map_err(|e| format!("Failed to read {}: {e}", guide.display())),
        _ => return Err((-32602, format!("Unknown resource: {uri}"))),
    }
    .map_err(|e| (-32603, e))?;
    Ok(json!({
        "contents": [{
            "uri": uri,
            "mimeType": "text/markdown",
            "text": text,
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_list_advertises_the_guide_and_the_scripting_api() {
        let list = resources_list();
        assert_eq!(list["resources"][0]["uri"], GAME_GUIDE_URI);
        assert_eq!(list["resources"][1]["uri"], SCRIPTING_API_URI);
        assert_eq!(list["resources"][1]["mimeType"], "text/markdown");
    }

    #[test]
    fn read_returns_the_game_guide_from_the_checkout() {
        let result = resources_read(&json!({ "uri": GAME_GUIDE_URI })).expect("reads");
        let text = result["contents"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("# Building a game on rusty"), "{text}");
        assert!(
            text.contains(SCRIPTING_API_URI),
            "the guide points at the API"
        );
        assert_eq!(result["contents"][0]["uri"], GAME_GUIDE_URI);
        let absent = Path::new("/nonexistent/rusty/docs/game-agent.md");
        let params = json!({ "uri": GAME_GUIDE_URI });
        let (code, _) = read_from(&params, Path::new("."), absent).expect_err("no guide");
        assert_eq!(code, -32603);
    }

    #[test]
    fn read_returns_the_assembled_markdown() {
        let params = json!({ "uri": SCRIPTING_API_URI });
        let result = resources_read(&params).expect("known uri reads");
        let text = result["contents"][0]["text"].as_str().unwrap();
        assert!(!text.is_empty(), "the assembled doc must have content");
        assert_eq!(result["contents"][0]["uri"], SCRIPTING_API_URI);
    }

    #[test]
    fn read_of_an_unknown_uri_is_a_protocol_error() {
        let params = json!({ "uri": "rusty://nope.md" });
        let (code, msg) = resources_read(&params).expect_err("unknown uri");
        assert_eq!(code, -32602);
        assert!(msg.contains("nope.md"));
    }

    #[test]
    fn an_unreadable_doc_is_an_internal_error() {
        let params = json!({ "uri": SCRIPTING_API_URI });
        let absent = Path::new("/nonexistent/rusty/docs/api");
        let (code, msg) = read_from(&params, absent, absent).expect_err("no index.md");
        assert_eq!(code, -32603);
        assert!(msg.contains("index.md"), "{msg}");
    }
}
