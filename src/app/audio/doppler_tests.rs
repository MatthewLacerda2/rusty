//! Doppler through the world's ticks (#1749): a source parented under a moving
//! body is measured by its world position, a pause holds the rate, and a scene load
//! forgets the listener's motion.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::app::GameWorld;
use crate::components::AudioSourceComponent;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

const DT: f32 = 0.01;

/// A playing world with a fully 3D speaker under a `Rocket` 100 m down -X of the
/// listener at the origin: `(world, rocket, speaker)`.
fn rocket() -> (GameWorld, u32, u32) {
    let mut scene = Scene::new();
    let rocket = scene.add_entity("Rocket".to_string());
    let speaker = scene.add_entity("Speaker".to_string());
    scene.set_parent(speaker, Some(rocket)).unwrap();
    scene.world.transform_mut(rocket).unwrap().position = Vec3::new(-100.0, 0.0, -2.0);
    let source = AudioSourceComponent {
        clip: "hum.ogg".to_string(),
        looping: true,
        play_on_start: true,
        spatial_blend: 1.0,
        ..Default::default()
    };
    scene.world.set_audio(speaker, Some(source));
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    game.boot_standalone();
    game.resources.camera.borrow_mut().position = Vec3::ZERO;
    game.tick(DT);
    (game, rocket, speaker)
}

/// Move the rocket `dx` along +X, then run one tick of `DT`.
fn fly(game: &mut GameWorld, rocket: u32, dx: f32) {
    let scene = game.scene().clone();
    scene
        .borrow_mut()
        .world
        .transform_mut(rocket)
        .unwrap()
        .position
        .x += dx;
    game.tick(DT);
}

fn rate(game: &GameWorld, speaker: u32) -> f32 {
    game.resources.audio.borrow().source_doppler(speaker)
}

#[test]
fn a_source_under_a_moving_parent_is_shifted() {
    let (mut game, rocket, speaker) = rocket();
    assert!(game.resources.audio.borrow().is_source_playing(speaker));
    assert_eq!(rate(&game, speaker), 1.0, "still at the start");
    fly(&mut game, rocket, 50.0 * DT);
    let r = rate(&game, speaker);
    assert!((r - 343.0 / 293.0).abs() < 1e-3, "{r}");
}

#[test]
fn a_pause_holds_the_rate_and_the_resume_does_not_jump() {
    let (mut game, rocket, speaker) = rocket();
    fly(&mut game, rocket, 50.0 * DT);
    let moving = rate(&game, speaker);
    game.time().borrow_mut().pause();
    game.paused_frame();
    assert_eq!(rate(&game, speaker), moving, "paused, the rate holds");
    game.time().borrow_mut().resume();
    fly(&mut game, rocket, 0.0);
    assert_eq!(rate(&game, speaker), 1.0, "stopped while paused: still");
}

#[test]
fn a_scene_load_forgets_the_listener_motion() {
    let (mut game, rocket, speaker) = rocket();
    let level = crate::test_temp::dir().join("rusty_1749_level.scene");
    Scene::new().save_to_file(&level.to_string_lossy()).unwrap();
    let scene = game.scene().clone();
    scene.borrow_mut().dont_destroy_on_load(rocket); // the speaker rides along
    scene
        .borrow_mut()
        .request_load(level.to_string_lossy().into_owned());
    game.tick(DT); // the load lands at the tick's tail
    assert!(game.resources.audio.borrow().is_source_playing(speaker));
    // The camera lands 1 m closer: 100 m/s if it were measured across the load.
    game.resources.camera.borrow_mut().position = Vec3::new(-1.0, 0.0, 0.0);
    game.tick(DT);
    assert_eq!(rate(&game, speaker), 1.0, "the load forgot the listener");
    game.resources.camera.borrow_mut().position = Vec3::new(-2.0, 0.0, 0.0);
    game.tick(DT);
    assert!(rate(&game, speaker) > 1.0, "measured again after the load");
}
