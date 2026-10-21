//! src/dev/mcp/attach.rs — the **attach** backend: drive a *running window* (#307).
//!
//! [`SocketClient`] is the second [`EvalBackend`] (the first is the in-process
//! [`Session`](crate::dev::session::Session)): instead of
//! evaluating in-process against a headless world, it forwards each line to a live
//! windowed engine over its command socket ([`crate::dev::command_channel`], #282) and
//! unwraps the framed reply. This is what turns the MCP bridge from "drive a headless
//! engine" into "drive the engine you are watching on screen."
//!
//! ## Lifecycle: stay up, connect per call
//! Claude Code does not auto-restart an stdio MCP server that exits, so this backend
//! never exits when the window goes away. It holds **no** persistent connection: each
//! [`eval`](SocketClient::eval) dials the socket fresh, sends one line, reads one
//! reply, and drops the connection. So when idle it holds zero engine resources, and a
//! window that closes and reopens is picked up transparently by the next call. A dial
//! failure (no window running) is reported in-band as a tool error, never a crash.

use std::cell::RefCell;
use std::io;

use serde_json::Value;

use super::EvalBackend;
use crate::dev::command_channel::client::{self, Client};

/// An [`EvalBackend`] that forwards to a running windowed engine's command socket.
///
/// This type holds no world of its own — it is a thin client over the socket.
pub struct SocketClient {
    /// An explicit `--attach <addr>` override (a socket path on unix, a `host:port` on
    /// windows), or `None` to use the shared default address the listener binds.
    explicit: Option<String>,
    /// The engine warnings the last reply carried, until [`EvalBackend::take_warnings`].
    warnings: RefCell<Vec<String>>,
}

impl SocketClient {
    /// Build an attach backend. `explicit` is the `--attach <addr>` override; `None`
    /// uses the default address.
    pub fn new(explicit: Option<String>) -> Self {
        Self {
            explicit,
            warnings: RefCell::default(),
        }
    }

    /// The in-band message for "no engine reachable" — naming where we looked and how
    /// to start one. Surfaced to the agent as a tool error so it can recover.
    fn no_engine(&self, err: &io::Error) -> String {
        let addr = client::address_label(self.explicit.as_deref());
        format!(
            "no rusty engine reachable at {addr} ({err}) — start it with \
             `cargo run --bin rusty --features dev`, then retry"
        )
    }
}

impl EvalBackend for SocketClient {
    fn eval(&self, line: &str) -> Result<String, String> {
        // Connect per call: no persistent socket is held between evals, so a window
        // restart is picked up transparently on the next call.
        let mut client =
            Client::connect(self.explicit.as_deref()).map_err(|e| self.no_engine(&e))?;
        let framed = client.request(line).map_err(|e| self.no_engine(&e))?;
        let (outcome, warnings) = parse_framed(&framed);
        *self.warnings.borrow_mut() = warnings;
        outcome
    }

    /// What the window raised while the last line was evaluated; the window's own
    /// console has those and everything else.
    fn take_warnings(&self) -> Vec<String> {
        std::mem::take(&mut *self.warnings.borrow_mut())
    }
}

/// Unwrap one framed reply line (`{"ok":true,"result":"…"}` /
/// `{"ok":false,"error":"…"}`, the byte-identical framing from
/// [`crate::dev::session::response_line`]) into the evaluator's `Result`, plus the
/// engine warnings its optional `"warnings"` key carries. A reply that doesn't parse
/// is itself an error (the engine should never send one — we never panic).
fn parse_framed(framed: &str) -> (Result<String, String>, Vec<String>) {
    let value: Value = match serde_json::from_str(framed) {
        Ok(value) => value,
        Err(e) => {
            let error = format!("malformed reply from engine: {e}: {framed:?}");
            return (Err(error), Vec::new());
        }
    };
    let warnings = value
        .get("warnings")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let outcome = if value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        Ok(field(&value, "result"))
    } else {
        Err(field(&value, "error"))
    };
    (outcome, warnings)
}

/// Pull a string field from the framed reply, tolerating its absence.
fn field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_framed_maps_ok_to_result() {
        let (outcome, warnings) = parse_framed(r#"{"ok":true,"result":"2"}"#);
        assert_eq!(outcome, Ok("2".to_string()));
        assert!(warnings.is_empty());
    }

    #[test]
    fn parse_framed_maps_not_ok_to_error() {
        assert_eq!(
            parse_framed(r#"{"ok":false,"error":"boom"}"#).0,
            Err("boom".to_string())
        );
    }

    #[test]
    fn parse_framed_passes_the_engine_warnings_through() {
        let reply = r#"{"ok":true,"result":"","warnings":["[Asset] skipped"]}"#;
        let (outcome, warnings) = parse_framed(reply);
        assert_eq!(outcome, Ok(String::new()));
        assert_eq!(warnings, ["[Asset] skipped"]);
    }

    #[test]
    fn parse_framed_rejects_garbage() {
        assert!(parse_framed("not json at all").0.is_err());
    }

    #[test]
    fn eval_reports_a_clean_error_when_nothing_is_listening() {
        // An address with no listener: connect fails and the failure surfaces as an
        // in-band `Err` naming how to start the engine — never a panic.
        let backend = SocketClient::new(Some(absent_address()));
        let err = backend.eval("1 + 1").expect_err("no engine listening");
        assert!(err.contains("no rusty engine reachable"), "got: {err}");
    }

    #[cfg(unix)]
    fn absent_address() -> String {
        // lint: allow(test-temp-dir) a Unix socket path must stay short (sun_path is ~104 bytes)
        std::env::temp_dir()
            .join(format!("rusty-attach-absent-{}.sock", std::process::id()))
            .display()
            .to_string()
    }
    #[cfg(windows)]
    fn absent_address() -> String {
        // Port 1 is privileged and unbound in CI: connect is refused immediately.
        "127.0.0.1:1".to_string()
    }
}
