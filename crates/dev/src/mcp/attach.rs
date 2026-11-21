use std::cell::RefCell;
use std::io;

use serde_json::Value;

use super::EvalBackend;
use crate::command_channel::client::{self, Client};

pub struct SocketClient {
    explicit: Option<String>,
    warnings: RefCell<Vec<String>>,
}

impl SocketClient {
    pub fn new(explicit: Option<String>) -> Self {
        Self {
            explicit,
            warnings: RefCell::default(),
        }
    }

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
        let mut client =
            Client::connect(self.explicit.as_deref()).map_err(|e| self.no_engine(&e))?;
        use crate::bake::progress::listen;
        let framed = match listen::is_listening() {
            true => client.request_progress(line, &mut |text| listen::tell(text)),
            false => client.request(line),
        };
        let framed = framed.map_err(|e| self.no_engine(&e))?;
        let (outcome, warnings) = parse_framed(&framed);
        *self.warnings.borrow_mut() = warnings;
        outcome
    }

    fn take_warnings(&self) -> Vec<String> {
        std::mem::take(&mut *self.warnings.borrow_mut())
    }
}

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
        let backend = SocketClient::new(Some(absent_address()));
        let err = backend.eval("1 + 1").expect_err("no engine listening");
        assert!(err.contains("no rusty engine reachable"), "got: {err}");
    }

    fn absent_address() -> String {
        // lint: allow(test-temp-dir) a Unix socket path must stay short (sun_path is ~104 bytes)
        std::env::temp_dir()
            .join(format!("rusty-attach-absent-{}.sock", std::process::id()))
            .display()
            .to_string()
    }
}
