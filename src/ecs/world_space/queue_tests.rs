//! The store's queue: a despawn re-roots its children where they stand (#1264),
//! and writes made parents first, as the Animator makes them, are all seen by the
//! next read (#1170).

use glam::{Mat4, Vec3};

use crate::components::{ColliderComponent, ColliderShape};
use crate::ecs::World;

fn child_of(w: &mut World, parent: u32) -> u32 {
    let c = w.spawn("child".into());
    w.add_child(parent, c);
    w.set_parent_id(c, Some(parent));
    c
}

#[test]
fn despawning_a_parent_leaves_its_children_as_roots_at_their_world_pose() {
    let mut w = World::new();
    let p = w.spawn("p".into());
    w.transform_mut(p).unwrap().position = Vec3::new(0.0, 5.0, 0.0);
    let c = child_of(&mut w, p);
    w.transform_mut(c).unwrap().position = Vec3::X;
    let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    col.shape = ColliderShape::Box { size: Vec3::ONE };
    w.set_collider(c, Some(col));
    let other = w.spawn("other".into());
    w.transform_mut(other).unwrap().position = Vec3::Z;
    assert_eq!(w.world_position(c), Vec3::new(1.0, 5.0, 0.0));
    assert_eq!(w.world_position(other), Vec3::Z);
    w.despawn(p);
    assert_eq!(w.parent_id(c), None, "a root, not a child of a dead id");
    assert_eq!(w.transform(c).unwrap().position, Vec3::new(1.0, 5.0, 0.0));
    assert_eq!(w.world_position(c), Vec3::new(1.0, 5.0, 0.0));
    assert_eq!(w.local_to_world(p), Mat4::IDENTITY);
    assert_eq!(
        w.collider_bounds(c).unwrap(),
        (Vec3::new(0.5, 4.5, -0.5), Vec3::new(1.5, 5.5, 0.5))
    );
    w.flush_bounds();
    let c_box = w.collider(c).unwrap();
    assert_eq!(
        (c_box.aabb_min, c_box.aabb_max),
        w.collider_bounds(c).unwrap()
    );
}

#[test]
fn a_write_under_a_written_parent_is_seen_by_the_next_read() {
    // Parents first, as the Animator writes: the second write queues nothing new.
    let mut w = World::new();
    let p = w.spawn("p".into());
    let c = child_of(&mut w, p);
    let g = child_of(&mut w, c);
    assert_eq!(w.world_position(g), Vec3::ZERO);
    w.transform_mut(p).unwrap().position = Vec3::Y;
    w.transform_mut(c).unwrap().position = Vec3::X;
    w.transform_mut(g).unwrap().position = Vec3::Z;
    assert_eq!(w.world_position(g), Vec3::ONE);
    w.transform_mut(c).unwrap().position = Vec3::ZERO;
    assert_eq!(w.world_position(g), Vec3::new(0.0, 1.0, 1.0));
}
