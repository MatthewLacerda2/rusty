//! `bench_open` in embed mode, end to end (#942, #1042).
//! Gated on `dev`. Opening a project moves the process's working directory
//! (`core::project::open`), so the real `Embedded::switch` runs in a `session-mcp`
//! child: under `cargo test` (and cargo-mutants) every test of this binary shares one
//! process, and a moved working directory sends every later relative path into the
//! opened project.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

/// One `tools/call` request line.
fn call(id: u64, name: &str, arguments: Value) -> String {
    let params = json!({ "name": name, "arguments": arguments });
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": params }).to_string()
}

/// Run `session-mcp` on `start` with `lines` on stdin, its user config under `home`,
/// and return its response lines.
fn drive(start: &Path, home: &Path, lines: &[String]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_session-mcp"))
        .args(["--project".as_ref(), start.as_os_str(), "--empty".as_ref()])
        .current_dir(start)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn session-mcp");
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(lines.join("\n").as_bytes()).unwrap();
    drop(stdin);
    let out = child.wait_with_output().expect("session-mcp runs to EOF");
    assert!(out.status.success(), "session-mcp exited {}", out.status);
    let stdout = String::from_utf8(out.stdout).unwrap();
    stdout
        .lines()
        .map(|l| serde_json::from_str(l).expect("each response line is JSON"))
        .collect()
}

/// A tool result's text.
fn text(response: &Value) -> &str {
    response["result"]["content"][0]["text"].as_str().unwrap()
}

#[test]
fn bench_open_moves_the_session_to_the_project() {
    let dir = crate::temp::dir();
    let (start, home, parent) = (dir.join("start"), dir.join("home"), dir.join("bench"));
    for d in [&start, &home, &parent] {
        std::fs::create_dir_all(d).unwrap();
    }
    let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2024-11-05" } });
    let out = drive(
        &start,
        &home,
        &[
            initialize.to_string(),
            call(
                2,
                "bench_create",
                json!({ "name": "Horde", "parent": parent }),
            ),
            call(3, "bench_open", json!({ "path": parent.join("Horde") })),
            call(4, "eval", json!({ "line": "1 + 1" })),
        ],
    );
    assert_eq!(out.len(), 4, "{out:?}");
    assert_eq!(out[1]["result"]["isError"], false, "{}", text(&out[1]));
    let created: Value = serde_json::from_str(text(&out[1])).unwrap();
    let root = Path::new(created["root"].as_str().unwrap()).to_path_buf();
    assert_eq!(out[2]["result"]["isError"], false, "{}", text(&out[2]));
    let opened: Value = serde_json::from_str(text(&out[2])).unwrap();
    assert_eq!(opened["opened"], json!(true));
    assert_eq!(opened["root"], json!(root));
    assert!(
        root.join(rusty::scene::DEFAULT_SCENE_PATH).is_file(),
        "the opened project's default scene is seeded"
    );
    assert_eq!(text(&out[3]), "2", "the new session answers");
}
