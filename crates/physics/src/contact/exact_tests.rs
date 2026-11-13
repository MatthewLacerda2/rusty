//! crates/physics/src/contact/exact_tests.rs — a static or kinematic pillar keeps its
//! exact, smooth side; only a dynamic cylinder rolls as a hull (#2071).

use glam::{Quat, Vec3};

use super::fixture::{body, DT};
use crate::PhysicsWorld;
use rusty_components::ColliderShape;
use rusty_scene::Scene;

const PILLAR: ColliderShape = ColliderShape::Cylinder {
    radius: 1.0,
    height: 2.0,
};

/// Which body a pillar is on.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pillar {
    Static,
    Kinematic,
    Dynamic,
}

/// The largest angle, in degrees, between a ball's contact normal on an upright
/// pillar (r 1) and the true radial direction, the ball set against the side a
/// quarter of a hull facet off a facet's normal, where a hull's flat facet
/// tilts the normal by that quarter (1.4°) and the exact side does not.
fn worst_normal_tilt(pillar: Pillar) -> f32 {
    let mut scene = Scene::new();
    let still = Vec3::ZERO;
    let id = body(&mut scene, PILLAR, Quat::IDENTITY, Vec3::ZERO, still, still);
    {
        let mut rb = scene.world.rigidbody_mut(id).unwrap();
        (rb.use_gravity, rb.mass) = (false, 1.0e4);
        rb.is_kinematic = pillar == Pillar::Kinematic;
    }
    scene.world.set_static(id, pillar == Pillar::Static);
    let angle = std::f32::consts::TAU / super::side::SIDES as f32 / 4.0;
    let toward = Vec3::new(angle.cos(), 0.0, angle.sin());
    let ball = ColliderShape::Sphere { radius: 0.25 };
    let ball = body(
        &mut scene,
        ball,
        Quat::IDENTITY,
        toward * 1.249,
        -toward * 0.1,
        still,
    );
    scene.world.rigidbody_mut(ball).unwrap().use_gravity = false;

    let mut world = PhysicsWorld::from_scene(&scene);
    let (h1, h2) = (world.id_to_collider[&id], world.id_to_collider[&ball]);
    let (mut worst, mut touched): (f32, bool) = (0.0, false);
    for _ in 0..30 {
        world.step(&mut scene, DT);
        let (p1, p2) = (position(&scene, id), position(&scene, ball));
        let radial = Vec3::new(p2.x - p1.x, 0.0, p2.z - p1.z).normalize();
        let pair = world.narrow_phase.contact_pair(h1, h2);
        for m in pair.map_or(&[][..], |p| p.manifolds()) {
            if !m.points.is_empty() {
                touched = true;
                let n = Vec3::from(m.data.normal.to_array());
                worst = worst.max(n.dot(radial).abs().min(1.0).acos().to_degrees());
            }
        }
    }
    assert!(touched, "the ball never touched the {pillar:?} pillar");
    worst
}

fn position(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}

#[test]
fn a_static_or_kinematic_pillar_has_a_smooth_side() {
    for pillar in [Pillar::Static, Pillar::Kinematic] {
        let tilt = worst_normal_tilt(pillar);
        assert!(tilt < 0.1, "{pillar:?} pillar tilts a contact by {tilt}°");
    }
    // The control: a dynamic one rolls as a hull, so the same contact tilts.
    let tilt = worst_normal_tilt(Pillar::Dynamic);
    assert!(tilt > 1.0, "a dynamic pillar's hull tilts only {tilt}°");
}
