//! The occlusion verbs (#467): global settings, the per-source opt-out, and the
//! factor `GetSpatial` reads back. The casts themselves are covered in-crate
//! against the live physics world.

use super::{fixture, register};
use mlua::Lua;

#[test]
fn settings_patch_round_trip_and_reject_unknown_keys() {
    let lua = Lua::new();
    let f = fixture();
    lua.scope(|scope| {
        register(&lua, scope, &f);
        let read = "local s = Audio.GetOcclusionSettings() \
                    return s.strength, s.layer_mask, s.voices_per_tick";
        let defaults: (f32, u32, u32) = lua.load(read).eval().unwrap();
        assert_eq!(defaults, (1.0, u32::MAX, 16));
        lua.load("Audio.SetOcclusionSettings({ strength = 0.5, layer_mask = ~(1 << 8) })")
            .exec()
            .unwrap();
        let set: (f32, u32, u32) = lua.load(read).eval().unwrap();
        assert_eq!(set, (0.5, !(1 << 8), 16), "unnamed fields kept");
        let e = lua
            .load("Audio.SetOcclusionSettings({ strenght = 1 })")
            .exec()
            .unwrap_err();
        assert!(e.to_string().contains("strength"), "{e}");
        Ok(())
    })
    .unwrap();
}

#[test]
fn the_opt_out_round_trips_and_get_spatial_reports_occlusion() {
    let lua = Lua::new();
    let f = fixture();
    lua.scope(|scope| {
        register(&lua, scope, &f);
        let id = f.source_id;
        let on: bool = lua
            .load(format!("return Audio.GetOcclusionEnabled({id})"))
            .eval()
            .unwrap();
        assert!(on, "occluded by default");
        lua.load(format!("Audio.SetOcclusionEnabled({id}, false)"))
            .exec()
            .unwrap();
        assert!(!f.scene.borrow().world.audio(id).unwrap().occlusion_enabled);
        let bare = format!("Audio.SetOcclusionEnabled({}, true)", f.bare_id);
        assert!(lua.load(bare).exec().is_err(), "no AudioSource");
        lua.load(format!("Audio.Play({id})")).exec().unwrap();
        let (_, _, playing, occlusion): (f32, f32, bool, f32) = lua
            .load(format!("return Audio.GetSpatial({id})"))
            .eval()
            .unwrap();
        assert!(playing);
        assert_eq!(occlusion, 0.0, "a 2D source is never occluded");
        Ok(())
    })
    .unwrap();
}
