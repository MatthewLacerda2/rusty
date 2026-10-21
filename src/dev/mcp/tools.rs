//! src/dev/mcp/tools.rs — the MCP `tools/list` and `tools/call` handlers.
//!
//! Two tools front the *one* evaluator: `eval` (run a line of the rusty Lua API
//! against the live edit-mode world) and `snapshot` (a convenience wrapper for
//! `Debug.Snapshot()`). Both ultimately call the active [`EvalBackend`], so the MCP
//! surface is the same control surface the console and the headless session already
//! drive — no second engine API exists, and the embed and attach modes go through one
//! code path here. The `bench_*` tools (rusty bench, #942) manage projects instead
//! and live in [`super::bench`].

use serde_json::{json, Value};

use super::EvalBackend;

/// The `tools/list` result: the tools and their JSON-Schema input shapes.
pub fn tools_list() -> Value {
    let mut list = json!({
        "tools": [
            {
                "name": "eval",
                "description": "Evaluate one line of the rusty Lua API against the \
                    live edit-mode engine world and return the result. This is the \
                    full engine control surface: create/edit/inspect entities, \
                    author assets, save scenes, step the sim, etc. See the \
                    scripting-api resource for the available namespaces and functions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "line": {
                            "type": "string",
                            "description": "One Lua API statement/expression, e.g. \
                                Scene.CreateEntity(\"Enemy\",\"Box\") or \
                                Transform.GetPosition(id)."
                        }
                    },
                    "required": ["line"]
                }
            },
            {
                "name": "snapshot",
                "description": "Return a JSON snapshot of the live world (entities, \
                    components, transforms, play state) so the agent can look around. \
                    Convenience wrapper for Debug.Snapshot().",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }
        ]
    });
    if let Some(tools) = list["tools"].as_array_mut() {
        tools.extend(super::bench::tools());
    }
    list
}

/// Handle `tools/call`. `Ok` is the MCP result value (including the in-band
/// `isError` flag for engine/eval failures, per MCP convention); `Err` is a
/// JSON-RPC protocol error `(code, message)` for a malformed call (unknown tool, or
/// `eval` missing its `line` argument).
pub fn tools_call(backend: &dyn EvalBackend, params: &Value) -> Result<Value, (i64, String)> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
    if let Some(outcome) = super::bench::call(backend, name, &arguments) {
        return outcome;
    }

    match name {
        "eval" => {
            let line = arguments
                .get("line")
                .and_then(Value::as_str)
                .ok_or_else(|| (-32602, "eval requires a string `line` argument".to_string()))?;
            Ok(eval_result(backend.eval(line), &backend.take_warnings()))
        }
        "snapshot" => Ok(eval_result(
            backend.eval("Debug.Snapshot()"),
            &backend.take_warnings(),
        )),
        other => Err((-32602, format!("Unknown tool: {other}"))),
    }
}

/// Map one evaluator outcome to an MCP tool-result value. A Lua/eval error is *not*
/// a protocol error: per MCP it is reported in-band with `isError: true` so the
/// agent sees the message and can recover.
fn eval_result(outcome: Result<String, String>, warnings: &[String]) -> Value {
    let (text, is_error) = match outcome {
        Ok(text) => (text, false),
        Err(err) => (err, true),
    };
    let mut content = vec![json!({ "type": "text", "text": text })];
    // A second block, so the result text itself (a snapshot's JSON) stays parseable.
    if !warnings.is_empty() {
        let text = format!("engine warnings:\n{}", warnings.join("\n"));
        content.push(json!({ "type": "text", "text": text }));
    }
    json!({ "content": content, "isError": is_error })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::session::Session;

    fn session() -> Session {
        Session::new("").expect("session boots in edit mode")
    }

    #[test]
    fn tools_list_advertises_eval_and_snapshot() {
        let list = tools_list();
        let names: Vec<&str> = list["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"eval"));
        assert!(names.contains(&"snapshot"));
    }

    #[test]
    fn eval_tool_returns_text_content_on_success() {
        let s = session();
        let params = json!({ "name": "eval", "arguments": { "line": "1 + 1" } });
        let result = tools_call(&s, &params).expect("eval ok");
        assert_eq!(result["content"][0]["text"], "2");
        assert_eq!(result["isError"], json!(false));
    }

    #[test]
    fn eval_tool_reports_engine_errors_in_band() {
        let s = session();
        let params = json!({ "name": "eval", "arguments": { "line": "nope %%%" } });
        let result = tools_call(&s, &params).expect("malformed lua is an in-band tool error");
        assert_eq!(result["isError"], json!(true));
        assert!(!result["content"][0]["text"].as_str().unwrap().is_empty());
    }

    #[test]
    fn eval_without_a_line_argument_is_a_protocol_error() {
        let s = session();
        let params = json!({ "name": "eval", "arguments": {} });
        let (code, _) = tools_call(&s, &params).expect_err("missing line is a protocol error");
        assert_eq!(code, -32602);
    }

    #[test]
    fn snapshot_tool_returns_the_world_json() {
        let s = session();
        let params = json!({ "name": "snapshot" });
        let result = tools_call(&s, &params).expect("snapshot ok");
        assert_eq!(result["isError"], json!(false));
        let snap: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(snap["play_state"], "editor");
    }

    #[test]
    fn engine_warnings_come_back_as_a_second_block() {
        crate::logging::init();
        let s = session();
        log::warn!("[Test] mcp: a mesh collider was skipped");
        let result = tools_call(&s, &json!({ "name": "snapshot" })).expect("snapshot ok");
        let warnings = result["content"][1]["text"].as_str().unwrap_or_default();
        assert!(warnings.starts_with("engine warnings:\n"));
        assert!(warnings.contains("[Test] mcp: a mesh collider was skipped"));
        let snap = result["content"][0]["text"].as_str().unwrap();
        assert!(
            serde_json::from_str::<Value>(snap).is_ok(),
            "the snapshot stays JSON"
        );
    }

    #[test]
    fn unknown_tool_is_a_protocol_error() {
        let s = session();
        let params = json!({ "name": "frobnicate" });
        let (code, msg) = tools_call(&s, &params).expect_err("unknown tool");
        assert_eq!(code, -32602);
        assert!(msg.contains("frobnicate"));
    }
}
