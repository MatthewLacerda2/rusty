//! The world-space API and its store (#1170).

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};

use crate::components::{ColliderComponent, ColliderShape};
use crate::ecs::World;

const EPS: f32 = 1e-5;

fn child_of(w: &mut World, parent: u32) -> u32 {
    let c = w.spawn("child".into());
    w.add_child(parent, c);
    w.set_parent_id(c, Some(parent));
    c
}

fn with_box(w: &mut World, id: u32, size: Vec3) {
    let mut c = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    c.shape = ColliderShape::Box { size };
    w.set_collider(id, Some(c));
}

fn aabb(w: &World, id: u32) -> (Vec3, Vec3) {
    w.collider_bounds(id).unwrap()
}

#[test]
fn a_turned_child_reads_its_parents_scale_along_its_own_axes() {
    // #1156: parent (1,1,3), child turned 90° about Y: its X lies on world Z.
    let mut w = World::new();
    let p = w.spawn("p".into());
    w.transform_mut(p).unwrap().scale = Vec3::new(1.0, 1.0, 3.0);
    let c = child_of(&mut w, p);
    w.transform_mut(c).unwrap().rotation = Quat::from_rotation_y(FRAC_PI_2);
    assert!(w.lossy_scale(c).abs_diff_eq(Vec3::new(3.0, 1.0, 1.0), EPS));
    with_box(&mut w, c, Vec3::ONE);
    let (min, max) = aabb(&w, c);
    assert!(max.abs_diff_eq(Vec3::new(0.5, 0.5, 1.5), EPS), "{max}");
    assert!(min.abs_diff_eq(-Vec3::new(0.5, 0.5, 1.5), EPS), "{min}");
}

#[test]
fn a_direction_lands_where_the_entity_draws_it_under_its_own_mirror() {
    // #1145: an entity's own (1,1,-1) scale draws its local +Z at world -Z.
    let mut w = World::new();
    let e = w.spawn("e".into());
    w.transform_mut(e).unwrap().scale = Vec3::new(1.0, 1.0, -1.0);
    let drawn = w.transform_point(e, Vec3::Z) - w.world_position(e);
    assert!(w.transform_direction(e, Vec3::Z).abs_diff_eq(drawn, EPS));
    assert!(w.is_mirrored(e));
}

#[test]
fn set_world_pose_round_trips_under_a_turned_mirrored_parent() {
    let mut w = World::new();
    let p = w.spawn("p".into());
    {
        let mut t = w.transform_mut(p).unwrap();
        t.position = Vec3::new(1.0, 2.0, 3.0);
        t.rotation = Quat::from_rotation_z(0.7);
        t.scale = Vec3::new(-2.0, 2.0, 2.0);
    }
    let c = child_of(&mut w, p);
    let (at, facing) = (Vec3::new(-4.0, 5.0, 0.5), Quat::from_rotation_x(0.3));
    assert!(w.set_world_position(c, at) && w.set_world_rotation(c, facing));
    assert!(w.world_position(c).abs_diff_eq(at, 1e-4));
    assert!(w.world_rotation(c).abs_diff_eq(facing, 1e-4));
    let local = w.inverse_transform_point(p, at).unwrap();
    assert!(local.abs_diff_eq(w.transform(c).unwrap().position, 1e-4));
}

#[test]
fn a_singular_parent_keeps_the_local_position_it_cannot_solve_for() {
    // #1164: a group at scale (0,1,1) has no inverse; the child keeps its last value.
    let mut w = World::new();
    let p = w.spawn("p".into());
    w.transform_mut(p).unwrap().scale = Vec3::new(0.0, 1.0, 1.0);
    let c = child_of(&mut w, p);
    w.transform_mut(c).unwrap().position = Vec3::new(1.0, 2.0, 3.0);
    assert_eq!(w.inverse_transform_point(p, Vec3::ONE), None);
    assert!(w.set_world_position(c, Vec3::new(9.0, 9.0, 9.0)));
    assert_eq!(w.transform(c).unwrap().position, Vec3::new(1.0, 2.0, 3.0));
    let pose = w.world_pose(c).unwrap();
    assert!(pose.pos.is_finite() && pose.rot.is_finite() && pose.scale.is_finite());
}

#[test]
fn moving_a_parent_moves_every_collider_under_it() {
    // #1157: the write lands once, for the whole subtree, whoever made it.
    let mut w = World::new();
    let root = w.spawn("root".into());
    let mid = child_of(&mut w, root);
    let leaf = child_of(&mut w, mid);
    w.transform_mut(leaf).unwrap().position = Vec3::X;
    with_box(&mut w, leaf, Vec3::ONE);
    w.transform_mut(root).unwrap().position = Vec3::new(0.0, 10.0, 0.0);
    w.transform_mut(mid).unwrap().rotation = Quat::from_rotation_y(FRAC_PI_2);
    let centre = (aabb(&w, leaf).0 + aabb(&w, leaf).1) * 0.5;
    assert!(
        centre.abs_diff_eq(Vec3::new(0.0, 10.0, -1.0), EPS),
        "{centre}"
    );
    assert!(centre.abs_diff_eq(w.world_position(leaf), EPS));
}

#[test]
fn a_root_reads_its_local_pose_bit_for_bit() {
    let mut w = World::new();
    let e = w.spawn("e".into());
    let r = Quat::from_xyzw(0.1, 0.2, 0.3, 0.9);
    w.transform_mut(e).unwrap().rotation = r;
    assert_eq!(w.world_rotation(e), r);
    assert_eq!(w.lossy_scale(e), Vec3::ONE);
}

#[test]
fn equal_axis_lengths_that_shear_are_not_read_as_a_uniform_scale() {
    // A parent turned under a (2, 1, 1) grandparent so each of its axes has an x
    // component of 1/√3: three axes √2 long, but not square.
    let mut w = World::new();
    let gp = w.spawn("grandparent".into());
    w.transform_mut(gp).unwrap().scale = Vec3::new(2.0, 1.0, 1.0);
    let p = child_of(&mut w, gp);
    w.transform_mut(p).unwrap().rotation = Quat::from_rotation_arc(Vec3::ONE.normalize(), Vec3::X);
    let c = child_of(&mut w, p);
    w.transform_mut(c).unwrap().rotation = Quat::from_rotation_z(FRAC_PI_2 / 2.0);
    let m = w.local_to_world(c);
    let columns = Vec3::new(m.x_axis.length(), m.y_axis.length(), m.z_axis.length());
    let lossy = w.lossy_scale(c);
    assert!(
        lossy.abs().abs_diff_eq(columns, EPS),
        "{lossy} vs {columns}"
    );
    assert!((lossy.x - 3f32.sqrt()).abs() < EPS, "{lossy}");
}
