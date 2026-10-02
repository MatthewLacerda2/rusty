//! Integration coverage for the `Audio` scripting namespace (issue #350), on the
//! `NullBackend` (`AudioMaestro::default()`): the null device accepts voices, so
//! play/stop/volume bookkeeping and the `GetSpatial` introspection assert
//! identically with or without hardware — exactly what the harness runs on.

mod global;
mod mixer;
mod occlusion;
mod oneshot;
mod reverb;
mod voices;

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::audio::AudioMaestro;
use rusty::components::AudioSourceComponent;
use rusty::scene::Camera;
use rusty::scene::Scene;
use rusty::time::Time;

struct Fixture {
    scene: RefCell<Scene>,
    audio: RefCell<AudioMaestro>,
    time: RefCell<Time>,
    camera: RefCell<Camera>,
    source_id: u32,
    bare_id: u32,
}

/// One entity with a 2D source (volume 0.8) and one entity with no source; the
/// listener camera sits at the origin.
fn fixture() -> Fixture {
    let mut scene = Scene::new();
    let source_id = scene.add_entity("Shooter".to_string());
    scene.world.set_audio(
        source_id,
        Some(AudioSourceComponent {
            clip: "sounds/shot.ogg".to_string(),
            volume: 0.8,
            ..Default::default()
        }),
    );
    let bare_id = scene.add_entity("Silent".to_string());
    Fixture {
        scene: RefCell::new(scene),
        audio: RefCell::new(AudioMaestro::default()),
        time: RefCell::new(Time::default()),
        camera: RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0)),
        source_id,
        bare_id,
    }
}

fn register<'lua, 'scope>(lua: &'lua Lua, scope: &mlua::Scope<'lua, 'scope>, f: &'scope Fixture) {
    rusty::api::audio::register(lua, scope, &f.scene, &f.audio, &f.time, &f.camera).unwrap();
}
