//! `Damage.ApplyPoint` on a `Physics.RaycastThrough` crossing, which keeps where
//! the shot landed under `enter`: the event still carries it and the impulse
//! still lands.

use glam::Vec3;
use rusty::components::RigidBodyComponent;
use rusty::scene::authoring::default_rigidbody;
use rusty::scene::Scene;

use super::{eval, play, static_box};

/// Keeps the last event's hit in the global `HIT`.
const KEEPER: &str = r#"
local M = {}
function M.OnTakeDamage(id, amount, event) HIT = event.hit end
return M
"#;

#[test]
fn a_crossing_carries_its_entry_point_and_takes_the_impulse() {
    let mut scene = Scene::new();
    let crate_id = static_box(&mut scene, Vec3::ZERO, Vec3::ONE, Some(KEEPER));
    scene.world.set_static(crate_id, false);
    let body = RigidBodyComponent {
        use_gravity: false,
        is_kinematic: false,
        ..default_rigidbody()
    };
    scene.world.set_rigidbody(crate_id, Some(body));
    let game = play(scene);
    let line = "local s = Physics.RaycastThrough(0,0,-5, 0,0,1, 20)[1] \
                local taken = Damage.ApplyPoint(s, 10, 0,0,1, { impulse = 5 }) \
                local _, _, vz = Physics.GetVelocity(s.id) \
                return taken, HIT.point.z, HIT.normal.z, HIT.distance, HIT.thickness, vz > 0";
    assert_eq!(
        eval(&game, line),
        "10, -0.5, -1, 4.5, 1, true",
        "the entry's point, normal and distance ride up into event.hit; the crate moves"
    );
}
