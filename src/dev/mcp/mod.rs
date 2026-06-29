//! src/dev/mcp/ — MCP stdio server over the one edit-mode evaluator (#288).
//!
//! "Blender-MCP, but for game development." This is the **third framing** over the
//! engine's *single* evaluator ([`crate::dev::session::Session::eval`]), exactly
//! parallel to the two that already exist:
//!   - [`crate::dev::session`]     — line-Lua-in / line-JSON-out over stdin/stdout.
//!   - [`crate::dev::command_channel`] — the same protocol over a socket (windowed).
//!   - **this** — the same `Session::eval`, spoken as Model Context Protocol so
//!     Claude Code attaches to the engine natively, with no second control surface.
//!
//! ## Design: in-process, no-dep, synchronous
//! The session is sync and single-threaded (`Rc<RefCell<…>>`, not `Send`), so there
//! is no async runtime and no subprocess: we [`run`] a plain read-line / dispatch /
//! write-line loop on one thread that calls [`Session::eval`] in-process. The wire
//! format is MCP's stdio transport — newline-delimited JSON-RPC 2.0 — and the only
//! dependency is `serde_json` (already in the tree). No tokio, no rmcp, no Node.
//!
//! ## stdout is JSON-RPC only
//! Exactly one JSON-RPC message per line goes to `output`; nothing else. Engine logs
//! already go to stderr (env_logger) or the in-memory `ConsoleLogs` (Lua `print` /
//! `Debug.*`), never stdout — so the channel stays clean. Any diagnostics this layer
//! emits use `eprintln!` (stderr).
//!
//! ## Robustness
//! A request (has an `id`) gets exactly one response line; a notification (no `id`)
//! gets none. A malformed line yields a JSON-RPC parse-error response and the loop
//! continues — a bad message never panics and never tears down the server. The loop
//! ends only at EOF (stdin closed).

pub mod protocol;
pub mod resources;
pub mod tools;

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use crate::dev::session::Session;
use protocol::Incoming;

/// Run the MCP stdio server: read newline-delimited JSON-RPC messages from `input`,
/// dispatch each against the live `session`, and write one response line per request
/// to `output` (notifications produce nothing). Blank lines are skipped. The loop
/// ends at EOF; an I/O error on the channel itself is fatal and propagates.
pub fn run<R: BufRead, W: Write>(
    session: &Session,
    input: R,
    mut output: W,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_line(session, &line) {
            writeln!(output, "{response}")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// Process one wire line, returning the response to write (a request) or `None` (a
/// notification, or a blank/handled line). Never panics: an unparseable line becomes
/// a parse-error response.
fn handle_line(session: &Session, line: &str) -> Option<String> {
    match protocol::parse(line) {
        Some(Incoming::Request { id, method, params }) => {
            Some(dispatch(session, &id, &method, &params))
        }
        // Notifications are processed for side effects only; `notifications/*` here
        // carry none, so we simply acknowledge by writing nothing back.
        Some(Incoming::Notification { .. }) => None,
        None => Some(protocol::parse_error()),
    }
}

/// Route a parsed request to its handler and frame the JSON-RPC response. Method
/// handlers return either a result `Value` (success) or a `(code, message)` protocol
/// error; tool/resource *execution* errors are reported in-band, not here.
fn dispatch(session: &Session, id: &Value, method: &str, params: &Value) -> String {
    let outcome: Result<Value, (i64, String)> = match method {
        "initialize" => Ok(initialize(params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools::tools_list()),
        "tools/call" => tools::tools_call(session, params),
        "resources/list" => Ok(resources::resources_list()),
        "resources/read" => resources::resources_read(params),
        other => Err((-32601, format!("Method not found: {other}"))),
    };
    match outcome {
        Ok(result) => protocol::success(id, result),
        Err((code, message)) => protocol::error(id, code, &message),
    }
}

/// The `initialize` result: echo the client's protocol version (defaulting to the
/// baseline `2024-11-05`), advertise the tools + resources capabilities, and
/// identify the server.
fn initialize(params: &Value) -> Value {
    let protocol_version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("2024-11-05");
    json!({
        "protocolVersion": protocol_version,
        "capabilities": { "tools": {}, "resources": {} },
        "serverInfo": { "name": "rusty-session", "version": "0.1.0" },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session::new("").expect("session boots in edit mode")
    }

    fn parse_line(line: &str) -> Value {
        serde_json::from_str(line).expect("response is JSON")
    }

    #[test]
    fn initialize_echoes_version_and_identifies_the_server() {
        let s = session();
        let req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#;
        let resp = parse_line(&handle_line(&s, req).unwrap());
        assert_eq!(resp["id"], json!(1));
        assert_eq!(resp["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(resp["result"]["serverInfo"]["name"], "rusty-session");
    }

    #[test]
    fn initialize_defaults_the_protocol_version() {
        let s = session();
        let req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let resp = parse_line(&handle_line(&s, req).unwrap());
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    }

    #[test]
    fn a_notification_produces_no_response() {
        let s = session();
        let note = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        assert!(handle_line(&s, note).is_none());
    }

    #[test]
    fn ping_returns_an_empty_result() {
        let s = session();
        let req = r#"{"jsonrpc":"2.0","id":9,"method":"ping"}"#;
        let resp = parse_line(&handle_line(&s, req).unwrap());
        assert_eq!(resp["result"], json!({}));
    }

    #[test]
    fn an_unknown_method_is_method_not_found() {
        let s = session();
        let req = r#"{"jsonrpc":"2.0","id":2,"method":"no/such"}"#;
        let resp = parse_line(&handle_line(&s, req).unwrap());
        assert_eq!(resp["error"]["code"], json!(-32601));
    }

    #[test]
    fn a_malformed_line_yields_a_parse_error() {
        let s = session();
        let resp = parse_line(&handle_line(&s, "{ not json").unwrap());
        assert_eq!(resp["id"], Value::Null);
        assert_eq!(resp["error"]["code"], json!(-32700));
    }

    #[test]
    fn run_writes_one_line_per_request_and_skips_notifications() {
        let s = session();
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            "\n",
        )
        .as_bytes();
        let mut out = Vec::new();
        run(&s, input, &mut out).expect("runs to EOF");
        let text = String::from_utf8(out).unwrap();
        // Two requests -> two lines; the notification writes nothing.
        assert_eq!(text.lines().count(), 2);
    }
}
