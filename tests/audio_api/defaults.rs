//! The project's speed of sound (#1749): `Audio.SetDefaultSpeedOfSound` is the
//! Build Settings window's twin on `Application`, apart from the running value.

use super::{fixture, register};
use mlua::Lua;

#[test]
fn the_default_speed_of_sound_is_the_projects_not_the_running_one() {
    let (lua, f) = (Lua::new(), fixture());
    register(&lua, &f);
    let get = |call: &str| -> f32 { lua.load(format!("return Audio.{call}()")).eval().unwrap() };
    assert_eq!(get("GetDefaultSpeedOfSound"), 343.0);
    lua.load("Audio.SetDefaultSpeedOfSound(40)").exec().unwrap();
    assert_eq!(f.app.borrow().audio_settings().speed_of_sound, 40.0);
    assert_eq!(get("GetDefaultSpeedOfSound"), 40.0);
    assert_eq!(
        get("GetSpeedOfSound"),
        343.0,
        "applied at the next Play or Stop"
    );
    let err = lua
        .load("Audio.SetDefaultSpeedOfSound(0)")
        .exec()
        .unwrap_err();
    assert!(err.to_string().contains("positive"), "{err}");
    assert_eq!(
        get("GetDefaultSpeedOfSound"),
        40.0,
        "a refused value changes nothing"
    );
}
