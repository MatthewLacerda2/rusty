//! `api-parity`: every inspector op is reachable from Lua (#1368).
//!
//! "Anything the editor does, the API does" (`docs/game-agent.md`). `--parity`
//! holds that the inspector writes through a shared `scene::authoring` op; this rule
//! holds the other half, field by field: every `authoring::<module>::set_*` op that
//! `src/editor/` names is also named from `src/api/`, either called by a named setter
//! or listed in `api::component`'s `op!`/`with!`/`group!` tables (a table entry names
//! the op as a path, which counts the same). An op the inspector alone may write (a
//! display toggle) says why at its `fn` with `// lint: allow(api-parity) <why>`, above
//! it or trailing it, and an allow on an op that is reached, or that no inspector
//! names, is itself a finding: the rule reads its own allows (`Allow::Own`), so a
//! stale one can't hide. An allow is matched to its op by where the op is defined,
//! so one in a re-exported sub-file (`particles/render.rs`) counts.
//!
//! A line scan, not a parser, with known limits:
//! - an op is matched by path: `use crate::scene::authoring::…` trees give each
//!   file's aliases (`rigidbody as rb`, `{self as ops, …}`, `nav::{agent, …}`,
//!   `rigidbody::{set_mass}`), and `alias::…::set_x`, a bare imported `set_x` (or its
//!   `as` name), or `crate::scene::authoring::…::set_x` names the op;
//! - a glob import (`use …::rigidbody::*`) and an op reached only through a helper
//!   in another module are not seen. On the API side that is a false finding; on
//!   the editor side the rule **fails open**: an op the inspector writes that way is
//!   never asked about;
//! - test code is not scanned.

use super::finding::{allowed, Finding};
use super::scan::Scan;
use crate::code::{code_lines, is_ident};
use std::collections::{BTreeMap, BTreeSet};

#[path = "api_parity_names.rs"]
mod names;
use names::join;
pub use names::{local_ops_named, ops_named};

pub const RULE: &str = "api-parity";

const AUTHORING: &str = "src/scene/authoring/";
const ROOT: &str = "crate::scene::authoring";
const EDITOR: &str = "src/editor/";
/// Where Lua reaches ops from: the bindings, and `Component.Set`'s field table
/// once it sits under `scene::authoring` (#1418 moves it there).
const API: &[&str] = &["src/api/", "src/scene/authoring/fields/"];
/// Modules whose own file holds the field-table row (and knob table) that reaches
/// its ops, so an op it calls by bare name is reached from Lua: the post effects,
/// each declaring its `Effect` beside its ops (#1467).
const ROWS: &[&str] = &["src/scene/authoring/rendering/post_processing/"];

/// An op by its path under `scene::authoring`: `nav::agent::set_speed`.
type Op = String;

pub fn run(scan: &Scan) -> Vec<Finding> {
    let named = |dirs: &[&str]| -> BTreeSet<Op> {
        let files = scan
            .files
            .iter()
            .filter(|p| dirs.iter().any(|d| p.starts_with(d)));
        files
            .flat_map(|p| ops_named(scan.source(p), scan.test_lines(p)))
            .collect()
    };
    let ops: BTreeMap<String, &str> = scan
        .files
        .iter()
        .filter(|p| p.starts_with(AUTHORING))
        .map(|p| (p.clone(), scan.source(p)))
        .collect();
    let mut api = named(API);
    for (path, source) in ops
        .iter()
        .filter(|(p, _)| ROWS.iter().any(|d| p.starts_with(d)))
    {
        api.extend(local_ops_named(
            source,
            scan.test_lines(path),
            &module_of(path),
        ));
    }
    check(&named(&[EDITOR]), &api, &ops)
}

