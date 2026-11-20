//! crates/ecs/src/static_revision_tests.rs — activation moves `World::static_revision` only
//! when it can change the static caster set (#1496): the shadow sweeps skip inactive
//! entities, but a muzzle flash toggled every shot must not re-bake the static shadows.
//! A static move moves `World::static_geometry`'s revision instead (#2346).

use rusty_components::ColliderComponent;

use crate::World;

/// `root` with one child `wall`, linked both ways.
fn room(w: &mut World) -> (u32, u32) {
    let root = w.spawn("root".into());
    let wall = w.spawn("wall".into());
    w.set_parent_id(wall, Some(root));
    w.add_child(root, wall);
    (root, wall)
}

#[test]
fn deactivating_a_static_entity_or_its_ancestor_moves_the_revision() {
    let mut w = World::new();
    let (root, wall) = room(&mut w);
    w.set_static(wall, true);
    for id in [wall, root] {
        for active in [false, true] {
            let before = w.static_revision();
            w.set_active(id, active);
            assert_ne!(w.static_revision(), before, "{id} -> {active}");
        }
    }
    let before = w.static_revision();
    w.set_active(wall, true);
    assert_eq!(w.static_revision(), before, "no change, no bump");
}

#[test]
fn toggling_an_entity_with_no_static_below_leaves_the_revision() {
    let mut w = World::new();
    let (root, flash) = room(&mut w);
    w.set_static(root, true);
    let before = w.static_revision();
    for active in [false, true, false] {
        w.set_active(flash, active);
    }
    assert_eq!(w.static_revision(), before, "a static parent is not below");
}

/// A static entity's move moves `World::static_geometry`'s revision but not
/// `static_revision`, whose readers (the shadow sweeps, probe lighting) must not
/// re-run on a move; a dynamic one's moves and Collider edits move neither.
#[test]
fn a_static_move_moves_the_geometry_revision_only() {
    let mut w = World::new();
    let (root, wall) = room(&mut w);
    w.set_static(wall, true);
    let mut geometry = w.static_geometry();
    let statics = w.static_revision();
    w.transform_mut(root).unwrap().position.x += 1.0;
    assert!(geometry.changed(&w), "the wall moved with root");
    assert_eq!(w.static_revision(), statics, "and the static set did not");
    w.set_static(wall, false);
    assert!(
        !geometry.changed(&w),
        "a flag is the static set's, not a pose"
    );
    let statics = w.static_revision();
    w.transform_mut(root).unwrap().position.x += 1.0;
    let collider = ColliderComponent::from_mesh_bounds(false, glam::Vec3::ZERO, glam::Vec3::ONE);
    w.set_collider(wall, Some(collider));
    assert!(!geometry.changed(&w), "nothing static moved");
    assert_eq!(w.static_revision(), statics);
}

/// A watch opened on one World, asked of another whose revision is equal, reads
/// a change: the cursor's epoch is what tells the two Worlds apart.
#[test]
fn a_watch_asked_of_another_world_reads_a_change() {
    let (mut first, mut other) = (World::new(), World::new());
    let mut geometry = first.static_geometry();
    let _pose = other.static_geometry(); // `other` records `WorldPose` too
    for w in [&mut first, &mut other] {
        w.spawn("e".to_string());
    }
    assert!(geometry.changed(&other), "another World");
    assert!(!geometry.changed(&other), "then it is that World's");
}
