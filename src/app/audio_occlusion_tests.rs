//! Occlusion through the live rapier world (#467): a wall between the camera and a
//! playing source muffles it, removing the wall clears it after smoothing, and the
//! factor is the same on two headless runs. Primitive shapes only.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::app::GameWorld;
use crate::components::{AudioSourceComponent, ColliderComponent, ColliderShape};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

/// A playing 3D source at z = 10, a 20 × 10 × 1 static wall at z = 5 between it
/// and the camera at the origin. Returns the world and the wall's id.
fn walled() -> (GameWorld, u32) {
    let mut scene = Scene::new();
    let wall = scene.add_entity("Wall".to_string());
    scene.world.set_static(wall, true);
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(0.0, 0.0, 5.0);
    let collider = ColliderComponent {
        active: true,
        shape: ColliderShape::Box {
            size: Vec3::new(20.0, 10.0, 1.0),
        },
        is_trigger: false,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    };
    scene.world.set_collider(wall, Some(collider));
    let speaker = scene.add_entity("Speaker".to_string());
    scene.world.transform_mut(speaker).unwrap().position = Vec3::new(0.0, 0.0, 10.0);
    let source = AudioSourceComponent {
        clip: "loop.ogg".to_string(),
        looping: true,
        play_on_start: true,
        spatial_blend: 1.0,
        final_distance: 50.0,
        ..Default::default()
    };
    scene.world.set_audio(speaker, Some(source));
    let scene = Rc::new(RefCell::new(scene));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = NavigationGraph::new(-5.0, 5.0, -5.0, 5.0, 1.0);
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut world = GameWorld::new(scene, input, Rc::new(RefCell::new(nav)), console);
    world.resources.camera.borrow_mut().position = Vec3::ZERO;
    world.set_playing(true);
    (world, wall)
}

/// Tick `frames` frames, returning the speaker's occlusion after each.
fn run(world: &mut GameWorld, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|_| {
            world.tick(DT);
            world.resources.audio.borrow().source_occlusion(2)
        })
        .collect()
}

#[test]
fn a_wall_occludes_and_removing_it_clears_after_smoothing() {
    let (mut world, wall) = walled();
    let walled_in = run(&mut world, 3);
    assert_eq!(*walled_in.last().unwrap(), 1.0, "{walled_in:?}");
    world.scene().borrow_mut().world.set_active(wall, false);
    let clearing = run(&mut world, 30);
    assert!(clearing[2] > 0.0, "smoothed, not snapped: {clearing:?}");
    assert_eq!(*clearing.last().unwrap(), 0.0, "{clearing:?}");
}

#[test]
fn the_factor_is_identical_across_two_headless_runs() {
    let trace = || {
        let (mut world, wall) = walled();
        let mut trace = run(&mut world, 10);
        world.scene().borrow_mut().world.set_active(wall, false);
        trace.extend(run(&mut world, 20));
        trace
    };
    assert_eq!(trace(), trace());
}
