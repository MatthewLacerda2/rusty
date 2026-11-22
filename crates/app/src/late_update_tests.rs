use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use rusty_components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use rusty_core::input::InputState;
use rusty_navigation::NavigationGraph;
use rusty_scene::{Scene, ScriptComponent};
use rusty_script::scripting::ConsoleLogs;

use super::GameWorld;

const DT: f32 = 1.0 / 60.0;

fn falling_body_world() -> GameWorld {
    let script = rusty_test_support::temp::dir().join("rusty_324_fall.lua");
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
            velocity: Vec3::new(0.0, -12.0, 0.0),
            use_gravity: false,
            ..rusty_edit::authoring::default_rigidbody()
        }),
    );
    // A collider gives the dynamic body mass so rapier integrates it (a
    // rigidbody-only entity has zero mass and never moves).
    s.world.set_collider(
        id,
        Some(ColliderComponent {
            shape: ColliderShape::Box {
                size: Vec3::splat(1.0),
            },
            ..rusty_edit::authoring::default_collider()
        }),
    );
    *s.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
        path: script.to_string_lossy().into_owned(),
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
    gw.tick(DT);

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
