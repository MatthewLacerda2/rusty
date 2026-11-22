pub mod attach;
pub mod bench;
pub mod embed;
pub mod progress;
pub mod protocol;
pub mod resources;
pub mod tools;

use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::path::Path;
use std::rc::Rc;

use serde_json::{json, Value};

use crate::session::Session;
use protocol::Incoming;

pub trait EvalBackend {
    fn eval(&self, line: &str) -> Result<String, String>;

    fn reopen(&self, _root: &Path) -> Option<Result<(), String>> {
        None
    }

    fn take_warnings(&self) -> Vec<String> {
        Vec::new()
    }
}

impl EvalBackend for Session {
    fn eval(&self, line: &str) -> Result<String, String> {
        Session::eval(self, line)
    }

    fn take_warnings(&self) -> Vec<String> {
        Session::take_warnings(self)
    }
}

pub fn run<R: BufRead, W: Write + 'static>(
    backend: &dyn EvalBackend,
    input: R,
    output: W,
) -> std::io::Result<()> {
    let output = Rc::new(RefCell::new(output));
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let token = progress::token(&line);
        if let Some(response) = progress::reporting(token, &output, || handle_line(backend, &line))
        {
            let mut output = output.borrow_mut();
            writeln!(output, "{response}")?;
            output.flush()?;
        }
    }
    Ok(())
}

fn handle_line(backend: &dyn EvalBackend, line: &str) -> Option<String> {
    match protocol::parse(line) {
        Some(Incoming::Request { id, method, params }) => {
            Some(dispatch(backend, &id, &method, &params))
        }
        Some(Incoming::Notification) => None,
        None => Some(protocol::parse_error()),
    }
}

fn dispatch(backend: &dyn EvalBackend, id: &Value, method: &str, params: &Value) -> String {
    let outcome: Result<Value, (i64, String)> = match method {
        "initialize" => Ok(initialize(params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools::tools_list()),
        "tools/call" => tools::tools_call(backend, params),
        "resources/list" => Ok(resources::resources_list()),
        "resources/read" => resources::resources_read(params),
        other => Err((-32601, format!("Method not found: {other}"))),
    };
    match outcome {
        Ok(result) => protocol::success(id, result),
        Err((code, message)) => protocol::error(id, code, &message),
    }
}

// Claude Code truncates long instructions: point at the resources, never inline the guide.
const INSTRUCTIONS: &str = "rusty game engine. Before working on a game, read the \
resource rusty://game-guide.md (the engine's rules for building a game), then \
rusty://scripting-api.md (the Lua API that the eval tool runs) before writing Lua.";

fn initialize(params: &Value) -> Value {
    let protocol_version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or("2024-11-05");
    json!({
        "protocolVersion": protocol_version,
        "capabilities": { "tools": {}, "resources": {} },
        "serverInfo": { "name": "rusty-session", "version": "0.1.0" },
        "instructions": INSTRUCTIONS,
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
        let instructions = resp["result"]["instructions"].as_str().unwrap();
        assert!(
            instructions.contains(resources::GAME_GUIDE_URI),
            "{instructions}"
        );
        assert!(
            instructions.contains(resources::SCRIPTING_API_URI),
            "{instructions}"
        );
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
        let out = progress::Captured::default();
        run(&s, input, out.clone()).expect("runs to EOF");
        let text = out.text();
        assert_eq!(text.lines().count(), 2);
    }
}

#[cfg(test)]
mod progress_tests;
