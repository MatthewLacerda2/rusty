//! The audio mix push (#412, #1579): a playing `AudioSource` is re-resolved against
//! the active camera and the clock on every tick and on a paused frame that ran
//! none, whoever drives the world, and the device receives what `Audio.GetSpatial`
//! reports.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::app::GameWorld;
use crate::audio::recording::{Recording, RecordingBackend};
use crate::components::AudioSourceComponent;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// A playing world with one fully-3D `play_on_start` speaker at the origin, on a
/// recording backend.
fn playing_speaker() -> (GameWorld, u32, Rc<RefCell<Recording>>) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Speaker".to_string());
    scene.world.set_audio(
        id,
        Some(AudioSourceComponent {
            clip: "hum.ogg".to_string(),
            looping: true,
            play_on_start: true,
            spatial_blend: 1.0,
            initial_distance: 1.0,
            final_distance: 11.0,
            ..Default::default()
        }),
    );
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    let (backend, rec) = RecordingBackend::new();
    game.resources.audio.borrow_mut().set_backend(backend);
    game.boot_standalone();
    game.tick(0.02);
    (game, id, rec)
}

/// Put the listener (the render camera) at `pos`, facing -Z (right = +X).
fn place_camera(game: &GameWorld, pos: Vec3) {
    let mut cam = game.resources.camera.borrow_mut();
    cam.position = pos;
}

#[test]
fn the_frame_mix_pans_a_source_to_the_listeners_left() {
    let (game, id, rec) = playing_speaker();
    assert!(game.resources.audio.borrow().is_source_playing(id));
    let right = game.resources.camera.borrow().right();
    place_camera(&game, right * 3.0);
    game.push_audio(0.0);
    let voice = rec.borrow().last_voice();
    let applied = rec.borrow().current(voice).unwrap();
    assert!(applied.pan < -0.9, "got {applied:?}");
    assert!((applied.gain - 0.8).abs() < 1e-4, "3 m in a 1→11 band");
}

#[test]
fn the_frame_mix_matches_the_spatial_read_back() {
    let (game, id, rec) = playing_speaker();
    place_camera(&game, Vec3::new(2.0, 0.0, 5.0));
    game.push_audio(0.0);
    let (src, pos) = crate::api::audio::emitter(&game.scene().borrow(), id).unwrap();
    let listener = crate::api::audio::listener(&game.resources.camera, &game.world.scene.borrow());
    let read = game
        .resources
        .audio
        .borrow()
        .voice_info(id, &src, pos, &listener)
        .spatial;
    let applied = rec.borrow().current(rec.borrow().last_voice()).unwrap();
    assert_eq!((applied.gain, applied.pan), (read.gain, read.pan));
}

#[test]
fn the_frame_mix_follows_time_scale_and_pause() {
    let (game, id, _rec) = playing_speaker();
    game.time().borrow_mut().set_time_scale(0.25);
    game.push_audio(0.0);
    let audio = || game.resources.audio.borrow().voice_mix(id).unwrap();
    assert_eq!(audio().speed, 0.25);
    game.time().borrow_mut().pause();
    game.push_audio(0.0);
    assert!(audio().paused);
    game.time().borrow_mut().resume();
    game.push_audio(0.0);
    assert!(!audio().paused);
}

#[test]
fn a_paused_frame_with_no_step_still_pushes_the_mix() {
    let (mut game, id, rec) = playing_speaker();
    rec.borrow_mut().advances.clear();
    game.time().borrow_mut().pause();
    game.paused_frame();
    assert_eq!(rec.borrow().advances, [0.0], "one push, no time, no tick");
    assert!(game.resources.audio.borrow().voice_mix(id).unwrap().paused);
}

#[test]
fn a_ticking_frame_pushes_the_mix_once() {
    let (mut game, _id, rec) = playing_speaker();
    rec.borrow_mut().advances.clear();
    game.tick(0.02);
    game.time().borrow_mut().pause();
    game.time().borrow_mut().request_steps(1);
    game.paused_frame();
    let fixed = f64::from(crate::time::FIXED_DELTA_TIME);
    assert_eq!(rec.borrow().advances, [f64::from(0.02f32), fixed]);
}
