//! The group-effect verbs (#1748), as a script drives them: set and read back a
//! chain, Master's default limiter, errors reaching Lua, and a snapshot gliding
//! an effect's parameter.

use super::{fixture, register};
use mlua::Lua;

fn run(script: &str, check: impl FnOnce(&Lua, &super::Fixture)) {
    let lua = Lua::new();
    let f = fixture();
    register(&lua, &f);
    lua.load(script).exec().unwrap();
    check(&lua, &f);
}

fn eval<T: mlua::FromLua>(lua: &Lua, expr: &str) -> T {
    lua.load(format!("return {expr}")).eval().unwrap()
}

#[test]
fn a_chain_is_set_and_read_back_in_order_with_its_defaults() {
    let script = r#"
        Audio.SetGroupEffects("SFX", {
            { kind = "compressor", threshold = -18, sidechain = "Voice" },
            { kind = "eq", bands = { { shape = "peak", frequency = 2500, gain = 4 } } },
            { kind = "distortion", mode = "hard", drive = 0.2 },
            { kind = "delay", delay = 90, wet = 0.3, low_pass = 4000 },
        })
    "#;
    run(script, |lua, _| {
        assert_eq!(eval::<usize>(lua, "#Audio.GetGroupEffects('SFX')"), 4);
        let fx = "Audio.GetGroupEffects('SFX')";
        assert_eq!(eval::<String>(lua, &format!("{fx}[1].kind")), "compressor");
        assert_eq!(eval::<f32>(lua, &format!("{fx}[1].threshold")), -18.0);
        assert_eq!(eval::<f32>(lua, &format!("{fx}[1].ratio")), 4.0, "default");
        assert_eq!(eval::<String>(lua, &format!("{fx}[1].sidechain")), "Voice");
        assert_eq!(eval::<f32>(lua, &format!("{fx}[2].bands[1].gain")), 4.0);
        assert_eq!(
            eval::<String>(lua, &format!("{fx}[2].bands[2].shape")),
            "off"
        );
        assert_eq!(eval::<String>(lua, &format!("{fx}[3].mode")), "hard");
        assert_eq!(eval::<f32>(lua, &format!("{fx}[4].delay")), 90.0);
        assert!(eval::<Option<String>>(lua, "Audio.GetGroupEffects('Music')[1]").is_none());
    });
}

#[test]
fn master_has_a_limiter_until_a_game_removes_it() {
    run("", |lua, _| {
        let master = "Audio.GetGroupEffects('Master')[1]";
        assert_eq!(eval::<String>(lua, &format!("{master}.kind")), "limiter");
        let ceiling: f32 = eval(lua, &format!("{master}.ceiling"));
        assert!((ceiling - -0.3).abs() < 1e-6);
        lua.load("Audio.SetGroupEffects('Master', {})")
            .exec()
            .unwrap();
        assert_eq!(eval::<usize>(lua, "#Audio.GetGroupEffects('Master')"), 0);
    });
}

#[test]
fn a_bad_kind_field_value_or_group_is_a_lua_error() {
    run("", |lua, _| {
        for (script, says) in [
            ("{ { kind = 'chorus' } }", "kinds:"),
            ("{ { kind = 'limiter', celing = -1 } }", "ceiling"),
            ("{ { kind = 'limiter', ceiling = 6 } }", "out of range"),
            ("{ { kind = 'compressor', sidechain = 'Nope' } }", "Music"),
            ("{ { kind = 'delay', wet = true } }", "boolean"),
        ] {
            let e = lua
                .load(format!("Audio.SetGroupEffects('SFX', {script})"))
                .exec()
                .unwrap_err();
            assert!(e.to_string().contains(says), "{script}: {e}");
        }
        assert!(lua.load("Audio.GetGroupEffects('Sfx')").exec().is_err());
    });
}

#[test]
fn a_snapshot_glides_an_effect_parameter_on_unscaled_time() {
    let script = r#"
        Audio.SetGroupEffects("Music", { { kind = "compressor", threshold = -10 } })
        Audio.DefineSnapshot("Combat", { Music = { effects = { { kind = "compressor", threshold = -30 } } } })
        Audio.TransitionToSnapshot("Combat", 1)
    "#;
    run(script, |lua, f| {
        f.time.borrow_mut().advance(0.5);
        let now = f.time.borrow().unscaled_time;
        f.audio.borrow_mut().advance_mixer(now);
        let at: f32 = eval(lua, "Audio.GetGroupEffects('Music')[1].threshold");
        assert_eq!(at, -20.0);
        let wrong = "Audio.DefineSnapshot('X', { Music = { effects = { { kind = 'eq' } } } })";
        assert!(lua.load(wrong).exec().is_err(), "the kind must match");
    });
}
