//! src/dev/mcp/bench.rs — **rusty bench** on the MCP: list, create, open and forget
//! projects without the editor's picker (#942).
//!
//! Four `bench_*` tools over [`Bench`] (`core::project::bench`), the code the
//! picker calls too. `bench_open` checks the folder as the picker does; a project
//! a newer engine (or a branch build) last opened is *reported*, and opens only when
//! the call repeats with `confirm: true` (the picker's *Open anyway*). Then the backend moves to it
//! ([`EvalBackend::reopen`]): embed mode drives the new project at once, attach
//! mode answers with the command that relaunches the window on it.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::EvalBackend;
use crate::core::project::bench::{check_existing, recent::ago, Bench};

/// A protocol error: `(code, message)`.
type Fault = (i64, String);

/// The four tools' `tools/list` entries.
pub fn tools() -> Vec<Value> {
    let path = json!({ "type": "string", "description": "The project folder, absolute." });
    let schema = |props: Value, required: &[&str]| json!({ "type": "object", "properties": props, "required": required });
    vec![
        json!({
            "name": "bench_list",
            "description": "rusty bench: the projects opened before, most recent first, \
                each with name, path, opened_at (Unix seconds), ago and missing.",
            "inputSchema": schema(json!({}), &[]),
        }),
        json!({
            "name": "bench_create",
            "description": "rusty bench: make a new project folder `name` under `parent` \
                (the editor's New Project) and list it. Does not open it: bench_open does.",
            "inputSchema": schema(
                json!({ "name": { "type": "string" }, "parent": path }),
                &["name", "parent"],
            ),
        }),
        json!({
            "name": "bench_open",
            "description": "rusty bench: open an existing project and drive it from now \
                on (embed mode; attach mode returns the command that relaunches the window \
                on it). A project last opened by a newer engine or a branch build is \
                reported; repeat with confirm: true to open it anyway. An older one \
                opens at once: an engine upgrade.",
            "inputSchema": schema(
                json!({ "path": path, "confirm": { "type": "boolean" } }),
                &["path"],
            ),
        }),
        json!({
            "name": "bench_forget",
            "description": "rusty bench: take a project off the recent list. The folder \
                is left alone.",
            "inputSchema": schema(json!({ "path": path }), &["path"]),
        }),
    ]
}

/// Handle a `bench_*` call against this user's bench; `None` for any other tool.
pub fn call(backend: &dyn EvalBackend, name: &str, args: &Value) -> Option<Result<Value, Fault>> {
    name.starts_with("bench_")
        .then(|| call_on(backend, &mut Bench::for_user(), name, args, unix_now()))
}

/// [`call`] against `bench`, at `now` (Unix seconds).
pub fn call_on(
    backend: &dyn EvalBackend,
    bench: &mut Bench,
    name: &str,
    args: &Value,
    now: u64,
) -> Result<Value, Fault> {
    let outcome = match name {
        "bench_list" => Ok(list(bench, now)),
        "bench_create" => {
            let parent = path_arg(args, "parent")?;
            let name = str_arg(args, "name")?;
            bench
                .create(&parent, name, now)
                .map(|root| json!({ "root": root, "next": "bench_open to drive it" }))
        }
        "bench_open" => open(backend, bench, &path_arg(args, "path")?, args, now),
        "bench_forget" => bench
            .forget(&path_arg(args, "path")?)
            .map(|removed| json!({ "removed": removed })),
        other => return Err((-32602, format!("Unknown tool: {other}"))),
    };
    Ok(tool_result(outcome))
}

fn list(bench: &Bench, now: u64) -> Value {
    let rows = bench.recent.projects.iter().map(|p| {
        json!({
            "name": p.name(),
            "path": p.path,
            "opened_at": p.opened_at,
            "ago": ago(p.opened_at, now),
            "missing": p.is_missing(),
        })
    });
    Value::Array(rows.collect())
}

fn open(
    backend: &dyn EvalBackend,
    bench: &mut Bench,
    dir: &Path,
    args: &Value,
    now: u64,
) -> Result<Value, String> {
    let confirmed = args.get("confirm").and_then(Value::as_bool) == Some(true);
    if let Some(warning) = check_existing(dir)? {
        if !confirmed {
            return Err(format!(
                "{warning} Call bench_open again with confirm: true to open it anyway."
            ));
        }
    }
    let (root, warning) = bench.open(dir, now)?;
    match backend.reopen(&root) {
        Some(Ok(())) => Ok(json!({ "root": root, "opened": true, "warning": warning })),
        Some(Err(err)) => Err(format!("could not open {}: {err}", root.display())),
        None => Ok(json!({
            "root": root,
            "opened": false,
            "warning": warning,
            "relaunch": format!("rusty --project {}", root.display()),
            "why": "attach mode drives a running window: relaunch it on the project",
        })),
    }
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, Fault> {
    let value = args.get(key).and_then(Value::as_str);
    value.ok_or_else(|| (-32602, format!("requires a string `{key}` argument")))
}

fn path_arg(args: &Value, key: &str) -> Result<PathBuf, Fault> {
    str_arg(args, key).map(PathBuf::from)
}

/// An MCP tool result: the value as JSON text, or the failure in-band.
fn tool_result(outcome: Result<Value, String>) -> Value {
    let (text, is_error) = match outcome {
        Ok(value) => (value.to_string(), false),
        Err(err) => (err, true),
    };
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

/// Now, in Unix seconds (`dev` is platform layer, so it may read the clock).
fn unix_now() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    now.map_or(0, |d| d.as_secs())
}

#[cfg(test)]
#[path = "bench_tests.rs"]
mod tests;
