//! A reference cleared by a despawn comes back with an Undo, or with Stop's
//! snapshot restore, and the next despawn clears it again (#1708): the World's
//! reverse index reads the restored holder, not what it held before.

use crate::components::JointComponent;
use crate::scene::snapshot::SceneSnapshot;
use crate::scene::Scene;

/// A body and a hinge whose joint names it.
fn hinged() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let body = scene.add_entity("body".into());
    let hinge = scene.add_entity("hinge".into());
    let joint = JointComponent {
        connected_body: Some(body),
        ..JointComponent::default()
    };
    scene.world.set_joint(hinge, Some(joint));
    (scene, body, hinge)
}

fn connected(scene: &Scene, hinge: u32) -> Option<u32> {
    scene.world.joint(hinge).expect("kept").connected_body
}

#[test]
fn an_undone_destroy_brings_the_reference_back_for_the_next_one_to_clear() {
    let (mut scene, body, hinge) = hinged();
    for _ in 0..2 {
        scene.begin_edits();
        scene.destroy_entity(body);
        scene.commit_edits(None);
        assert_eq!(connected(&scene, hinge), None);
        scene.undo();
        assert_eq!(connected(&scene, hinge), Some(body), "undo restores it");
    }
}

#[test]
fn stop_brings_the_reference_back_for_the_next_destroy_to_clear() {
    let (mut scene, body, hinge) = hinged();
    let snapshot = SceneSnapshot::capture(&scene); // Play
    scene.destroy_entity(body);
    assert_eq!(connected(&scene, hinge), None);
    snapshot.restore(&mut scene); // Stop
    assert_eq!(connected(&scene, hinge), Some(body));
    scene.destroy_entity(body);
    assert_eq!(
        connected(&scene, hinge),
        None,
        "the restored World is indexed"
    );
}
