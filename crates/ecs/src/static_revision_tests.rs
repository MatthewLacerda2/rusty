//! crates/ecs/src/static_revision_tests.rs — activation moves `World::static_revision` only
//! when it can change the static caster set (#1496): the shadow sweeps skip inactive
//! entities, but a muzzle flash toggled every shot must not re-bake the static shadows.
//! A static move moves `World::static_geometry_revision` instead (#2346).

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

/// A static entity's move or Collider edit moves `static_geometry_revision` but not
/// `static_revision`, whose readers (the shadow sweeps, probe lighting) must not
/// re-run on a move; a dynamic one's moves neither.
#[test]
fn a_static_move_moves_the_geometry_revision_only() {
    let mut w = World::new();
    let (root, wall) = room(&mut w);
    w.set_static(wall, true);
    let _pose = w.change_cursor::<crate::WorldPose>(); // the walk runs while one is open
    let revisions = |w: &World| (w.static_revision(), w.static_geometry_revision());
    let (statics, geometry) = revisions(&w);
    w.transform_mut(root).unwrap().position.x += 1.0;
    assert_eq!(
        revisions(&w),
        (statics, geometry + 1),
        "the wall moved with root"
    );
    w.set_static(wall, false);
    let before = revisions(&w);
    w.transform_mut(root).unwrap().position.x += 1.0;
    let collider = ColliderComponent::from_mesh_bounds(false, glam::Vec3::ZERO, glam::Vec3::ONE);
    w.set_collider(wall, Some(collider));
    assert_eq!(revisions(&w), before, "nothing static moved");
}
