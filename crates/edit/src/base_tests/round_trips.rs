//! Base state through a document or a Play snapshot: a scene's identity
//! (`rusty_scene::identity`) and the `Debug.Draw*` lines (`rusty_scene::runtime::debug_lines`).

use glam::Vec3;

use crate::serialize::{apply_scene_data, to_scene_data};
use crate::SceneSnapshot;
use rusty_scene::Scene;

const WHITE: [f32; 4] = [1.0; 4];

/// Opening another scene in edit mode, where no tick ages them, drops the lines
/// drawn over the old one.
#[test]
fn applying_a_document_drops_the_lines() {
    let mut scene = Scene::new();
    scene.debug_lines.push(Vec3::ZERO, Vec3::X, WHITE, 7);
    let data = to_scene_data(&Scene::new());
    apply_scene_data(&mut scene, data);
    assert!(scene.debug_lines.is_empty());
}

#[test]
fn an_id_survives_the_play_mode_round_trip() {
    // Play snapshots to `SceneData` and applies it back to the SAME `Scene`, so
    // the identity — and therefore every cached GPU resource keyed by it — must
    // come through unchanged.
    let mut scene = Scene::new();
    let before = scene.id();
    let snapshot = SceneSnapshot::capture(&scene);
    snapshot.restore(&mut scene);
    assert_eq!(scene.id(), before);
}
