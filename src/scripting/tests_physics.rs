use super::console::ConsoleLogs;
use super::manager::{ScriptCtx, ScriptManager};
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
/// world. The binding casts against the live world borrowed through the scope.
#[test]
fn raycast_matches_engine_cast_through_rapier() {
    let mut scene = Scene::new();
    let target = scene.add_entity("Target".to_string());
    if let Some(mut e) = scene.get_entity_mut(target) {
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
    let mut physics = Some(PhysicsWorld::from_scene(&scene));

    // Engine path: cast straight down +Z, expect the target.
    let engine = physics
        .as_ref()
        .unwrap()
        .cast_ray(Vec3::ZERO, Vec3::Z, f32::MAX)
        .map(|(id, _)| id);
    assert_eq!(engine, Some(target), "engine cast should hit the target");

    let mut input = InputState::new();
    let mut nav = NavigationGraph::new(-10.0, 10.0, -10.0, 10.0, 1.0);
    let mut console = ConsoleLogs::new();
    let mut camera = Camera::new(Vec3::ZERO, 0.0, 0.0);
    let mut time = Time::new();
    let mut ctx = ScriptCtx {
        scene: &mut scene,
        input: &mut input,
        nav: &mut nav,
        console: &mut console,
        camera: &mut camera,
        time: &mut time,
        physics: &mut physics,
    };

    let mut m = ScriptManager::new();
    m.init_runtime().expect("runtime inits");

    // Script path: the binding returns (hit, id, dist); pull the id back out.
    let lua_id: u32 = m
        .eval(&mut ctx, "select(2, Physics.Raycast(0,0,0, 0,0,1))")
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(Some(lua_id), engine, "script raycast must match the engine");
}
