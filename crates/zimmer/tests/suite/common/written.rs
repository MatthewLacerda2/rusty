//! What a document writes back: the keys a round trip adds, held to none.

use std::collections::BTreeSet;

use serde_json::Value;

/// Each entry is a key path's tail (`[]` any array entry, a leading `.` the root); a match excuses
/// that key and every key under it.
const PREDATES_THE_RULE: &[&str] = &[
    "patch.filter",
    "patch.lfo",
    "patch.fx",
    "filter.adsr",
    "filter.env_octaves",
    "filter.resonance",
    "filter.vel_octaves",
    "oscs[].detune_cents",
    "oscs[].gain",
    "oscs[].octave",
    "operators[].feedback",
    "partials[].decay",
    "partials[].detune_cents",
    "fx[].mix",
    "fx[].makeup",
    "bands[].q",
    "tracks[].gain",
    "notes[].vel",
    ".seed",
    ".velocity",
];

pub(crate) fn drift(what: &str, root: &str, written: &str, rewritten: &str) -> Option<String> {
    let before = keys(root, written);
    let after = keys(root, rewritten);
    let added: Vec<_> = after
        .difference(&before)
        .filter(|path| !excused(path))
        .collect();
    let dropped: Vec<_> = before.difference(&after).collect();
    (!added.is_empty() || !dropped.is_empty()).then(|| {
        format!(
            "{what} changed shape on the way back out — added {added:?}, dropped \
         {dropped:?}. An optional field that writes its default changes every \
         recipe's bytes and so every bake's address: give it \
         `skip_serializing_if` at its default (crates/zimmer/CLAUDE.md)."
        )
    })
}

fn excused(path: &str) -> bool {
    let segments: Vec<&str> = path.split('.').collect();
    (1..=segments.len()).any(|end| {
        PREDATES_THE_RULE
            .iter()
            .any(|tail| segments[..end].ends_with(&tail.split('.').collect::<Vec<_>>()))
    })
}

fn keys(root: &str, json: &str) -> BTreeSet<String> {
    let value: Value = serde_json::from_str(json).expect("the document is JSON");
    let mut found = BTreeSet::new();
    walk(&value, root.to_owned(), &mut found);
    found
}

fn walk(value: &Value, at: String, found: &mut BTreeSet<String>) {
    match value {
        Value::Object(fields) => {
            for (key, inner) in fields {
                let path = format!("{at}.{key}");
                found.insert(path.clone());
                walk(inner, path, found);
            }
        }
        Value::Array(items) => {
            for inner in items {
                walk(inner, format!("{at}[]"), found);
            }
        }
        _ => {}
    }
}
