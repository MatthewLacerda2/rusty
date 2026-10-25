//! A prefab file is held once, when it is read (#1418), as a scene file is, and
//! an in-range one reads exactly as written.

use super::tests::dressed;
use crate::scene::{load_and_instantiate, read_prefab_file, save_prefab, PrefabData, Scene};

/// `dressed()` with `edit` applied, saved as a prefab under `tag`; its path.
fn saved(tag: &str, edit: impl FnOnce(&mut Scene, u32)) -> String {
    let (mut scene, id) = dressed();
    edit(&mut scene, id);
    let path = crate::test_temp::dir().join(format!("rusty_1418_{tag}.prefab"));
    let path = path.to_string_lossy().into_owned();
    save_prefab(&scene, id, &path).unwrap();
    path
}

#[test]
fn a_prefab_is_held_when_read() {
    let path = saved("held", |scene, id| {
        scene.world.audio_mut(id).unwrap().volume = 4.0;
    });
    let prefab = read_prefab_file(&path).unwrap();
    assert_eq!(prefab.entities[0].audio.as_ref().unwrap().volume, 1.0);
    let mut scene = Scene::new();
    let root = load_and_instantiate(&mut scene, &path, None).unwrap();
    assert_eq!(scene.world.audio(root).unwrap().volume, 1.0);
}

/// The hold goes through a scratch World; what it has no column for (a bone
/// attachment's `parent_bone`) comes back as written.
#[test]
fn an_in_range_prefab_reads_as_written() {
    let path = saved("as_written", |_, _| {});
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    doc["entities"][0]["parent_bone"] = serde_json::json!(["hand_r", "forearm_r"]);
    std::fs::write(&path, doc.to_string()).unwrap();
    let raw: PrefabData = serde_json::from_value(doc).unwrap();
    let read = read_prefab_file(&path).unwrap();
    assert_eq!(
        serde_json::to_value(read).unwrap(),
        serde_json::to_value(raw).unwrap()
    );
}
