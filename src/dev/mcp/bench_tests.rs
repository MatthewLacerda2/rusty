//! rusty bench on the MCP (#942): the four tools over a bench held in memory.

use std::cell::RefCell;

use super::*;
use crate::core::project::{engine_commit, ProjectFile, PROJECT_FILE, UNKNOWN_COMMIT};

/// Attach mode's shape: evaluates nothing, cannot move to another project.
struct Window;

impl EvalBackend for Window {
    fn eval(&self, _line: &str) -> Result<String, String> {
        Ok(String::new())
    }
}

fn fresh(name: &str) -> PathBuf {
    let dir = crate::test_temp::dir().join("mcp_bench").join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    crate::core::paths::canonicalize(&dir).unwrap()
}

/// Call `name` and return its text parsed as JSON, or its in-band error as `Err`.
fn run(
    backend: &dyn EvalBackend,
    bench: &mut Bench,
    name: &str,
    args: Value,
) -> Result<Value, String> {
    let result = call_on(backend, bench, name, &args, 100).expect("a well-formed call");
    let text = result["content"][0]["text"].as_str().unwrap().to_string();
    if result["isError"] == json!(true) {
        return Err(text);
    }
    Ok(serde_json::from_str(&text).unwrap())
}

#[test]
fn create_lists_the_project_and_forget_takes_it_off() {
    let dir = fresh("create");
    let mut bench = Bench::at(None);
    let made = run(
        &Window,
        &mut bench,
        "bench_create",
        json!({ "name": "Horde", "parent": dir }),
    );
    let root = dir.join("Horde");
    assert_eq!(made.unwrap()["root"], json!(root));
    assert!(root.join(PROJECT_FILE).is_file());
    let list = run(&Window, &mut bench, "bench_list", json!({})).unwrap();
    assert_eq!(list[0]["name"], "Horde");
    assert_eq!(list[0]["missing"], json!(false));
    assert_eq!(list[0]["ago"], "just now");
    let forgot = run(&Window, &mut bench, "bench_forget", json!({ "path": root }));
    assert_eq!(forgot.unwrap()["removed"], json!(true));
    assert_eq!(
        run(&Window, &mut bench, "bench_list", json!({})).unwrap(),
        json!([])
    );
}

#[test]
fn bad_calls_fail_in_band_or_as_protocol_errors() {
    let dir = fresh("bad");
    let mut bench = Bench::at(None);
    let args = json!({ "name": ".hidden", "parent": dir });
    assert!(run(&Window, &mut bench, "bench_create", args).is_err());
    let not_a_project = run(&Window, &mut bench, "bench_open", json!({ "path": dir }));
    assert!(not_a_project.unwrap_err().contains("not a rusty project"));
    let missing = call_on(&Window, &mut bench, "bench_open", &json!({}), 0);
    assert_eq!(missing.unwrap_err().0, -32602);
    assert!(
        call(&Window, "eval", &json!({})).is_none(),
        "not a bench tool"
    );
}

#[test]
fn attach_mode_answers_with_the_relaunch_command() {
    let dir = fresh("attach");
    let mut bench = Bench::at(None);
    let root = bench.create(&dir, "Horde", 1).unwrap();
    let opened = run(&Window, &mut bench, "bench_open", json!({ "path": root })).unwrap();
    assert_eq!(opened["opened"], json!(false));
    let command = format!("rusty --project {}", root.display());
    assert_eq!(opened["relaunch"], json!(command));
}

#[test]
fn another_engines_project_opens_only_when_confirmed() {
    let dir = fresh("mismatch");
    let file = ProjectFile {
        engine_commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
        ..ProjectFile::default()
    };
    file.write(&dir.join(PROJECT_FILE)).unwrap();
    let mut bench = Bench::at(None);
    let asked = run(&Window, &mut bench, "bench_open", json!({ "path": dir }));
    if engine_commit() == UNKNOWN_COMMIT {
        assert!(asked.is_ok(), "a build without git can't compare");
        return;
    }
    assert!(asked.unwrap_err().contains("confirm: true"));
    assert!(bench.recent.projects.is_empty(), "nothing opened");
    let args = json!({ "path": dir, "confirm": true });
    let opened = run(&Window, &mut bench, "bench_open", args).unwrap();
    assert!(opened["warning"].is_string());
}

/// Embed mode's shape, minus the move: records the root `bench_open` hands it. The
/// real `Embedded::switch` moves the process's working directory, so it is proven in
/// its own process instead (`tests/mcp_bench_embed.rs`, #1042).
#[derive(Default)]
struct Mover(RefCell<Option<PathBuf>>);

impl EvalBackend for Mover {
    fn eval(&self, _line: &str) -> Result<String, String> {
        Ok(String::new())
    }

    fn reopen(&self, root: &Path) -> Option<Result<(), String>> {
        *self.0.borrow_mut() = Some(root.to_path_buf());
        Some(Ok(()))
    }
}

#[test]
fn embed_mode_hands_the_opened_root_to_the_backend() {
    let dir = fresh("embed");
    let mut bench = Bench::at(None);
    let root = bench.create(&dir, "Horde", 1).unwrap();
    let mover = Mover::default();
    let opened = run(&mover, &mut bench, "bench_open", json!({ "path": root })).unwrap();
    assert_eq!(opened["opened"], json!(true));
    assert_eq!(opened["root"], json!(root));
    assert_eq!(mover.0.take(), Some(root.clone()));
    assert_eq!(bench.recent.projects[0].path, root);
}
