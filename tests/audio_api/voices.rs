//! Per-entity voices: `Play`, `Stop`, `SetVolume` and `GetSpatial`.

use super::{fixture, register};
use mlua::Lua;

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
