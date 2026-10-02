//! The mixer verbs (#465), driven as a script drives them, read back headlessly.

use super::{fixture, register};
use mlua::Lua;

/// Run `script` against a fresh fixture, then `check` with the Lua state and fixture.
fn run(script: &str, check: impl FnOnce(&Lua, &super::Fixture)) {
    let lua = Lua::new();
    let f = fixture();
    lua.scope(|scope| {
        register(&lua, scope, &f);
        lua.load(script).exec().unwrap();
        check(&lua, &f);
        Ok(())
    })
    .unwrap();
}

fn eval<T: for<'l> mlua::FromLua<'l>>(lua: &Lua, expr: &str) -> T {
    lua.load(format!("return {expr}")).eval().unwrap()
}

#[test]
fn groups_list_create_and_read_back() {
    let script = r#"
        Audio.CreateGroup("Guns", "SFX")
        Audio.SetGroupVolume("SFX", 0.5)
        Audio.SetGroupLowPass("Guns", 800, 0.4)
        Audio.SetGroupReverbSend("Guns", 0.3)
    "#;
    run(script, |lua, _| {
        let names: Vec<String> = eval(lua, "Audio.GetGroups()");
        assert_eq!(
            names,
            ["Master", "Music", "SFX", "Voice", "World", "UI", "Guns"]
        );
        assert_eq!(
            eval::<String>(lua, "Audio.GetGroupState('Guns').parent"),
            "SFX"
        );
        assert_eq!(
            eval::<f32>(lua, "Audio.GetGroupState('Guns').low_pass"),
            800.0
        );
        let res: f32 = eval(lua, "Audio.GetGroupState('Guns').low_pass_resonance");
        assert!((res - 0.4).abs() < 1e-6);
        let eff: f32 = eval(lua, "Audio.GetGroupState('Guns').effective_volume");
        assert_eq!(eff, 0.5, "follows its parent");
        assert!(eval::<Option<String>>(lua, "Audio.GetGroupState('Master').parent").is_none());
    });
}

#[test]
fn unknown_names_are_errors_that_list_what_exists() {
    run("", |lua, _| {
        let e = lua
            .load("Audio.SetGroupVolume('Musik', 1)")
            .exec()
            .unwrap_err();
        assert!(e.to_string().contains("Music"), "{e}");
        let bad_field = "Audio.DefineSnapshot('X', { World = { lowpass = 1 } })";
        assert!(lua.load(bad_field).exec().is_err());
        assert!(lua
            .load("Audio.TransitionToSnapshot('Nope', 1)")
            .exec()
            .is_err());
        assert!(
            lua.load("Audio.CreateGroup('Music')").exec().is_err(),
            "taken"
        );
    });
}

#[test]
fn a_snapshot_blends_on_unscaled_sim_time() {
    let script = r#"
        Audio.DefineSnapshot("BulletTime", { World = { volume = 0.5, low_pass = 900 } })
        Audio.TransitionToSnapshot("BulletTime", 1)
    "#;
    run(script, |lua, f| {
        f.time.borrow_mut().advance(0.5);
        let now = f.time.borrow().unscaled_time;
        f.audio.borrow_mut().advance_mixer(now);
        assert_eq!(
            eval::<f32>(lua, "Audio.GetGroupState('World').volume"),
            0.75
        );
        let (name, t): (String, f32) = lua.load("return Audio.GetSnapshot()").eval().unwrap();
        assert_eq!((name.as_str(), t), ("BulletTime", 0.5));
        assert_eq!(eval::<f32>(lua, "Audio.GetGroupState('Music').volume"), 1.0);
    });
}

#[test]
fn a_source_routes_through_its_output_group_and_ducks_music() {
    run("", |lua, f| {
        let id = f.source_id;
        lua.load(format!("Audio.SetOutputGroup({id}, 'Voice')"))
            .exec()
            .unwrap();
        assert_eq!(
            eval::<String>(lua, &format!("Audio.GetOutputGroup({id})")),
            "Voice"
        );
        let bare = f.bare_id;
        assert!(lua
            .load(format!("Audio.SetOutputGroup({bare}, 'Voice')"))
            .exec()
            .is_err());
        let script = format!("Audio.AddDuck('Voice', 'Music', 0.25, 0, 0) Audio.Play({id})");
        lua.load(script).exec().unwrap();
        f.audio.borrow_mut().advance_mixer(0.1);
        assert_eq!(eval::<f32>(lua, "Audio.GetGroupState('Music').duck"), 0.25);
        lua.load(format!("Audio.Stop({id}) Audio.ClearDucks()"))
            .exec()
            .unwrap();
        assert_eq!(eval::<f32>(lua, "Audio.GetGroupState('Music').duck"), 1.0);
    });
}
