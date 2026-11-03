//! What the occupancy grid tells reflection placement (#1737): an open courtyard,
//! a cluttered room against an empty one.

use glam::Vec3;

use super::test_level::{block, plan_static};
use super::test_sites::yard;
use super::ReflectionPlacement;
use crate::scene::Scene;

#[test]
fn an_open_courtyard_gets_one_probe_without_box_projection() {
    let mut scene = yard();
    let (r, h) = (15.0, 5.0);
    block(
        &mut scene,
        Vec3::new(-r, 0.0, -r),
        Vec3::new(r, h, -r + 0.3),
    );
    block(&mut scene, Vec3::new(-r, 0.0, r - 0.3), Vec3::new(r, h, r));
    block(
        &mut scene,
        Vec3::new(-r, 0.0, -r),
        Vec3::new(-r + 0.3, h, r),
    );
    block(&mut scene, Vec3::new(r - 0.3, 0.0, -r), Vec3::new(r, h, r));
    let plans = plan_static(&scene, &ReflectionPlacement::default());
    assert_eq!(plans.len(), 1, "{plans:?}");
    assert!(!plans[0].box_projection);
}

/// A closed 12 × 12 m room under a roof, with `crates` 1.5 m crates in a row.
fn room(crates: usize) -> Scene {
    let mut scene = Scene::new();
    let (r, h) = (6.0, 4.0);
    block(&mut scene, Vec3::new(-r, -0.2, -r), Vec3::new(r, 0.0, r));
    block(&mut scene, Vec3::new(-r, h, -r), Vec3::new(r, h + 0.2, r));
    for k in 0..crates {
        let (x, z) = (-4.5 + (k % 4) as f32 * 3.0, -4.5 + (k / 4) as f32 * 3.0);
        block(
            &mut scene,
            Vec3::new(x, 0.0, z),
            Vec3::new(x + 1.5, 1.5, z + 1.5),
        );
    }
    scene
}

#[test]
fn a_cluttered_room_gets_more_probes_than_an_empty_one() {
    let params = ReflectionPlacement::default();
    let (empty, full) = (
        plan_static(&room(0), &params),
        plan_static(&room(12), &params),
    );
    assert_eq!(empty.len(), 1, "{empty:?}");
    assert!(
        full.len() > empty.len(),
        "{} vs {}",
        full.len(),
        empty.len()
    );
    assert!(
        full.iter().all(|p| p.box_projection),
        "a roofed room projects"
    );
}
