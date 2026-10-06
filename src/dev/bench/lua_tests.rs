//! `LoadStress`'s options table (#889): every key lands on its own [`StressSpec`]
//! field, so deleting one arm of the match is an "unknown option" error here.

use mlua::{Lua, Table};

use super::spec_from_lua;
use crate::dev::bench::stress::StressSpec;

fn spec(lua: &Lua, src: &str) -> mlua::Result<StressSpec> {
    let opts: Table = lua.load(src).eval()?;
    spec_from_lua(lua, Some(opts))
}

#[test]
fn every_load_stress_option_sets_its_own_field() {
    let lua = Lua::new();
    let got = spec(
        &lua,
        r#"return {
            enemies = 1, lights = 2, flickering = 3, shadowed = 4, decals = 5,
            particle_systems = 6, enemy_script = "e.lua", flicker_script = "f.lua",
        }"#,
    )
    .unwrap();
    let want = StressSpec {
        enemies: 1,
        lights: 2,
        flickering: 3,
        shadowed: 4,
        decals: 5,
        particle_systems: 6,
        enemy_script: "e.lua".to_string(),
        flicker_script: "f.lua".to_string(),
    };
    assert_eq!(got, want);
}

#[test]
fn no_options_is_the_default_scene_and_a_script_must_be_a_string() {
    let lua = Lua::new();
    assert_eq!(spec_from_lua(&lua, None).unwrap(), StressSpec::default());
    let err = spec(&lua, "return { enemy_script = 3 }").unwrap_err();
    assert!(err.to_string().contains("expected a path string"), "{err}");
}
