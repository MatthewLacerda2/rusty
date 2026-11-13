use super::*;
use rusty_components::CapsuleAxis;
use rusty_scene::{ColliderComponent, ColliderShape};

fn with_collider(shape: ColliderShape, pos: Vec3, trigger: bool) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("c".to_string());
    scene.world.transform_mut(id).expect("fresh").position = pos;
    let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    col.shape = shape;
    col.is_trigger = trigger;
    scene.world.set_collider(id, Some(col));
    (scene, id)
}

fn reach(wire: &ColliderWire, centre: Vec3) -> (f32, f32) {
    let ends = wire.segments.iter().flatten();
    let up = ends
        .clone()
        .map(|p| (p - centre).y)
        .fold(f32::MIN, f32::max);
    let out = ends.map(|p| (p - centre).x.abs()).fold(0.0, f32::max);
    (up, out)
}

/// A box draws its twelve edges at its pose, not a bigger axis-aligned box.
#[test]
fn a_box_is_twelve_edges_where_it_stands() {
    let size = Vec3::new(2.0, 1.0, 4.0);
    let (scene, id) = with_collider(ColliderShape::Box { size }, Vec3::new(5.0, 0.0, 0.0), false);
    let wire = collider_wire(&scene, id).expect("a box");
    assert_eq!(wire.segments.len(), 12);
    assert!(!wire.trigger);
    for [a, b] in &wire.segments {
        let d = (*b - *a).abs();
        let len = d.max_element();
        assert!([2.0, 1.0, 4.0].contains(&len), "edge {a} {b}");
        assert!(d.min_element() < 1e-6, "an edge runs along one axis");
    }
}

/// A capsule is its real shape: it reaches its full height over its caps, and no
/// point of it sits outside its radius from its axis.
#[test]
fn a_capsule_reaches_its_caps_and_stays_round() {
    let shape = ColliderShape::Capsule {
        radius: 0.5,
        height: 2.0,
        axis: CapsuleAxis::Y,
    };
    let (scene, id) = with_collider(shape, Vec3::ZERO, true);
    let wire = collider_wire(&scene, id).expect("a capsule");
    assert!(wire.trigger, "a trigger says so");
    let (up, out) = reach(&wire, Vec3::ZERO);
    assert!((up - 1.0).abs() < 1e-4, "reaches its cap's pole: {up}");
    assert!((out - 0.5).abs() < 1e-4, "its radius: {out}");
    for p in wire.segments.iter().flatten() {
        let off_axis = Vec3::new(p.x, 0.0, p.z).length();
        assert!(off_axis <= 0.5 + 1e-4, "{p} outside the radius");
    }
}

/// Every primitive closes its circles: each segment's end is another's start.
#[test]
fn spheres_and_cylinders_are_closed_outlines() {
    for shape in [
        ColliderShape::Sphere { radius: 1.0 },
        ColliderShape::Cylinder {
            radius: 1.0,
            height: 2.0,
        },
    ] {
        let (scene, id) = with_collider(shape.clone(), Vec3::ZERO, false);
        let wire = collider_wire(&scene, id).expect("a primitive");
        let (up, out) = reach(&wire, Vec3::ZERO);
        assert!(
            (up - 1.0).abs() < 1e-4 && (out - 1.0).abs() < 1e-4,
            "{shape:?}"
        );
    }
}

/// No collider, no wire; an inactive one neither.
#[test]
fn no_active_collider_draws_nothing() {
    let mut scene = Scene::new();
    let id = scene.add_entity("bare".to_string());
    assert!(collider_wire(&scene, id).is_none());
    let (mut scene, id) = with_collider(ColliderShape::Sphere { radius: 1.0 }, Vec3::ZERO, false);
    scene.world.collider_mut(id).expect("collider").active = false;
    assert!(collider_wire(&scene, id).is_none());
}
