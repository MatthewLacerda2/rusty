//! `Sound.BakeSong` against zimmer (#413): the song leg's two edges that rusty
//! owns rather than the synthesiser — resolving a track's patch *by path* (zimmer
//! does no I/O, so rusty supplies the resolver) and writing the mixed WAV.

use mlua::Lua;

fn tmp(name: &str) -> String {
    // Forward slashes: a Windows temp path interpolated into a Lua string literal
    // trips Lua's escape parser (`\U`).
    std::env::temp_dir()
        .join(name)
        .to_str()
        .unwrap()
        .replace('\\', "/")
}

const PLUCK: &str = r#"{
    source = { kind = "karplus" },
    amp = { a = 0.001, d = 0.3, s = 0.0, r = 0.2 },
}"#;

/// A two-pattern song whose one track names `patch` — inline table or path.
fn song(patch: &str) -> String {
    format!(
        r#"{{
          bpm = 120, seed = 2,
          tracks = {{ {{ name = "pluck", patch = {patch} }} }},
          patterns = {{
            a = {{ beats = 1, notes = {{ {{ track = "pluck", note = "E3", start = 0, dur = 0.5 }} }} }},
            b = {{ beats = 1, notes = {{ {{ track = "pluck", note = "G3", start = 0, dur = 0.5 }} }} }},
          }},
          arrangement = {{ "a", "b", "a" }},
        }}"#
    )
}

#[test]
fn a_track_naming_a_patch_file_bakes_the_same_mix_as_the_inline_patch() {
    let lua = Lua::new();
    rusty::api::sound::register(&lua).unwrap();
    let (patch_path, inline, by_path) = (
        tmp("rusty_song_path_pluck.json"),
        tmp("rusty_song_path_inline.wav"),
        tmp("rusty_song_path_named.wav"),
    );
    let saved: String = lua
        .load(format!("return Sound.ToJson({PLUCK})"))
        .eval()
        .unwrap();
    std::fs::write(&patch_path, saved).unwrap();

    let inline_song = song(PLUCK);
    let named_song = song(&format!("\"{patch_path}\""));
    lua.load(format!(
        r#"Sound.BakeSong({inline_song}, "{inline}")
           Sound.BakeSong({named_song}, "{by_path}")"#
    ))
    .exec()
    .expect("both songs bake");

    let (a, b) = (
        std::fs::read(&inline).unwrap(),
        std::fs::read(&by_path).unwrap(),
    );
    assert!(a.len() > 44, "a mix, not an empty header");
    assert_eq!(a, b, "a patch by path is the same instrument as inline");
    for f in [patch_path, inline, by_path] {
        std::fs::remove_file(f).ok();
    }
}

#[test]
fn a_missing_patch_file_is_reported_and_writes_nothing() {
    let lua = Lua::new();
    rusty::api::sound::register(&lua).unwrap();
    let out = tmp("rusty_song_path_missing.wav");
    std::fs::remove_file(&out).ok();
    let broken = song(r#""no/such/patch.json""#);
    let err = lua
        .load(format!(r#"Sound.BakeSong({broken}, "{out}")"#))
        .exec()
        .expect_err("an unreadable patch must fail the bake");
    assert!(err.to_string().contains("no/such/patch.json"), "got {err}");
    assert!(!std::path::Path::new(&out).exists(), "no file on failure");
}
