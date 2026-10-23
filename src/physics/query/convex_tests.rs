//! src/physics/query/convex_tests.rs — point queries at a convex mesh collider's
//! corners (#1211).
//!
//! parry's point projection can call a point that sits exactly on a hull vertex
//! outside and answer a point far away. The engine answers a convex hull by its
//! face planes first, so a corner is its own closest point and is contained.

use glam::Vec3;

use super::convex_fixture::{flat_hull, turned_hull};
use crate::physics::PhysicsWorld;

#[test]
fn every_corner_of_a_turned_hull_is_its_own_closest_point() {
    let (scene, id, corners) = turned_hull();
    let world = PhysicsWorld::from_scene(&scene);
    for corner in corners {
        let closest = world.closest_point_on(id, corner).expect("a live collider");
        assert!(
            (closest - corner).length() < 0.01,
            "{corner}: closest point {closest} is {} away",
            (closest - corner).length()
        );
        assert_eq!(world.collider_contains_point(id, corner), Some(true));
    }
}

#[test]
fn a_point_off_a_turned_hull_still_projects_onto_it() {
    let (scene, id, corners) = turned_hull();
    let world = PhysicsWorld::from_scene(&scene);
    let centre = corners.iter().sum::<Vec3>() / corners.len() as f32;
    for corner in corners {
        // Out along the line from the centre, 10 units past the corner.
        let out = corner + (corner - centre).normalize() * 10.0;
        let closest = world.closest_point_on(id, out).expect("a live collider");
        let gap = (closest - out).length();
        assert!(gap <= 10.01 && gap > 1.0, "{out}: projected {gap} away");
        assert_eq!(world.collider_contains_point(id, out), Some(false));
    }
}

#[test]
fn a_flat_hull_holds_only_its_outline() {
    // A flat hull's two face planes hold its whole plane: it is answered by parry.
    let (scene, id) = flat_hull();
    let world = PhysicsWorld::from_scene(&scene);
    for (out, edge) in [
        (Vec3::new(500.0, 0.0, 500.0), Vec3::new(7.5, 0.0, 7.5)),
        (Vec3::new(10.0, 0.0, 1.0), Vec3::new(7.5, 0.0, 1.0)),
    ] {
        assert_eq!(world.collider_contains_point(id, out), Some(false), "{out}");
        let closest = world.closest_point_on(id, out).expect("a live collider");
        assert!(
            (closest - edge).length() < 0.01,
            "{out}: projected to {closest}"
        );
    }
}
