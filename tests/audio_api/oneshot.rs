//! `Audio.PlayAt`'s optional rolloff band (#575): both distances or neither, and a
//! band that makes sense. (The gain it produces is asserted on the recording backend
//! in `src/audio/oneshot_tests.rs`.)

use super::{fixture, register};
use mlua::Lua;

#[test]
fn play_at_takes_an_optional_rolloff_band() {
    let lua = Lua::new();
    let f = fixture();

    lua.scope(|scope| {
        register(&lua, scope, &f);
        for call in [
            "Audio.PlayAt('boom.wav', 30, 0, 0)",
            "Audio.PlayAt('boom.wav', 30, 0, 0, 0.9, 2, 80)",
            "Audio.PlayAt('boom.wav', 30, 0, 0, nil, 0, 0)",
        ] {
            let played: bool = lua.load(format!("return {call}")).eval().unwrap();
            assert!(played, "{call}");
        }
        assert_eq!(f.audio.borrow().events().len(), 3);
        Ok(())
    })
    .unwrap();
}

#[test]
fn play_at_rejects_a_half_or_inverted_band() {
    let lua = Lua::new();
    let f = fixture();

    lua.scope(|scope| {
        register(&lua, scope, &f);
        for call in [
            "Audio.PlayAt('boom.wav', 0, 0, 0, 1, 5)",
            "Audio.PlayAt('boom.wav', 0, 0, 0, 1, nil, 50)",
            "Audio.PlayAt('boom.wav', 0, 0, 0, 1, 20, 10)",
            "Audio.PlayAt('boom.wav', 0, 0, 0, 1, -1, 10)",
            "Audio.PlayAt('boom.wav', 0, 0, 0, 1, 1, math.huge)",
        ] {
            let err = lua.load(call).exec().unwrap_err().to_string();
            assert!(err.contains("Audio.PlayAt"), "{call}: {err}");
        }
        assert!(f.audio.borrow().events().is_empty(), "nothing fired");
        Ok(())
    })
    .unwrap();
}
