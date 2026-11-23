//! The hierarchy's *Save as Prefab* says in the console what the click did not do
//! and which prefab it replaced.

use super::tree::save_as_prefab;
use rusty_scene::Scene;
use rusty_test_support::{logs, temp};

fn one_entity() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Crate".into());
    (scene, id)
}

#[test]
fn a_prefab_that_cannot_be_written_is_said_in_the_console() {
    let (scene, id) = one_entity();
    let file = temp::dir().join("a_file");
    std::fs::write(&file, "").unwrap();
    // A folder under a file cannot be made.
    let dir = file.join("prefabs");
    let said = logs::during(|| save_as_prefab(&dir, &scene, id, "Crate"));
    assert!(
        matches!(said.as_slice(), [line] if line.contains("Save as Prefab failed")),
        "{said:?}"
    );
    assert!(!dir.exists());
}

#[test]
fn saving_over_a_prefab_names_the_file_it_replaced() {
    let (scene, id) = one_entity();
    let dir = temp::dir().join("prefabs");
    let first = logs::during(|| save_as_prefab(&dir, &scene, id, "Crate"));
    assert!(first.is_empty(), "{first:?}");
    let path = dir.join(format!("Crate.{}", rusty_edit::PREFAB_EXTENSION));
    assert!(path.exists());
    let again = logs::during(|| save_as_prefab(&dir, &scene, id, "Crate"));
    let named = format!("replaced {}", path.display());
    assert!(
        matches!(again.as_slice(), [line] if line.contains(&named)),
        "{again:?}"
    );
}
