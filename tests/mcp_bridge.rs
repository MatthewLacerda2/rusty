//! MCP stdio bridge end-to-end (#288).
//! Gated on `dev`: a no-op under a plain `cargo test`, real under `--features dev`.
//! Drives `rusty::dev::mcp::run` with canned newline-delimited JSON-RPC against a
//! live `Session`, asserting the response lines on stdout.
#![cfg(feature = "dev")]

use serde_json::Value;

use rusty::dev::mcp;
use rusty::dev::session::Session;

/// Run a sequence of JSON-RPC request/notification lines through the bridge against a
/// fresh edit-mode session and return the parsed response lines (notifications
/// produce nothing, so the count is requests-only).
fn drive(lines: &[&str]) -> Vec<Value> {
    let sess = Session::new("").expect("session boots in edit mode");
    let input = lines.join("\n").into_bytes();
    let mut out = Vec::new();
    mcp::run(&sess, &input[..], &mut out).expect("runs to EOF");
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("each response line is JSON"))
        .collect()
}

#[test]
fn initialize_reports_server_info_and_protocol_version() {
    let out = drive(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
    ]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0]["id"], 1);
    assert_eq!(out[0]["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(out[0]["result"]["serverInfo"]["name"], "rusty-session");
    assert_eq!(
        out[0]["result"]["capabilities"]["tools"],
        serde_json::json!({})
    );
}

#[test]
fn initialized_notification_produces_no_line() {
    let out = drive(&[
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
    ]);
    // Only the ping request yields a line; the notification is silent.
    assert_eq!(out.len(), 1);
    assert_eq!(out[0]["result"], serde_json::json!({}));
}

#[test]
fn tools_list_advertises_eval_and_snapshot() {
    let out = drive(&[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#]);
    let names: Vec<&str> = out[0]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"eval"));
    assert!(names.contains(&"snapshot"));
}

#[test]
fn eval_authors_an_entity_then_snapshot_sees_it() {
    let out = drive(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"eval","arguments":{"line":"Scene.CreateEntity(\"Widget\",\"Box\")"}}}"#,
        // The created id is rendered as the text content; reuse a known position set.
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"snapshot"}}"#,
    ]);
    assert_eq!(out[0]["result"]["isError"], false);
    let snap: Value =
        serde_json::from_str(out[1]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(snap["play_state"], "editor");
    let found = snap["entities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "Widget");
    assert!(found, "the authored entity must appear in the snapshot");
}

#[test]
fn eval_of_a_simple_expression_returns_text_content() {
    let out = drive(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"eval","arguments":{"line":"1 + 1"}}}"#,
    ]);
    assert_eq!(out[0]["result"]["content"][0]["text"], "2");
    assert_eq!(out[0]["result"]["isError"], false);
}

#[test]
fn malformed_lua_is_an_in_band_tool_error() {
    let out = drive(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"eval","arguments":{"line":"this is not lua %%%"}}}"#,
    ]);
    assert_eq!(out[0]["result"]["isError"], true);
    assert!(!out[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .is_empty());
}

#[test]
fn resources_read_returns_the_scripting_api_markdown() {
    let out = drive(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"rusty://scripting-api.md"}}"#,
    ]);
    let text = out[0]["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("# Scripting API reference"));
}
