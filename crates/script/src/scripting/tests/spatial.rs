use std::cell::RefCell;
use std::rc::Rc;

use crate::scripting::console::ConsoleLogs;
use crate::scripting::manager::ScriptManager;
use glam::Vec3;
use rusty_components::{ColliderComponent, ColliderShape};
use rusty_core::input::InputState;
use rusty_navigation::NavigationGraph;
use rusty_physics::PhysicsWorld;
use rusty_scene::Camera;
use rusty_scene::Scene;
use rusty_time::Time;

fn add_box(scene: &mut Scene, name: &str, pos: Vec3, layer: u8) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    scene.world.set_static(id, true);
    scene.world.set_layer(id, layer);
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            shape: ColliderShape::Box {
                size: Vec3::splat(2.0),
            },
            ..rusty_edit::authoring::default_collider()
        }),
    );
    id
}

/// `a` spans [-1,1]³ on layer 0; `b` spans z 9 to 11 on layer 3.
pub(super) fn spatial_runtime() -> (ScriptManager, u32, u32) {
    let mut raw = Scene::new();
    let a = add_box(&mut raw, "A", Vec3::ZERO, 0);
    let b = add_box(&mut raw, "B", Vec3::new(0.0, 0.0, 10.0), 3);
    (runtime_over(raw), a, b)
}

pub(super) fn runtime_over(raw: Scene) -> ScriptManager {
    let scene = Rc::new(RefCell::new(raw));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -10.0, 10.0, -10.0, 10.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let camera = Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0)));
    let time = Rc::new(RefCell::new(Time::new()));
    let mut m = ScriptManager::new(Rc::clone(&scene), input, nav, console, camera, time);
    let physics = m.physics_cell();
    *physics.borrow_mut() = Some(PhysicsWorld::from_scene(&scene.borrow()));
    m.init_runtime().expect("runtime inits");
    m
}

#[test]
fn overlap_sphere_returns_ids_and_honours_the_layer_mask() {
    let (m, a, b) = spatial_runtime();
    let both = m
        .eval("local t = Physics.OverlapSphere(0,0,0, 12); return #t, t[1], t[2]")
        .unwrap();
    assert_eq!(both, format!("2, {}, {}", a.min(b), a.max(b)));
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
    let hit = m
        .eval("select(2, Physics.SphereCast(0,0,2, 0,0,1, 0.5))")
        .unwrap();
    assert!(hit.starts_with(&format!("{b}")), "expected {b}, got {hit}");
    let miss = m
        .eval(&format!("Physics.SphereCast(0,0,2, 0,0,1, 0.5, {b})"))
        .unwrap();
    assert!(miss.starts_with("false"), "ignore skips the target: {miss}");
    let _ = a;
}
