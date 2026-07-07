//! Lua bindings for the #311 spatial query surface answer through the same live
//! rapier world as the engine: each test compares a `Physics.*` query evaluated
//! from script against the `PhysicsWorld` answer (or hand-computed geometry)
//! for the same volume, including the layer-mask filter (#91).

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

/// Add a static 2×2×2 box entity at `pos` on layer `layer`.
fn add_box(scene: &mut Scene, name: &str, pos: Vec3, layer: u8) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    scene.world.set_static(id, true);
    scene.world.set_layer(id, layer);
    scene.world.set_collider(id, Some(ColliderComponent {
        active: true,
        shape: ColliderShape::Box {
            size: Vec3::splat(2.0),
        },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    });
    scene.update_entity_collider(id);
    id
}

/// A booted script runtime over a scene with two boxes: `a` at origin (layer 0)
/// and `b` at z=10 (layer 3), plus the live physics world they were built into.
fn spatial_runtime() -> (ScriptManager, u32, u32) {
    let mut raw = Scene::new();
    let a = add_box(&mut raw, "A", Vec3::ZERO, 0);
    let b = add_box(&mut raw, "B", Vec3::new(0.0, 0.0, 10.0), 3);
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
    (m, a, b)
}

#[test]
fn overlap_sphere_returns_ids_and_honours_the_layer_mask() {
    let (m, a, b) = spatial_runtime();
    // Radius 12 at origin swallows both boxes; ids come back sorted ascending.
    let both = m
        .eval("local t = Physics.OverlapSphere(0,0,0, 12); return #t, t[1], t[2]")
        .unwrap();
    assert_eq!(both, format!("2, {}, {}", a.min(b), a.max(b)));
    // A layer mask for layer 3 keeps only `b`; radius 2 reaches only `a`.
    let masked = m
        .eval("local t = Physics.OverlapSphere(0,0,0, 12, 1 << 3); return #t, t[1]")
        .unwrap();
    assert_eq!(masked, format!("1, {b}"));
    let near = m
        .eval("local t = Physics.OverlapSphere(0,0,0, 2); return #t, t[1]")
        .unwrap();
    assert_eq!(near, format!("1, {a}"));
}

#[test]
fn overlap_box_capsule_and_checks_agree_with_the_geometry() {
    let (m, a, b) = spatial_runtime();
    // A thin box between the entities touches neither; stretched along z it
    // spans both — and the capsule along the same span agrees.
    assert_eq!(
        m.eval("#Physics.OverlapBox(0,0,5, 0.5,0.5,0.5)").unwrap(),
        "0"
    );
    assert_eq!(
        m.eval("#Physics.OverlapBox(0,0,5, 0.5,0.5,5)").unwrap(),
        "2"
    );
    let capsule = m
        .eval("local t = Physics.OverlapCapsule(0,0,0, 0,0,10, 0.5); return t[1], t[2]")
        .unwrap();
    assert_eq!(capsule, format!("{}, {}", a.min(b), a.max(b)));
    // The boolean fast-path matches the array emptiness.
    assert_eq!(m.eval("Physics.CheckSphere(0,0,5, 1)").unwrap(), "false");
    assert_eq!(m.eval("Physics.CheckSphere(0,0,10, 1)").unwrap(), "true");
    assert_eq!(
        m.eval("Physics.CheckBox(0,0,5, 0.5,0.5,0.5)").unwrap(),
        "false"
    );
    assert_eq!(
        m.eval("Physics.CheckBox(0,0,5, 0.5,0.5,5)").unwrap(),
        "true"
    );
}

#[test]
fn sphere_cast_hits_with_thickness_and_skips_ignored() {
    let (m, a, b) = spatial_runtime();
    // Cast from just outside `a` (z=1 is its +Z face) toward `b`: a plain hit.
    let hit = m
        .eval("select(2, Physics.SphereCast(0,0,2, 0,0,1, 0.5))")
        .unwrap();
    assert!(hit.starts_with(&format!("{b}")), "expected {b}, got {hit}");
    // Ignoring `b` leaves nothing ahead of the sweep.
    let miss = m
        .eval(&format!("Physics.SphereCast(0,0,2, 0,0,1, 0.5, {b})"))
        .unwrap();
    assert!(miss.starts_with("false"), "ignore skips the target: {miss}");
    let _ = a;
}

#[test]
fn point_queries_match_the_collider_geometry() {
    let (m, a, _) = spatial_runtime();
    // `a` spans [-1,1]³: from (5,0,0) the closest point is its +X face.
    assert_eq!(
        m.eval(&format!("Physics.ClosestPoint({a}, 5,0,0)"))
            .unwrap(),
        "true, 1, 0, 0"
    );
    assert_eq!(
        m.eval(&format!("Physics.ContainsPoint({a}, 0,0,0)"))
            .unwrap(),
        "true"
    );
    assert_eq!(
        m.eval(&format!("Physics.ContainsPoint({a}, 0,0,3)"))
            .unwrap(),
        "false"
    );
    // GetBounds surfaces the cached world AABB: [-1,1] on every axis.
    assert_eq!(
        m.eval(&format!("Physics.GetBounds({a})")).unwrap(),
        "true, -1, -1, -1, 1, 1, 1"
    );
    assert!(m
        .eval("Physics.GetBounds(9999)")
        .unwrap()
        .starts_with("false"));
}
