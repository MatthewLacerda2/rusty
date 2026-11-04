//! Integration coverage for the `Audio` scripting namespace (issue #350), on the
//! `NullBackend` (`AudioMaestro::default()`): the null device accepts voices, so
//! play/stop/volume bookkeeping and the `GetSpatial` introspection assert
//! identically with or without hardware — exactly what the harness runs on.

mod defaults;
mod global;
mod mixer;
mod occlusion;
mod oneshot;
mod parented;
mod reverb;
mod voices;

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;
use rusty::audio::AudioMaestro;
use rusty::components::AudioSourceComponent;
use rusty::core::application::Application;
use rusty::scene::Camera;
use rusty::scene::Scene;
use rusty::time::Time;

struct Fixture {
    scene: Rc<RefCell<Scene>>,
    audio: Rc<RefCell<AudioMaestro>>,
    time: Rc<RefCell<Time>>,
    camera: Rc<RefCell<Camera>>,
    app: Rc<RefCell<Application>>,
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
        scene: Rc::new(RefCell::new(scene)),
        audio: Rc::new(RefCell::new(AudioMaestro::default())),
        time: Rc::new(RefCell::new(Time::default())),
        camera: Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0))),
        app: Rc::new(RefCell::new(Application::new())),
        source_id,
        bare_id,
    }
}

fn register(lua: &Lua, f: &Fixture) {
    use rusty::api::bind;
    let (scene, audio) = (bind(lua, &f.scene), bind(lua, &f.audio));
    let (time, camera, app) = (bind(lua, &f.time), bind(lua, &f.camera), bind(lua, &f.app));
    rusty::api::audio::register(lua, scene, audio, time, camera, app).unwrap();
}
