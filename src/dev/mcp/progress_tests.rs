//! A request with a progress token hears its bake's lines as
//! `notifications/progress` before its response; one without hears none (#1999).

use serde_json::{json, Value};

use super::progress::{token, Captured};
use super::{run, EvalBackend};
use crate::dev::bake::progress::listen::tell;

/// A slow verb, mocked: it tells two progress lines, as a bake's log does, and
/// returns.
struct SlowBake;

impl EvalBackend for SlowBake {
    fn eval(&self, _line: &str) -> Result<String, String> {
        tell("[Lighting] 0:05 lightmaps 40%");
        tell("[Lighting] 0:10 lightmaps 80%");
        Ok("done".to_string())
    }
}

fn call(meta: Value) -> String {
    json!({
        "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": { "name": "eval", "arguments": { "line": "Lighting.Generate()" }, "_meta": meta },
    })
    .to_string()
}

/// Every line `run` wrote for `request`, parsed.
fn exchange(request: &str) -> Vec<Value> {
    let out = Captured::default();
    run(&SlowBake, format!("{request}\n").as_bytes(), out.clone()).expect("runs to EOF");
    let text = out.text();
    text.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn a_request_with_a_token_hears_the_bake_before_its_response() {
    let lines = exchange(&call(json!({ "progressToken": "bake-1" })));
    assert_eq!(lines.len(), 3, "{lines:?}");
    for (i, note) in lines[..2].iter().enumerate() {
        assert_eq!(note["method"], "notifications/progress");
        assert!(note.get("id").is_none(), "a notification has no id");
        assert_eq!(note["params"]["progressToken"], "bake-1");
        assert_eq!(note["params"]["progress"], json!(i + 1), "it only grows");
    }
    let message = lines[1]["params"]["message"].as_str().unwrap();
    assert_eq!(message, "[Lighting] 0:10 lightmaps 80%");
    assert_eq!(lines[2]["id"], json!(7));
    assert_eq!(lines[2]["result"]["content"][0]["text"], "done");
}

#[test]
fn a_request_without_a_token_hears_nothing_but_its_response() {
    let lines = exchange(&call(json!({})));
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["result"]["content"][0]["text"], "done");
}

#[test]
fn an_integer_token_counts_and_a_notification_line_never_asks() {
    assert_eq!(token(&call(json!({ "progressToken": 3 }))), Some(json!(3)));
    assert_eq!(token(&call(json!({ "progressToken": {} }))), None);
    let note = r#"{"jsonrpc":"2.0","method":"x","params":{"_meta":{"progressToken":1}}}"#;
    assert_eq!(
        token(note),
        None,
        "a notification gets no response to report before"
    );
}
