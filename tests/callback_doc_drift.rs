//! The "Script lifecycle callbacks" section of `docs/api/index.md` must match
//! `scripting::callbacks::LIFECYCLE_CALLBACKS`, the list dispatch and discovery read. Existence
//! parity both directions; not `dev`-gated.

use std::collections::BTreeSet;

use rusty::script::scripting::callbacks::LIFECYCLE_CALLBACKS;

fn reference() -> String {
    rusty::script::scripting::api_doc::api_reference().expect("docs/api/ reads")
}

const HEADING: &str = "## Script lifecycle callbacks";

#[test]
fn doc_matches_dispatched_callbacks() {
    let text = reference();
    let doc = parse_documented_callbacks(&text);
    let live: BTreeSet<&str> = LIFECYCLE_CALLBACKS.iter().copied().collect();
    assert!(
        !doc.is_empty(),
        "no callback rows found under `{HEADING}` in docs/api/index.md"
    );

    let doc_only: Vec<&&str> = doc.difference(&live).collect();
    let live_only: Vec<&&str> = live.difference(&doc).collect();
    assert!(
        doc_only.is_empty() && live_only.is_empty(),
        "docs/api/index.md has drifted from the dispatched lifecycle callbacks \
         (crates/script/src/scripting/callbacks.rs is the source of truth for existence):\n  \
         documented but never dispatched: {doc_only:?}\n  \
         dispatched but not documented: {live_only:?}"
    );
}

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

fn backticked_ident(cell: &str) -> Option<&str> {
    let body = cell.strip_prefix('`')?.strip_suffix('`')?;
    let mut chars = body.chars();
    let ident = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    ident.then_some(body)
}
