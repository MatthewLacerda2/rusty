//! `raycast_through` over per-bone hitboxes (#830): a shot through the head
//! names the bone and the character, and reports how much of the head it crossed.

use glam::Vec3;
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

use super::rig;

#[test]
fn a_ray_through_the_head_reports_its_bone_entry_and_exit() {
    let mut scene = Scene::new();
    let rig = rig::character(&mut scene, Vec3::ZERO, false);
    let head = scene.find_bone(rig, "head").unwrap();
    let physics = PhysicsWorld::from_scene(&scene);
    let crossings = physics.raycast_through(Vec3::new(0.0, 1.85, -5.0), Vec3::Z, 20.0, |_| true);
    let through_head: Vec<_> = crossings
        .iter()
        .filter(|c| scene.hit_bone(c.enter.id) == Some(head))
        .collect();
    let [c] = through_head[..] else {
        panic!("one crossing through the head: {crossings:?}");
    };
    assert_eq!(scene.root_of(c.enter.id), rig);
    let exit = c.exit.expect("the shot leaves the head");
    assert!(c.enter.point.z < 0.0 && exit.point.z > 0.0, "{c:?}");
    assert!(
        (c.thickness - (exit.distance - c.enter.distance)).abs() < 1e-5 && c.thickness > 0.1,
        "{c:?}"
    );
}
