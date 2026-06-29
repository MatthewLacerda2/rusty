//! src/dev/mcp/resources.rs — the MCP `resources/list` and `resources/read` handlers.
//!
//! One resource is exposed: the engine's Lua scripting-API reference, embedded at
//! compile time with `include_str!` so the binary always serves the doc that matches
//! its own build (no path lookup, no drift). The agent reads it to learn the
//! namespaces and functions it can drive through the `eval` tool.

use serde_json::{json, Value};

/// The canonical URI for the scripting-API reference resource.
const SCRIPTING_API_URI: &str = "rusty://scripting-api.md";

/// The scripting-API reference, baked into the binary so it ships with the build and
/// can never lag the engine it documents.
const SCRIPTING_API: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/scripting-api.md"
));

/// The `resources/list` result: the single scripting-API reference resource.
pub fn resources_list() -> Value {
    json!({
        "resources": [{
            "uri": SCRIPTING_API_URI,
            "name": "rusty scripting API reference",
            "description": "The full Lua API surface the engine is driven through.",
            "mimeType": "text/markdown",
        }]
    })
}

/// Handle `resources/read`. `Ok` is the MCP result with the resource contents; `Err`
/// is a JSON-RPC protocol error `(code, message)` for an unknown URI.
pub fn resources_read(params: &Value) -> Result<Value, (i64, String)> {
    let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
    if uri == SCRIPTING_API_URI {
        Ok(json!({
            "contents": [{
                "uri": SCRIPTING_API_URI,
                "mimeType": "text/markdown",
                "text": SCRIPTING_API,
            }]
        }))
    } else {
        Err((-32602, format!("Unknown resource: {uri}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_list_advertises_the_scripting_api() {
        let list = resources_list();
        assert_eq!(list["resources"][0]["uri"], SCRIPTING_API_URI);
        assert_eq!(list["resources"][0]["mimeType"], "text/markdown");
    }

    #[test]
    fn read_returns_the_embedded_markdown() {
        let params = json!({ "uri": SCRIPTING_API_URI });
        let result = resources_read(&params).expect("known uri reads");
        let text = result["contents"][0]["text"].as_str().unwrap();
        assert!(!text.is_empty(), "the embedded doc must have content");
        assert_eq!(result["contents"][0]["uri"], SCRIPTING_API_URI);
    }

    #[test]
    fn read_of_an_unknown_uri_is_a_protocol_error() {
        let params = json!({ "uri": "rusty://nope.md" });
        let (code, msg) = resources_read(&params).expect_err("unknown uri");
        assert_eq!(code, -32602);
        assert!(msg.contains("nope.md"));
    }
}
