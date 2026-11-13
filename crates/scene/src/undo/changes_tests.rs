//! An Undo or Redo step rewinds the change log once, whatever it restores (#1705):
//! each rewind resets every recording log, so one per restored entity made undoing
//! a Dust2-sized move a hitch.

use glam::Vec3;

use crate::Scene;
use rusty_components::TransformComponent;
use rusty_ecs::{Changes, WorldPose};

#[test]
fn undoing_a_three_thousand_entity_move_takes_one_rewind() {
    let mut scene = Scene::new();
    let ids: Vec<u32> = (0..3000)
        .map(|i| scene.world.spawn(format!("e{i}")))
        .collect();
    let mut local = scene.world.change_cursor::<TransformComponent>();
    let mut poses = scene.world.change_cursor::<WorldPose>();
    scene.begin_edits();
    for &id in &ids {
        scene.world.transform_mut(id).expect("live").position = Vec3::X;
    }
    scene.commit_edits(Some("Move all"));
    let _ = (
        scene.world.changes(&mut local),
        scene.world.changes(&mut poses),
    );

    let before = scene.world.change_rewinds();
    assert_eq!(scene.undo().as_deref(), Some("Move all"));
    assert_eq!(scene.world.change_rewinds(), before + 1, "one per step");
    assert_eq!(scene.world.changes(&mut local), Changes::Rebuild);
    assert_eq!(scene.world.changes(&mut poses), Changes::Rebuild);

    assert_eq!(scene.redo().as_deref(), Some("Move all"));
    assert_eq!(scene.world.change_rewinds(), before + 2);
    assert_eq!(
        scene.world.changes(&mut local),
        Changes::Rebuild,
        "a Redo too"
    );
}
