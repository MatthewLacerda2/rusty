//! Callback-doc drift gate (#309): the "Script lifecycle callbacks" section of
//! `docs/scripting-api.md` must agree with the callbacks the engine actually
//! dispatches — the callback sibling of the #280 namespace gate
//! (`tests/api_doc_drift.rs`). The ground truth is
//! `scripting::callbacks::LIFECYCLE_CALLBACKS`, the one list dispatch and
//! MonoBehaviour discovery both read, so a callback added (or removed) there
//! cannot merge undocumented — and the doc cannot advertise a callback that
//! silently never fires (the README once did). Existence parity both directions;
//! signatures are out of scope, as in the namespace gate. Not `dev`-gated:
//! callbacks are plain consts, so the gate runs under both CI feature sets.

use std::collections::BTreeSet;

use rusty::scripting::callbacks::LIFECYCLE_CALLBACKS;

/// The committed scripting-API doc, embedded at build time so the test reads
/// exactly what ships in the repo (not a path resolved at runtime).
const DOC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/scripting-api.md"
));

/// The heading the callback table lives under. Deliberately not backticked, so
/// the namespace gate's parser never mistakes the section for a namespace.
const HEADING: &str = "## Script lifecycle callbacks";

#[test]
fn doc_matches_dispatched_callbacks() {
    let doc = parse_documented_callbacks(DOC);
    let live: BTreeSet<&str> = LIFECYCLE_CALLBACKS.iter().copied().collect();
    assert!(
        !doc.is_empty(),
        "no callback rows found under `{HEADING}` in docs/scripting-api.md"
    );

    let doc_only: Vec<&&str> = doc.difference(&live).collect();
    let live_only: Vec<&&str> = live.difference(&doc).collect();
    assert!(
        doc_only.is_empty() && live_only.is_empty(),
        "scripting-api.md has drifted from the dispatched lifecycle callbacks \
         (src/scripting/callbacks.rs is the source of truth for existence):\n  \
         documented but never dispatched: {doc_only:?}\n  \
         dispatched but not documented: {live_only:?}"
    );
}

/// Parse the callback names documented under `HEADING`: each table row whose
/// first cell is a backticked bare identifier (`` `Start` ``), up to the next
/// `## ` heading. Mirrors the row convention of `tests/api_doc_drift.rs`, minus
/// the namespace dot.
fn parse_documented_callbacks(doc: &str) -> BTreeSet<&str> {
    let mut set = BTreeSet::new();
    let mut in_section = false;
    for line in doc.lines() {
        let s = line.trim();
        if s.starts_with("## ") {
            in_section = s == HEADING;
        } else if in_section && s.starts_with('|') {
            if let Some(first) = s.trim_matches('|').split('|').next() {
                if let Some(name) = backticked_ident(first.trim()) {
                    set.insert(name);
                }
            }
        }
    }
    set
}

/// If `cell` is exactly a backticked Lua identifier, return it.
fn backticked_ident(cell: &str) -> Option<&str> {
    let body = cell.strip_prefix('`')?.strip_suffix('`')?;
    let mut chars = body.chars();
    let ident = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    ident.then_some(body)
}
