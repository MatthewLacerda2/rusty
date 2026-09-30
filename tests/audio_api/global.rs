//! Entity-less verbs: the master volume and `PlayAt` one-shots.

use super::{fixture, register};
use mlua::Lua;

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
