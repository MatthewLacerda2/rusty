//! The floating origin in Play: past the threshold the World shifts at a tick's tail,
//! scripts hear of it, and Stop puts the World back where it was authored.

#[cfg(feature = "dev")]
mod city;

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::CameraComponent;
use rusty::scene::Scene;

use crate::play::{attach_script, playing, run};

/// `game`'s project with the floating origin on at its default 4 km.
pub fn turned_on(game: &GameWorld) {
    let mut app = game.resources.application.borrow_mut();
    let mut settings = app.settings();
    settings.floating_origin.enabled = true;
    app.run_with(settings);
}

/// A scene whose main camera is at `at`, and an empty entity `Listener` beside it.
fn camera_at(at: Vec3) -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let cam = scene.add_entity("Camera".to_string());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    scene.world.transform_mut(cam).unwrap().position = at;
    let listener = scene.add_entity("Listener".to_string());
    (scene, cam, listener)
}

const HEARS: &str = "_G.heard = {}\n\
    return { OnOriginShift = function(id, dx, dy, dz)\n\
        table.insert(heard, string.format('%d %g %g %g', id, dx, dy, dz)) end }";

#[test]
fn a_far_camera_shifts_the_world_and_every_script_hears_it_once() {
    let (mut scene, cam, listener) = camera_at(Vec3::new(30_000.0, 10.0, -5_000.0));
    attach_script(&mut scene, listener, "origin_hears.lua", HEARS);
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(scene.clone(), 10.0);
    turned_on(&game);
    run(&mut game, 3);
    let at = scene.borrow().world.transform(cam).unwrap().position;
    assert_eq!(
        at,
        Vec3::new(30_000.0 - 29.0 * 1024.0, 10.0, -5_000.0 + 5.0 * 1024.0)
    );
    let heard = game
        .script_manager()
        .eval("table.concat(heard, ';')")
        .unwrap();
    assert_eq!(
        heard,
        format!("{listener} -29696 0 5120"),
        "one shift, once"
    );
    let origin = game.script_manager().eval("Scene.GetOrigin()").unwrap();
    assert_eq!(origin, "29696, 0, -5120", "World plus origin is authored");
}

#[test]
fn off_by_default_a_far_world_stays_put() {
    let (scene, cam, _) = camera_at(Vec3::new(30_000.0, 0.0, 0.0));
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(scene.clone(), 10.0);
    run(&mut game, 3);
    assert_eq!(
        scene.borrow().world.transform(cam).unwrap().position.x,
        30_000.0
    );
}

#[test]
fn stop_puts_the_world_back_where_it_was_authored() {
    let (scene, cam, _) = camera_at(Vec3::new(30_000.0, 0.0, 0.0));
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(scene.clone(), 10.0);
    turned_on(&game);
    run(&mut game, 2);
    assert!(scene.borrow().world.transform(cam).unwrap().position.x < 1_000.0);
    game.set_playing(false);
    run(&mut game, 1);
    let s = scene.borrow();
    assert_eq!(s.world.transform(cam).unwrap().position.x, 30_000.0);
    assert!(s.floating_origin.is_zero(), "edit mode is authored space");
}

#[test]
fn a_chosen_focus_decides_the_shift_instead_of_the_camera() {
    let (mut scene, cam, listener) = camera_at(Vec3::ZERO);
    scene.world.transform_mut(listener).unwrap().position = Vec3::new(0.0, 0.0, 9_000.0);
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(scene.clone(), 10.0);
    turned_on(&game);
    run(&mut game, 1);
    game.script_manager()
        .eval(&format!("Scene.SetOriginFocus({listener})"))
        .unwrap();
    run(&mut game, 1);
    let s = scene.borrow();
    assert_eq!(s.world.transform(cam).unwrap().position.z, -9.0 * 1024.0);
}
