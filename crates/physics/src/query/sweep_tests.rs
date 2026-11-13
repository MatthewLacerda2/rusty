//! crates/physics/src/query/sweep_tests.rs — `shape_cast_all`'s candidates (#1881).
//!
//! On a field of boxes, an unlimited diagonal sweep visits only the colliders
//! along its path (not every one in the box around the sweep, which with an
//! unlimited distance is all of them ahead), and finds exactly the hits a cast
//! against each collider alone finds.

use glam::Vec3;
use rapier3d::prelude::QueryFilter;

use super::sweep::{swept_candidates, Sweep};
use crate::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape};
use rusty_scene::Scene;

const SIDE: i32 = 20;

/// A flat `SIDE`×`SIDE` field of static 1 m boxes, 4 m apart, around the
/// origin: some lie behind and beside any sweep from it.
fn field() -> (PhysicsWorld, Vec<u32>) {
    let mut scene = Scene::new();
    let mut ids = Vec::new();
    for i in -SIDE / 2..SIDE / 2 {
        for j in -SIDE / 2..SIDE / 2 {
            let id = scene.add_entity("Box".to_string());
            let at = Vec3::new(i as f32 * 4.0 + 2.0, 0.0, j as f32 * 4.0 + 2.0);
            scene.world.transform_mut(id).unwrap().position = at;
            scene.world.set_static(id, true);
            let collider = ColliderComponent {
                shape: ColliderShape::Box { size: Vec3::ONE },
                ..rusty_scene::authoring::default_collider()
            };
            scene.world.set_collider(id, Some(collider));
            ids.push(id);
        }
    }
    (PhysicsWorld::from_scene(&scene), ids)
}

#[test]
fn an_unlimited_diagonal_sweep_visits_only_its_path_and_misses_nothing() {
    let (physics, ids) = field();
    let sweep = Sweep::Sphere {
        center: Vec3::ZERO,
        radius: 0.6,
    };
    let dir = Vec3::new(1.0, 0.0, 1.0).normalize();
    let hits = physics.shape_cast_all(sweep, dir, f32::INFINITY, |_| true);
    // The boxes on the diagonal: (2,2), (6,6), … (38,38), ten of them.
    assert_eq!(hits.len(), (SIDE / 2) as usize, "{hits:?}");
    assert!(hits.windows(2).all(|w| w[0].distance <= w[1].distance));
    for id in &ids {
        let alone = physics.shape_cast_hit(sweep, dir, f32::INFINITY, |e| e == *id);
        let listed = hits.iter().find(|h| h.id == *id);
        assert_eq!(alone.map(|h| h.id), listed.map(|h| h.id), "entity {id}");
    }
    let queries = physics.queries(QueryFilter::default());
    let start = rapier3d::parry::bounding_volume::Aabb::new(Vec3::splat(-0.6), Vec3::splat(0.6));
    let visited = swept_candidates(&queries, start, dir, f32::INFINITY).count();
    // The box around this sweep holds every box ahead (a quarter of the
    // field, 100); the path holds the diagonal and its near neighbours.
    assert!(visited < 40, "visited {visited} of {}", ids.len());
}

#[test]
fn a_zero_radius_sweep_casts_like_a_ray() {
    let (physics, _) = field();
    let point = Sweep::Sphere {
        center: Vec3::new(2.0, 0.0, -50.0),
        radius: 0.0,
    };
    let hits = physics.shape_cast_all(point, Vec3::Z, 100.0, |_| true);
    assert_eq!(hits.len(), SIDE as usize, "one column of boxes");
    assert!(
        (hits[0].distance - (50.0 - 38.0 - 0.5)).abs() < 1e-3,
        "{hits:?}"
    );
}
