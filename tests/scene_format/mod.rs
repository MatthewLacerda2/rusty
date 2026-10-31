//! Pins the scene file format (#941): a change to what a `.scene` or `.prefab` file
//! holds must update `scene.fingerprint` beside this file, so it is a decision
//! someone saw, not a side effect.
//!
//! The fingerprint is the read side of `SceneData`, walked by a serde tracer
//! (`trace.rs`): one line per place a file can hold a value, as
//! `path: kind`. Paths go `.field`, `[]` for a list element, `{key}`/`{}` for a map
//! key and value, `<Variant>` for an enum variant's contents; enum lines list every
//! tag. Every enum variant is walked, so a renamed field, a retyped value, an added
//! or removed variant all change it. The blind spots are serde's: an untagged
//! enum, a `serde_json::Value` or a type that parses one itself reads as `any`, and a
//! `#[serde(flatten)]` struct's own fields are the list in `hidden.rs`.

mod access;
mod de;
mod hidden;
mod trace;

use std::collections::BTreeSet;

use rusty::components::{AnimatorComponent, Playback};
use rusty::scene::SceneData;

const FINGERPRINT: &str = "tests/scene_format/scene.fingerprint";

fn fingerprint() -> String {
    let (lines, _) = trace::trace::<SceneData>("scene");
    lines.into_iter().map(|l| l + "\n").collect()
}

#[test]
fn scene_format_matches_its_fingerprint() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(rusty::core::paths::native(FINGERPRINT));
    let now = fingerprint();
    if std::env::var_os("RUSTY_BLESS_SCENE_FORMAT").is_some() {
        std::fs::write(&path, &now).expect("write the fingerprint");
        return;
    }
    let pinned = std::fs::read_to_string(&path).unwrap_or_default();
    if pinned == now {
        return;
    }
    let (old, new): (BTreeSet<_>, BTreeSet<_>) = (pinned.lines().collect(), now.lines().collect());
    let removed: Vec<_> = old.difference(&new).map(|l| format!("  - {l}")).collect();
    let added: Vec<_> = new.difference(&old).map(|l| format!("  + {l}")).collect();
    panic!(
        "the scene file format changed:\n{}\n{}\n\n\
         This changes what saved scenes and prefabs hold. No migration is owed \
         (CLAUDE.md, *Backward compatibility is not a concern*): update the \
         engine's fixtures and the default scene, then update {FINGERPRINT} with\n  \
         RUSTY_BLESS_SCENE_FORMAT=1 cargo nextest run -E 'test(scene_format)'\n\
         and say in the PR what changed.",
        removed.join("\n"),
        added.join("\n"),
    );
}

/// `hidden.rs` must list every field of a flattened struct: a field it misses is a
/// part of the format the fingerprint would not see. Whatever the built value
/// writes must be either a listed field or the flattened part's.
#[test]
fn scene_format_flatten_list_is_complete() {
    fn check<T: serde::de::DeserializeOwned + serde::Serialize>(
        name: &str,
        flat: serde_json::Value,
    ) {
        let (own, _) = hidden::registry(name).expect("listed");
        let (_, built) = trace::trace::<T>("x");
        let serde_json::Value::Object(written) = serde_json::to_value(built).unwrap() else {
            panic!("{name} writes an object");
        };
        let serde_json::Value::Object(flat) = flat else {
            panic!("flattened part is an object")
        };
        for key in written.keys() {
            assert!(
                own.contains(&key.as_str()) || flat.contains_key(key),
                "{name}.{key} is not listed in tests/scene_format/hidden.rs"
            );
        }
    }
    let playback = serde_json::to_value(trace::trace::<Playback>("x").1).unwrap();
    check::<AnimatorComponent>("AnimatorComponent", playback.clone());
    check::<rusty::components::LayerState>("LayerState", playback);
}
