//! src/ecs/despawn/cost_tests.rs — a despawn pays for what it kills, not for the
//! level (#1708): once the reverse index is open, killing a leaf nothing names
//! re-reads the same few holders in a level of ten meshes and of two thousand.

use std::collections::BTreeSet;

use crate::components::{JointComponent, MeshComponent};
use crate::ecs::World;

/// `meshes` static meshes, a body with a joint naming it, and `leaves` projectiles.
fn level(meshes: usize, leaves: usize) -> (World, u32, Vec<u32>) {
    let mut w = World::new();
    for _ in 0..meshes {
        let id = w.spawn("wall".into());
        w.set_mesh(id, Some(MeshComponent::new("Box", Vec::new(), Vec::new())));
        w.set_static(id, true);
    }
    let body = w.spawn("body".into());
    let hinge = w.spawn("hinge".into());
    let joint = JointComponent {
        connected_body: Some(body),
        ..JointComponent::default()
    };
    w.set_joint(hinge, Some(joint));
    let leaves = (0..leaves).map(|_| w.spawn("shot".into())).collect();
    (w, hinge, leaves)
}

/// Holders the index touched while despawning the second of two leaves.
fn second_leaf(meshes: usize) -> u64 {
    let (mut w, _, leaves) = level(meshes, 2);
    w.despawn(leaves[0]); // opens the index: one walk over every holder
    let before = w.refs.as_ref().expect("opened").touched;
    w.despawn(leaves[1]);
    w.refs.as_ref().expect("opened").touched - before
}

#[test]
fn a_leaf_s_death_touches_the_same_holders_at_any_level_size() {
    let small = second_leaf(10);
    assert_eq!(small, second_leaf(2000), "O(1) in the level's meshes");
    assert!(
        small <= 2,
        "only the first leaf's own removal is re-read: {small}"
    );
}

#[test]
fn a_named_target_s_death_touches_its_holders() {
    let (mut w, hinge, leaves) = level(2000, 1);
    w.despawn(leaves[0]);
    let before = w.refs.as_ref().expect("opened").touched;
    let body = w.find_by_name("body").expect("live");
    w.despawn_all(&BTreeSet::from([body]));
    assert_eq!(w.joint(hinge).expect("kept").connected_body, None);
    let touched = w.refs.as_ref().expect("opened").touched - before;
    assert!(touched <= 2, "the leaf's removal and the hinge: {touched}");
}
