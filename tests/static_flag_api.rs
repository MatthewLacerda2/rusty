//! `Scene.SetStatic` / `Scene.IsStatic` (#1496): the inspector's Static checkbox
//! from Lua, writing the same `World::set_static` flag — live in Play: physics
//! reads it next tick, and the renderer's static shadow bakes key on the revision
//! it bumps. The nil-id cases are `entity_arg_api`'s shared table.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::{ColliderComponent, ColliderShape};
use rusty::scene::authoring::{default_collider, default_rigidbody};
use rusty::scene::Scene;

use crate::play::{playing, run};

/// A falling 1 m box at y = 10, in Play: (`game`, `scene`, its id).
fn falling_box() -> (GameWorld, Rc<RefCell<Scene>>, u32) {
    let mut s = Scene::new();
    let id = s.add_entity("Crate".to_string());
    s.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 10.0, 0.0);
    let shape = ColliderShape::Box { size: Vec3::ONE };
    let collider = ColliderComponent {
        shape,
        ..default_collider()
    };
    s.world.set_collider(id, Some(collider));
    s.world.set_rigidbody(id, Some(default_rigidbody()));
    let scene = Rc::new(RefCell::new(s));
    let mut game = playing(scene.clone(), 1.0);
    run(&mut game, 1);
    (game, scene, id)
}

#[test]
fn set_static_writes_the_flag_the_inspector_writes() {
    let (game, scene, id) = falling_box();
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    let revision = || scene.borrow().world.static_revision();
    assert_eq!(eval(&format!("return Scene.IsStatic({id})")), "false");
    let before = revision();
    assert_eq!(eval(&format!("return Scene.SetStatic({id}, true)")), "true");
    assert!(scene.borrow().world.is_static(id));
    assert_eq!(eval(&format!("return Scene.IsStatic({id})")), "true");
    assert_ne!(revision(), before, "a flip in Play stales the static bakes");
    let flipped = revision();
    eval(&format!("Scene.SetStatic({id}, true)"));
    assert_eq!(
        revision(),
        flipped,
        "re-setting the same value is no change"
    );
    assert_eq!(
        eval(&format!("return Scene.SetStatic({id}, false)")),
        "true"
    );
    assert!(!scene.borrow().world.is_static(id));
    assert_ne!(
        revision(),
        flipped,
        "true -> false stales them too (a ghost shadow)"
    );
    assert_eq!(eval("return Scene.SetStatic(99999, true)"), "false");
    assert_eq!(eval("return Scene.IsStatic(99999)"), "false");
}

#[test]
fn a_rigidbody_set_static_from_lua_stops_falling() {
    let y = |scene: &Rc<RefCell<Scene>>, id| scene.borrow().world.transform(id).unwrap().position.y;
    let (mut control, falls, id) = falling_box();
    run(&mut control, 30);
    assert!(
        y(&falls, id) < 9.5,
        "the control falls: y = {}",
        y(&falls, id)
    );

    let (mut game, scene, id) = falling_box();
    let at = y(&scene, id);
    let line = format!("Scene.SetStatic({id}, true)");
    game.script_manager().eval(&line).expect("evaluates");
    run(&mut game, 30);
    assert_eq!(y(&scene, id), at, "a static body is a Fixed one");
}
