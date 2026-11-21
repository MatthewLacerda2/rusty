//! What a document writes back: the keys a round trip adds, held to none.
//!
//! The address of a bake hashes the recipe's **bytes**, and tools re-serialise
//! a recipe whenever they write one. So an optional field that writes its
//! default into every document changes every recipe's bytes, and every cached
//! bake misses, without a sample moving (`crates/zimmer/CLAUDE.md`). Each such
//! field takes `skip_serializing_if`, and [`drift`] holds every one of them
//! at once — including the next one, which is the point.
//!
//! Shared by path with `crates/providers/tests/suite/documented`, which holds the
//! worked recipes in `docs/recipes.md` to the same, so the list of fields that
//! predate the rule is written once.

use std::collections::BTreeSet;

use serde_json::Value;

/// The fields that wrote their default before the rule existed, each as the
/// tail of a key path (`[]` for any array entry, a leading `.` for the root).
/// A match excuses the key and everything under it.
///
/// Each is a known exception, not a precedent: skipping one now would change
/// the bytes of every recipe a tool rewrites, and so every bake's address,
/// which is the user's call (#1363). A new field never joins this
/// list — it takes `skip_serializing_if` instead.
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
    // A one-shot recipe's own field, `providers`' `OneShot`.
    ".velocity",
];

/// Every key the round trip from `written` to `rewritten` added or dropped,
/// beyond [`PREDATES_THE_RULE`], as a failure message; `what` names the
/// document. `None` when the shape held.
///
/// `root` names what the document's top level is, so a bare patch's fields
/// read as `patch.…` the way they do inside a song or a recipe.
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

/// Whether `path`, or a key above it, ends in one of [`PREDATES_THE_RULE`].
fn excused(path: &str) -> bool {
    let segments: Vec<&str> = path.split('.').collect();
    (1..=segments.len()).any(|end| {
        PREDATES_THE_RULE
            .iter()
            .any(|tail| segments[..end].ends_with(&tail.split('.').collect::<Vec<_>>()))
    })
}

/// Every key in a JSON document, as a path from `root`, array indices erased.
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
