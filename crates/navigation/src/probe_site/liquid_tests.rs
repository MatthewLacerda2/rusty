//! A liquid is not floor on the nav surface either (#1737): a pit with water at
//! deck level.

use glam::Vec3;

use super::plan_on_nav;
use rusty_scene::lighting::placement::test_sites::pool;

#[test]
fn no_capture_point_is_at_or_under_the_water() {
    let scene = pool();
    let in_pit = |p: Vec3| p.x.abs() < 3.0 && p.z.abs() < 3.0 && p.y <= 0.0;
    let plans = plan_on_nav(&scene);
    assert!(!plans.is_empty());
    for p in &plans {
        assert!(!in_pit(p.position), "a capture in the pit: {p:?}");
        assert!(p.position.y > 1.0, "eye height over the deck: {p:?}");
    }
}
