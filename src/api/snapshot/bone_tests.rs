//! The snapshot leaves a rig's bones (#453) out unless asked.

use glam::{Vec2, Vec3};

use crate::scene::skeleton::rig::armature_owner;
use crate::scene::{Camera, Scene};

#[test]
fn the_snapshot_leaves_bones_out_unless_asked() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let read = |bones| {
        let v = super::world_value_with(
            &scene,
            &Camera::new(Vec3::ZERO, 0.0, 0.0),
            0,
            false,
            &crate::ui::UiView::screen(Vec2::new(800.0, 600.0), Default::default()),
            bones,
        );
        v["entities"].as_array().unwrap().clone()
    };
    assert_eq!(read(false).len(), 1);
    let all = read(true);
    assert_eq!(all.len(), 4);
    assert_eq!(all.iter().filter(|e| e["bone"] == true).count(), 3);
}
