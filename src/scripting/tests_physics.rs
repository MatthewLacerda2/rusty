use std::cell::RefCell;
use std::rc::Rc;

use super::console::ConsoleLogs;
use super::manager::ScriptManager;
use crate::components::{ColliderComponent, ColliderShape};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;
use glam::Vec3;

/// #31: a script's `Physics.Raycast` and the engine's `cast_ray` resolve to
/// the *same* entity for the same ray, because both go through the one rapier
/// world. The binding casts against the live world shared via the handle.
#[test]
fn raycast_matches_engine_cast_through_rapier() {
    let mut raw = Scene::new();
    let target = raw.add_entity("Target".to_string());
    if let Some(mut e) = raw.get_entity_mut(target) {
        e.transform.position = Vec3::new(0.0, 0.0, 5.0);
        e.is_static = true;
        e.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box {
                size: Vec3::splat(2.0),
            },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        });
    }
    let scene = Rc::new(RefCell::new(raw));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -10.0, 10.0, -10.0, 10.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let camera = Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0)));
    let time = Rc::new(RefCell::new(Time::new()));
    let mut m = ScriptManager::new(Rc::clone(&scene), input, nav, console, camera, time);
    let physics = Rc::new(RefCell::new(Some(PhysicsWorld::from_scene(
        &scene.borrow(),
    ))));
    m.init_runtime(&physics).expect("runtime inits");

    // Engine path: cast straight down +Z, expect the target.
    let engine = physics
        .borrow()
        .as_ref()
        .unwrap()
        .cast_ray(Vec3::ZERO, Vec3::Z, f32::MAX)
        .map(|(id, _)| id);
    assert_eq!(engine, Some(target), "engine cast should hit the target");

    // Script path: the binding returns (hit, id, dist); pull the id back out.
    let lua_id: u32 = m
        .eval("select(2, Physics.Raycast(0,0,0, 0,0,1))")
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(Some(lua_id), engine, "script raycast must match the engine");
}
