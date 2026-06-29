//! src/dev/mcp/protocol.rs — JSON-RPC 2.0 plumbing for the MCP stdio transport.
//!
//! Pure, unit-testable framing: parse one newline-delimited line into an
//! [`Incoming`] (a `Request` carries an `id`; a `Notification` does not), and build
//! the success / error response envelopes. The actual method dispatch lives in
//! [`super`]; this module only knows the wire shape, never the engine.

use serde_json::{json, Value};

/// One parsed JSON-RPC message. A request has an `id` and expects exactly one
/// response; a notification has no `id` and gets none.
pub enum Incoming {
    /// A request: `id` is preserved verbatim (int / string / null), and the method
    /// plus its `params` (defaulting to `null` when absent) are carried through.
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// A notification: a method + params with no `id`, so no response is written.
    Notification { method: String, params: Value },
}

/// Parse one wire line into an [`Incoming`].
///
/// Returns `None` only when the line cannot be understood as JSON-RPC at all
/// (unparseable JSON, or a missing/non-string `method`) — the caller turns that into
/// a parse-error response. A well-formed object with an `id` is a `Request`; without
/// one, a `Notification`. The error case carries no detail, so `Option` is the
/// honest return type (a `Result<_, ()>` would just be a worse spelling of it).
pub fn parse(line: &str) -> Option<Incoming> {
    let value: Value = serde_json::from_str(line).ok()?;
    let method = value.get("method").and_then(Value::as_str)?.to_string();
    let params = value.get("params").cloned().unwrap_or(Value::Null);

    Some(match value.get("id") {
        Some(id) => Incoming::Request {
            id: id.clone(),
            method,
            params,
        },
        None => Incoming::Notification { method, params },
    })
}

/// A successful response envelope: `{"jsonrpc":"2.0","id":<id>,"result":<result>}`.
pub fn success(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

/// An error response envelope: `{"jsonrpc":"2.0","id":<id>,"error":{...}}`.
pub fn error(id: &Value, code: i64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
    .to_string()
}

/// The parse-error response for a line we couldn't even extract an `id` from. Per
/// JSON-RPC, an unidentifiable request reports `id: null`.
pub fn parse_error() -> String {
    error(&Value::Null, -32700, "Parse error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_request_with_params_and_preserves_id() {
        let line = r#"{"jsonrpc":"2.0","id":7,"method":"ping","params":{"x":1}}"#;
        match parse(line).expect("valid request") {
            Incoming::Request { id, method, params } => {
                assert_eq!(id, json!(7));
                assert_eq!(method, "ping");
                assert_eq!(params["x"], json!(1));
            }
            _ => panic!("expected a request"),
        }
    }

    #[test]
    fn a_string_id_is_preserved_verbatim() {
        let line = r#"{"jsonrpc":"2.0","id":"abc","method":"ping"}"#;
        match parse(line).expect("valid request") {
            Incoming::Request { id, .. } => assert_eq!(id, json!("abc")),
            _ => panic!("expected a request"),
        }
    }

    #[test]
    fn a_message_without_id_is_a_notification() {
        let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        match parse(line).expect("valid notification") {
            Incoming::Notification { method, params } => {
                assert_eq!(method, "notifications/initialized");
                assert_eq!(params, Value::Null);
            }
            _ => panic!("expected a notification"),
        }
    }

    #[test]
    fn unparseable_json_is_an_error() {
        assert!(parse("not json at all {{{").is_none());
    }

    #[test]
    fn a_message_without_a_method_is_an_error() {
        assert!(parse(r#"{"jsonrpc":"2.0","id":1}"#).is_none());
    }

    #[test]
    fn success_envelope_carries_id_and_result() {
        let line = success(&json!(3), json!({"ok": true}));
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], json!(3));
        assert_eq!(v["result"]["ok"], json!(true));
        assert!(v.get("error").is_none());
    }

    #[test]
    fn error_envelope_carries_code_and_message() {
        let line = error(&json!(4), -32601, "Method not found: foo");
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["id"], json!(4));
        assert_eq!(v["error"]["code"], json!(-32601));
        assert_eq!(v["error"]["message"], "Method not found: foo");
        assert!(v.get("result").is_none());
    }

    #[test]
    fn parse_error_reports_null_id() {
        let v: Value = serde_json::from_str(&parse_error()).unwrap();
        assert_eq!(v["id"], Value::Null);
        assert_eq!(v["error"]["code"], json!(-32700));
    }
}
