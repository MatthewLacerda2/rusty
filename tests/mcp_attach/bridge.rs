//! Tool calls forwarded over the socket, and resources served with no engine.

use super::fake::{drive, fake_engine};

#[test]
fn eval_is_forwarded_over_the_socket_and_the_reply_is_mapped() {
    let out = drive(
        fake_engine(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"eval","arguments":{"line":"Scene.CreateEntity(\"Widget\",\"Box\")"}}}"#,
        ],
    );
    assert_eq!(out[0]["result"]["isError"], false);
    // The fake engine echoes the forwarded line back, proving the exact Lua line
    // crossed the socket.
    assert_eq!(
        out[0]["result"]["content"][0]["text"],
        "Scene.CreateEntity(\"Widget\",\"Box\")"
    );
}

#[test]
fn multiple_calls_each_reconnect_and_succeed() {
    let out = drive(
        fake_engine(),
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"eval","arguments":{"line":"a"}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"eval","arguments":{"line":"b"}}}"#,
        ],
    );
    assert_eq!(out[0]["result"]["content"][0]["text"], "a");
    assert_eq!(out[1]["result"]["content"][0]["text"], "b");
}

#[test]
fn resources_read_works_without_any_engine() {
    // The scripting-API resource is embedded, so attach mode serves it even with no
    // window running — you can read the API doc before opening the engine.
    let bogus = if cfg!(windows) {
        "127.0.0.1:1".to_string()
    } else {
        std::env::temp_dir()
            .join("rusty-attach-noengine.sock")
            .display()
            .to_string()
    };
    let out = drive(
        bogus,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"rusty://scripting-api.md"}}"#,
        ],
    );
    let text = out[0]["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("# Scripting API reference"));
}