/// The findings, given the ops the editor and the API name and the sources of
/// `src/scene/authoring/` by path.
pub fn check(
    editor: &BTreeSet<Op>,
    api: &BTreeSet<Op>,
    authoring: &BTreeMap<String, &str>,
) -> Vec<Finding> {
    let mut found = Vec::new();
    let finding = |path: String, line: usize, message: String| Finding {
        path,
        line,
        rule: RULE,
        message,
    };
    for op in editor.difference(api) {
        let fix = "add an `op!` to `api::component`'s table or a named setter, or say \
                   why it is editor-only (docs/linting.md)";
        let msg = |s: &str| {
            format!("the inspector writes through `authoring::{op}`{s} and no Lua binding reaches it: {fix}")
        };
        match definition(op, authoring) {
            Some((path, line)) if !allowed(authoring[&path], line, RULE) => {
                found.push(finding(path, line, msg("")));
            }
            Some(_) => {}
            None => found.push(finding(
                AUTHORING.into(),
                1,
                msg(" (its `fn` is not found under src/scene/authoring/)"),
            )),
        }
    }
    for (path, source) in authoring {
        for (line, at, name) in allows(source) {
            let here = Some((path.clone(), at));
            let op = editor
                .iter()
                .chain(api)
                .find(|op| definition(op, authoring) == here);
            let why = match op {
                _ if name.is_empty() => "it sits on no `set_*` fn",
                None => "no inspector writes through it",
                Some(op) if !editor.contains(op) => "no inspector writes through it",
                Some(op) if api.contains(op) => "Lua reaches it",
                Some(_) => continue,
            };
            let op = op.cloned().unwrap_or_else(|| join(&module_of(path), &name));
            let msg = format!("stale `lint: allow({RULE})` on `{op}`: {why}; remove it");
            found.push(finding(path.clone(), line, msg));
        }
    }
    found
}

/// `src/scene/authoring/nav/agent.rs` → `nav::agent`; a `mod.rs` is its folder.
fn module_of(path: &str) -> String {
    let rel = path.trim_start_matches(AUTHORING).trim_end_matches(".rs");
    let rel = rel.trim_end_matches("/mod").trim_end_matches("mod");
    rel.trim_end_matches('/').replace('/', "::")
}

/// Where `op`'s `fn` is defined: in its module's file, or a file of its folder
/// that the module re-exports.
fn definition(op: &str, authoring: &BTreeMap<String, &str>) -> Option<(String, usize)> {
    let (module, name) = op.rsplit_once("::").unwrap_or(("", op));
    let dir = format!("{AUTHORING}{}", module.replace("::", "/"));
    let dir = format!("{}/", dir.trim_end_matches('/'));
    let candidates = authoring.iter().filter(|(p, _)| {
        module_of(p) == module || p.strip_prefix(&dir).is_some_and(|f| !f.contains('/'))
    });
    let needle = format!("fn {name}(");
    candidates.into_iter().find_map(|(p, src)| {
        let at = src.lines().position(|l| l.contains(&needle))?;
        Some((p.clone(), at + 1))
    })
}

/// Each `lint: allow(api-parity)` in `source`: its line, the line of the item it
/// excuses (the next one below a comment, or its own when it trails), and that
/// item's `set_*` name (empty when it is no such fn).
fn allows(source: &str) -> Vec<(usize, usize, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let code = code_lines(source);
    let marker = format!("lint: allow({RULE})");
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let code = code.get(i).map_or("", |c| c.as_ref());
        if !l.contains(&marker) || code.contains(&marker) {
            continue;
        }
        let at = if code.trim().is_empty() {
            let below = lines[i..].iter().position(|l| {
                let t = l.trim_start();
                !t.starts_with("//") && !t.starts_with("#[")
            });
            below.map_or(lines.len(), |n| i + n)
        } else {
            i
        };
        let name = lines.get(at).and_then(|l| {
            let from = l.find("fn set_")? + 3;
            Some(l[from..].chars().take_while(|c| is_ident(*c)).collect())
        });
        out.push((i + 1, at + 1, name.unwrap_or_default()));
    }
    out
}

#[cfg(test)]
#[path = "api_parity_tests.rs"]
mod tests;
