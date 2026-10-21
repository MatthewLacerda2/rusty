//! A voice on a child is heard and logged at the child's **world** position (#1196):
//! a 3D source at local origin under a parent off to the listener's side
//! is panned and attenuated, and `Play` logs where it stands in the world.

use glam::Vec3;
use mlua::Lua;
use rusty::components::AudioSourceComponent;

use super::{fixture, register};

#[test]
fn a_parented_source_is_heard_and_logged_at_its_world_position() {
    let lua = Lua::new();
    let f = fixture();
    let child = {
        let mut scene = f.scene.borrow_mut();
        let parent = scene.add_entity("Pivot".to_string());
        scene.world.transform_mut(parent).unwrap().position = Vec3::new(10.0, 0.0, 10.0);
        let child = scene.add_entity("Speaker".to_string());
        scene.set_parent(child, Some(parent)).unwrap();
        let source = AudioSourceComponent {
            clip: "loop.ogg".to_string(),
            spatial_blend: 1.0,
            final_distance: 50.0,
            ..Default::default()
        };
        scene.world.set_audio(child, Some(source));
        child
    };
    lua.scope(|scope| {
        register(&lua, scope, &f);
        lua.load(format!("Audio.Play({child})")).exec().unwrap();
        let (gain, pan, playing): (f32, f32, bool) = lua
            .load(format!("return Audio.GetSpatial({child})"))
            .eval()
            .unwrap();
        assert!(playing);
        assert!(pan.abs() > 0.3, "off to the side, panned: {pan}");
        assert!(gain < 1.0, "far away, attenuated: {gain}");
        Ok(())
    })
    .unwrap();
    let audio = f.audio.borrow();
    let logged = audio.events().last().expect("Play is logged").position;
    assert_eq!(logged, [10.0, 0.0, 10.0]);
}
