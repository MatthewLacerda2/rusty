//! API-doc drift gate (#280): `docs/scripting-api.md` must agree with the **live Lua
//! API surface** about *what exists*. The doc is served to the agent over MCP (#288),
//! so a drifted doc lies to it. This enforces existence-parity both directions — every
//! documented `Namespace.Function` is a registered binding, and every registered
//! binding is documented. Signatures are out of scope (Lua closures are opaque at
//! runtime; the deferred macro would be needed for types). Gated on `dev` (a `Session`
//! is dev-only); the session boots an **empty** scene so only the `api::` namespaces
//! populate `_G` (no entity scripts).
#![cfg(feature = "dev")]

use std::collections::{BTreeMap, BTreeSet};

use rusty::dev::session::Session;

/// The committed scripting-API doc, embedded at build time so the test reads exactly
/// what ships in the repo (not a path resolved at runtime).
const DOC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/scripting-api.md"
));

/// Live namespaces deliberately *not* in the user-facing scripting doc. Empty by
/// design: every live namespace (including dev-only `Debug`) is documented, so any
/// addition here must carry a justification comment.
const NAMESPACE_ALLOWLIST: &[&str] = &[];

type Surface = BTreeMap<String, BTreeSet<String>>;

#[test]
fn doc_matches_live_lua_surface() {
    let sess = Session::new("").expect("session boots in edit mode");
    let live = sess.world().script_manager().api_surface();
    assert!(!live.is_empty(), "the live API surface must not be empty");
    let doc = parse_documented_surface(DOC);

    // Every namespace is a key in both maps (a documented namespace with no rows still
    // gets an empty entry), so one walk over the union covers names and functions.
    let mut problems = Vec::new();
    let names: BTreeSet<&String> = live.keys().chain(doc.keys()).collect();
    for ns in names {
        if NAMESPACE_ALLOWLIST.contains(&ns.as_str()) {
            continue;
        }
        match (live.get(ns), doc.get(ns)) {
            (Some(_), None) => {
                problems.push(format!("namespace registered but not documented: `{ns}`"))
            }
            (None, Some(_)) => {
                problems.push(format!("namespace documented but not registered: `{ns}`"))
            }
            (Some(l), Some(d)) => diff_fns(ns, l, d, &mut problems),
            (None, None) => unreachable!("ns came from one of the two maps"),
        }
    }

    assert!(
        problems.is_empty(),
        "scripting-api.md has drifted from the live Lua surface — reconcile the doc \
         (reality is the source of truth for existence):\n  {}",
        problems.join("\n  ")
    );
}

/// Push one line per drifting function in `ns`, in both directions.
fn diff_fns(ns: &str, live: &BTreeSet<String>, doc: &BTreeSet<String>, out: &mut Vec<String>) {
    let doc_only: Vec<&String> = doc.difference(live).collect();
    let live_only: Vec<&String> = live.difference(doc).collect();
    if !doc_only.is_empty() {
        out.push(format!(
            "[{ns}] documented but not registered: {doc_only:?}"
        ));
    }
    if !live_only.is_empty() {
        out.push(format!(
            "[{ns}] registered but not documented: {live_only:?}"
        ));
    }
}

/// Parse the doc into `namespace → {function names}`. A **namespace** is an `## `
/// heading beginning with a backticked identifier (trailing prose ignored). A
/// **function** is a table row's first cell beginning with a backticked dotted
/// identifier `` `Namespace.Function` `` — optionally ` / `SecondName`` for a combined
/// getter/setter row, or a trailing annotation like `*(dev-only)*`. Fences are skipped,
/// so a `Material.DefineAsset(...)` inside one never counts.
fn parse_documented_surface(doc: &str) -> Surface {
    let mut surface: Surface = BTreeMap::new();
    let mut in_fence = false;
    for line in doc.lines() {
        let s = line.trim();
        if s.starts_with("```") {
            in_fence = !in_fence;
        } else if in_fence {
            continue;
        } else if let Some(rest) = s.strip_prefix("## ") {
            if let Some(ns) = backticked_ident(rest.trim()) {
                surface.entry(ns).or_default();
            }
        } else if s.starts_with('|') {
            if let Some(first) = s.trim_matches('|').split('|').next() {
                add_table_fn(first.trim(), &mut surface);
            }
        }
    }
    surface
}

/// Record the documented function(s) named by a table row's first `cell`, if it
/// begins with a backticked dotted identifier (plus an optional `/ `Second`` form).
fn add_table_fn(cell: &str, surface: &mut Surface) {
    let Some(body) = cell.strip_prefix('`') else {
        return;
    };
    let Some(end) = body.find('`') else { return };
    let Some((ns, first)) = body[..end].split_once('.') else {
        return;
    };
    if !is_ident(ns) || !is_ident(first) {
        return;
    }
    let entry = surface.entry(ns.to_string()).or_default();
    entry.insert(first.to_string());
    // Combined getter/setter row: `` `Ns.GetX` / `SetX` ``.
    if let Some(after) = cell[end + 2..].trim_start().strip_prefix("/ ") {
        if let Some(second) = backticked_ident(after.trim_start()) {
            entry.insert(second);
        }
    }
}

/// If `s` starts with a backticked identifier, return it (ignores trailing text).
fn backticked_ident(s: &str) -> Option<String> {
    let body = s.strip_prefix('`')?;
    let end = body.find('`')?;
    let ident = &body[..end];
    is_ident(ident).then(|| ident.to_string())
}

/// A Lua identifier: a non-empty `[A-Za-z0-9_]` run starting with a letter or `_`.
fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
