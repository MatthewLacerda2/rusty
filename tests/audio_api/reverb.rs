//! The reverb-zone verbs (#469): `AudioReverbZone`'s radii, presets and params
//! round-trip through the shared ops, its errors name what is wrong, and
//! `Audio.GetReverbState` reads dry before any zone resolves. The blend itself is
//! covered in-crate on a ticking world.

use super::{fixture, register};
use mlua::Lua;
use rusty::components::ReverbZoneComponent;

#[test]
fn radii_presets_and_params_round_trip() {
    let lua = Lua::new();
    let f = fixture();
    let id = f.bare_id;
    f.scene
        .borrow_mut()
        .world
        .set_reverb_zone(id, Some(ReverbZoneComponent::default()));
    lua.scope(|scope| {
        register(&lua, scope, &f);
        rusty::api::reverb_zone::register(&lua, scope, &f.scene).unwrap();
        lua.globals().set("id", id)?;
        let radii: (f32, f32) = lua
            .load(
                "AudioReverbZone.SetMinDistance(id, 20) \
                   return AudioReverbZone.GetMinDistance(id), AudioReverbZone.GetMaxDistance(id)",
            )
            .eval()?;
        assert_eq!(radii, (20.0, 20.0), "the fade radius grows to keep up");
        let preset: (String, f32) = lua
            .load(
                "AudioReverbZone.SetPreset(id, 'tunnel') \
                   return AudioReverbZone.GetPreset(id), AudioReverbZone.GetParams(id).decay_time",
            )
            .eval()?;
        assert_eq!(preset, ("Tunnel".to_string(), 2.8));
        let custom: (String, f32, f32) = lua
            .load(
                "AudioReverbZone.SetParams(id, { wet = 5 }) \
                   local p = AudioReverbZone.GetParams(id) \
                   return AudioReverbZone.GetPreset(id), p.wet, p.decay_time",
            )
            .eval()?;
        assert_eq!(
            custom,
            ("Custom".to_string(), 1.0, 2.8),
            "clamped, rest kept"
        );
        let names: Vec<String> = lua.load("return AudioReverbZone.GetPresets()").eval()?;
        assert_eq!(names.len(), 6);
        Ok(())
    })
    .unwrap();
}

#[test]
fn errors_name_what_is_wrong_and_the_state_starts_dry() {
    let lua = Lua::new();
    let f = fixture();
    lua.scope(|scope| {
        register(&lua, scope, &f);
        rusty::api::reverb_zone::register(&lua, scope, &f.scene).unwrap();
        lua.globals().set("id", f.bare_id)?;
        let missing: Option<f32> = lua
            .load("return AudioReverbZone.GetMinDistance(id)")
            .eval()?;
        assert_eq!(missing, None);
        let e = lua
            .load("AudioReverbZone.SetPreset(id, 'Hall')")
            .exec()
            .unwrap_err();
        assert!(e.to_string().contains("no AudioReverbZone"), "{e}");
        f.scene
            .borrow_mut()
            .world
            .set_reverb_zone(f.bare_id, Some(ReverbZoneComponent::default()));
        let e = lua
            .load("AudioReverbZone.SetPreset(id, 'Cave')")
            .exec()
            .unwrap_err();
        assert!(e.to_string().contains("Tunnel"), "lists the presets: {e}");
        let e = lua
            .load("AudioReverbZone.SetParams(id, { decay = 2 })")
            .exec()
            .unwrap_err();
        assert!(e.to_string().contains("decay_time"), "lists the keys: {e}");
        let state: (f32, f32, usize) = lua
            .load("local r = Audio.GetReverbState() return r.wet, r.weight, r.zones")
            .eval()?;
        assert_eq!(state, (0.0, 0.0, 0));
        Ok(())
    })
    .unwrap();
}
