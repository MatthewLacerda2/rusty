//! Tests for the `Sound` namespace, driven the way a script drives it: author a
//! patch as a Lua table, bake a note, check the file that came out.

use mlua::Lua;
use rodio::Source as _;

use super::*;

/// A temp path with forward slashes: a Windows path (`C:\Users\…`) interpolated
/// into a Lua string literal trips Lua's escape parser (`\U`), and Windows accepts
/// `/` for file I/O, so the round-trip matches on every platform.
fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_str()
        .expect("utf-8 temp path")
        .replace('\\', "/")
}

/// A percussive noise patch — the shape every impact / footstep SFX takes.
const IMPACT_PATCH: &str = r#"{
    source = { kind = "noise" },
    amp = { a = 0.001, d = 0.08, s = 0.0, r = 0.05 },
    filter = { kind = "lowpass", cutoff = 400, resonance = 0.3, env_amount = 4000,
               adsr = { a = 0.0, d = 0.06, s = 0.0, r = 0.05 } },
}"#;

fn lua_with_sound() -> Lua {
    let lua = Lua::new();
    register(&lua).expect("the namespace registers");
    lua
}

#[test]
fn bake_writes_a_playable_wav_and_returns_its_path() {
    let lua = lua_with_sound();
    let path = tmp("rusty_sound_api_impact.wav");
    lua.globals().set("OUT", path.as_str()).unwrap();
    let returned: String = lua
        .load(format!(
            "return Sound.Bake({IMPACT_PATCH}, \"C2\", OUT, {{ duration = 0.15, seed = 7 }})"
        ))
        .eval()
        .expect("the bake runs");
    assert_eq!(returned, path);

    // The engine's own decoder must accept what we wrote — that is what "playable"
    // means here, not just "the bytes exist".
    let mut cache = crate::audio::decode::ClipCache::new();
    let clip = cache
        .get_or_decode(&path)
        .expect("the engine decodes the bake");
    assert_eq!(clip.source().channels(), 1, "mono");
    assert_eq!(clip.source().sample_rate(), crate::soundgen::SAMPLE_RATE);
    std::fs::remove_file(path).ok();
}

#[test]
fn a_named_note_and_a_midi_number_are_the_same_note() {
    let lua = lua_with_sound();
    let named = tmp("rusty_sound_api_named.wav");
    let numeric = tmp("rusty_sound_api_numeric.wav");
    lua.load(format!(
        r#"
        Sound.Bake({IMPACT_PATCH}, "A4", "{named}")
        Sound.Bake({IMPACT_PATCH}, 69, "{numeric}")
    "#
    ))
    .exec()
    .expect("both bakes run");
    assert_eq!(
        std::fs::read(&named).unwrap(),
        std::fs::read(&numeric).unwrap()
    );
    std::fs::remove_file(named).ok();
    std::fs::remove_file(numeric).ok();
}

#[test]
fn bake_json_matches_bake_table() {
    let lua = lua_with_sound();
    let from_table = tmp("rusty_sound_table.wav");
    let from_json = tmp("rusty_sound_json.wav");
    lua.load(format!(
        r#"
        local patch = {IMPACT_PATCH}
        Sound.Bake(patch, "C4", "{from_table}", {{ seed = 5 }})
        Sound.BakeJson(Sound.ToJson(patch), "C4", "{from_json}", {{ seed = 5 }})
    "#
    ))
    .exec()
    .expect("both bakes run");
    assert_eq!(
        std::fs::read(&from_table).unwrap(),
        std::fs::read(&from_json).unwrap(),
        "a table bake and the JSON re-bake must be byte-identical"
    );
    std::fs::remove_file(from_table).ok();
    std::fs::remove_file(from_json).ok();
}

#[test]
fn opts_are_optional_and_change_the_rendered_note() {
    let lua = lua_with_sound();
    let short = tmp("rusty_sound_short.wav");
    let long = tmp("rusty_sound_long.wav");
    lua.load(format!(
        r#"
        Sound.Bake({IMPACT_PATCH}, "C4", "{short}")
        Sound.Bake({IMPACT_PATCH}, "C4", "{long}", {{ duration = 2.0 }})
    "#
    ))
    .exec()
    .expect("both bakes run");
    let (a, b) = (
        std::fs::metadata(&short).unwrap().len(),
        std::fs::metadata(&long).unwrap().len(),
    );
    assert!(b > a * 2, "a 2 s note is far longer than the 0.5 s default");
    std::fs::remove_file(short).ok();
    std::fs::remove_file(long).ok();
}

#[test]
fn a_bad_patch_note_or_option_surfaces_the_reason_to_the_script() {
    let lua = lua_with_sound();
    let path = tmp("rusty_sound_rejected.wav");
    std::fs::remove_file(&path).ok();
    let calls = [
        format!(r#"Sound.Bake({{ source = {{ kind = "theremin" }} }}, "C4", "{path}")"#),
        format!(r#"Sound.Bake({IMPACT_PATCH}, "H4", "{path}")"#),
        format!(r#"Sound.Bake({IMPACT_PATCH}, "C4", "{path}", {{ duration = -1 }})"#),
        format!(r#"Sound.BakeJson("not json", "C4", "{path}")"#),
    ];
    for call in calls {
        let err = lua.load(&call).exec().expect_err("must be rejected");
        assert!(!err.to_string().is_empty(), "the reason reaches Lua");
    }
    assert!(
        !std::path::Path::new(&path).exists(),
        "a rejected bake writes no file"
    );
}

#[test]
fn to_json_round_trips_back_to_a_patch() {
    let lua = lua_with_sound();
    let json: String = lua
        .load(format!("return Sound.ToJson({IMPACT_PATCH})"))
        .eval()
        .expect("serializes");
    let patch = Patch::from_json(&json).expect("json parses");
    assert_eq!(patch.filter.expect("filter survived").cutoff, 400.0);
}
