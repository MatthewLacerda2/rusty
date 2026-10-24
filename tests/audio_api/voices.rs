//! Per-entity voices: `Play`, `Stop`, `SetVolume` / `GetVolume` and `GetSpatial`.

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

/// `SetVolume` is `AudioSource.volume`: it writes the component through the
/// inspector's op (clamped to `[0, 1]`) and the live voice follows (#1368).
#[test]
fn set_volume_writes_the_source_and_retunes_its_voice() {
    let lua = Lua::new();
    let f = fixture();

    lua.scope(|scope| {
        register(&lua, scope, &f);
        let id = f.source_id;
        lua.load(format!("Audio.Play({id}); Audio.SetVolume({id}, 0.5)"))
            .exec()
            .unwrap();
        let (vol, gain): (f32, f32) = lua
            .load(format!(
                "return Audio.GetVolume({id}), (Audio.GetSpatial({id}))"
            ))
            .eval()
            .unwrap();
        assert_eq!((vol, gain), (0.5, 0.5));
        let read = |src: String| lua.load(src).eval::<f32>().unwrap();
        assert_eq!(
            read(format!(
                "Audio.SetVolume({id}, 7); return Audio.GetVolume({id})"
            )),
            1.0
        );
        assert_eq!(read(format!("return Audio.GetVolume({})", f.bare_id)), 0.0);
        Ok(())
    })
    .unwrap();
}
