//! End-to-end proof of the #324 post-physics ordering through the real
//! `GameWorld::tick` schedule (not the dispatch helper in isolation): a dynamic
//! body moves during the physics step, so a script's `Update` records the
//! pre-physics height and its `LateUpdate` records the post-physics one — in the
//! same fixed tick, because `late_update_scripts` is registered after
//! `step_physics`/`animate`. Determinism: fixed dt, downward velocity, gravity
//! off, so the drop is an exact `-12 * dt` per step.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::{Scene, ScriptComponent};
use crate::scripting::ConsoleLogs;

use super::GameWorld;

const DT: f32 = 1.0 / 60.0;

/// A play-ready world with one entity at y=10 that carries a downward-velocity
/// dynamic body (gravity off ⇒ an exact -12·dt drop per step) plus a script that
/// samples its own height in both `Update` (pre-physics) and `LateUpdate`
/// (post-physics).
fn falling_body_world() -> GameWorld {
    let script = std::env::temp_dir().join("rusty_324_fall.lua");
    std::fs::write(
        &script,
        "_G.__u = 0\n_G.__l = 0\nreturn {\n\
         Update = function(id) _G.__u = ({Transform.GetPosition(id)})[2] end,\n\
         LateUpdate = function(id) _G.__l = ({Transform.GetPosition(id)})[2] end,\n}",
    )
    .unwrap();

    let mut s = Scene::new();
    let id = s.add_entity("Faller".to_string());
    s.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 10.0, 0.0);
    s.world.set_rigidbody(
        id,
        Some(RigidBodyComponent {
            active: true,
            is_kinematic: false,
            mass: 1.0,
            velocity: Vec3::new(0.0, -12.0, 0.0),
            angular_velocity: Vec3::ZERO,
            use_gravity: false,
            collision_detection: CollisionDetection::Discrete,
        }),
    );
    // A collider gives the dynamic body mass so rapier integrates it (a
    // rigidbody-only entity has zero mass and never moves).
    s.world.set_collider(
        id,
        Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box {
                size: Vec3::splat(1.0),
            },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        }),
    );
    // Forward slashes: a Windows `\` would be a Lua escape in the path literal.
    *s.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
        path: script.to_string_lossy().replace('\\', "/"),
        ..Default::default()
    }];

    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut gw = GameWorld::new(Rc::new(RefCell::new(s)), input, nav, console);
    gw.set_playing(true);
    gw
}

#[test]
fn late_update_sees_post_physics_transform_through_the_play_loop() {
    let mut gw = falling_body_world();
    gw.tick(DT); // one fixed tick: Update → physics (body falls) → LateUpdate

    let eval = |line: &str| {
        gw.script_manager()
            .eval(line)
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let u = eval("__u");
    let l = eval("__l");
    assert!(
        (u - 10.0).abs() < 1e-5,
        "Update saw the pre-physics height, got {u}"
    );
    // Gravity off, v = -12 m/s ⇒ exactly -12 * dt = -0.2 m over this step.
    assert!(
        (l - 9.8).abs() < 1e-4,
        "LateUpdate saw the post-physics height, got {l}"
    );
    assert!(
        l < u,
        "LateUpdate must observe the resolved (lower) position"
    );
}
