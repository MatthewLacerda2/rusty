//! Integration coverage for the `Audio` scripting namespace (issue #350), on the
//! `NullBackend` (`AudioMaestro::default()`): the null device accepts voices, so
//! play/stop/volume bookkeeping and the `GetSpatial` introspection assert
//! identically with or without hardware — exactly what the harness runs on.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::audio::AudioMaestro;
use rusty::components::AudioSourceComponent;
use rusty::render::Camera;
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
    scene.get_entity_mut(source_id).unwrap().audio = Some(AudioSourceComponent {
        clip: "sounds/shot.ogg".to_string(),
        volume: 0.8,
        ..Default::default()
    });
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

#[test]
fn play_reports_whether_the_entity_has_a_source() {
    let lua = Lua::new();
    let f = fixture();

    lua.scope(|scope| {
        register(&lua, scope, &f);
        let played: bool = lua
            .load(format!("return Audio.Play({})", f.source_id))
            .eval()
            .unwrap();
        assert!(played, "an entity with an AudioSource starts a voice");
        for silent in [f.bare_id, 999] {
            let played: bool = lua
                .load(format!("return Audio.Play({silent})"))
                .eval()
                .unwrap();
            assert!(!played, "no source, no voice");
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn spatial_state_tracks_play_and_stop() {
    let lua = Lua::new();
    let f = fixture();

    lua.scope(|scope| {
        register(&lua, scope, &f);
        let id = f.source_id;

        lua.load(format!("Audio.Play({id})")).exec().unwrap();
        let (gain, pan, playing): (f32, f32, bool) = lua
            .load(format!("return Audio.GetSpatial({id})"))
            .eval()
            .unwrap();
        assert!(playing);
        assert_eq!(gain, 0.8, "a fully-2D source is heard at its own volume");
        assert_eq!(pan, 0.0, "a fully-2D source is centred");

        lua.load(format!("Audio.SetVolume({id}, 0.25); Audio.Stop({id})"))
            .exec()
            .unwrap();
        let (_, _, playing): (f32, f32, bool) = lua
            .load(format!("return Audio.GetSpatial({id})"))
            .eval()
            .unwrap();
        assert!(!playing, "Stop releases the voice");

        // No source at all: silent, centred, not playing.
        let spatial: (f32, f32, bool) = lua
            .load(format!("return Audio.GetSpatial({})", f.bare_id))
            .eval()
            .unwrap();
        assert_eq!(spatial, (0.0, 0.0, false));
        Ok(())
    })
    .unwrap();
}

#[test]
fn master_volume_round_trips_and_clamps() {
    let lua = Lua::new();
    let f = fixture();

    lua.scope(|scope| {
        register(&lua, scope, &f);
        // In-range writes round-trip; out-of-range writes clamp to [0, 1].
        for (set, expect) in [(0.5, 0.5), (5.0, 1.0), (-2.0, 0.0)] {
            lua.load(format!("Audio.SetMasterVolume({set})"))
                .exec()
                .unwrap();
            let v: f32 = lua.load("return Audio.GetMasterVolume()").eval().unwrap();
            assert_eq!(v, expect);
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn play_at_fires_a_oneshot_with_optional_volume() {
    let lua = Lua::new();
    let f = fixture();

    lua.scope(|scope| {
        register(&lua, scope, &f);
        let played: bool = lua
            .load("return Audio.PlayAt('sounds/boom.ogg', 1, 2, 3)")
            .eval()
            .unwrap();
        assert!(played, "the null backend accepts the one-shot");
        let played: bool = lua
            .load("return Audio.PlayAt('sounds/boom.ogg', 1, 2, 3, 0.4)")
            .eval()
            .unwrap();
        assert!(played, "explicit volume form");
        Ok(())
    })
    .unwrap();
}
