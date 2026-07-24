//! Integration coverage for the `Sound` scripting namespace (#357): an agent
//! authors a patch, bakes a one-shot to `.wav`, and fires that very path through
//! `Audio.PlayAt` — the wired-up day-one consumer. The audio backend is the
//! `NullBackend` here, exactly as on the headless harness, so the assertion is on
//! the maestro's introspection log: *what* played, and that it was the file the
//! agent had just invented.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::audio::{AudioEventKind, AudioMaestro};
use rusty::render::Camera;
use rusty::scene::Scene;
use rusty::time::Time;

/// A gunshot: a noise burst through a fast, resonant lowpass sweep — the patch
/// shape the north-star shooter fires thousands of times.
const GUNSHOT: &str = r#"{
    source = { kind = "noise" },
    amp = { a = 0.0, d = 0.09, s = 0.0, r = 0.06 },
    filter = { kind = "lowpass", cutoff = 300, resonance = 0.5, env_amount = 6000,
               adsr = { a = 0.0, d = 0.05, s = 0.0, r = 0.05 } },
    fx = { { fx = "reverb", size = 0.4, damp = 0.6, mix = 0.15 } },
}"#;

fn tmp(name: &str) -> String {
    // Forward slashes: a Windows temp path (`C:\Users\…`) interpolated into a Lua
    // string literal trips Lua's escape parser (`\U`).
    std::env::temp_dir()
        .join(name)
        .to_str()
        .unwrap()
        .replace('\\', "/")
}

#[test]
fn a_baked_one_shot_plays_through_the_audio_namespace() {
    let lua = Lua::new();
    let path = tmp("rusty_sound_e2e_gunshot.wav");
    let scene = RefCell::new(Scene::new());
    let audio = RefCell::new(AudioMaestro::default());
    let time = RefCell::new(Time::default());
    let camera = RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0));

    rusty::api::sound::register(&lua).unwrap();
    lua.scope(|s| {
        rusty::api::audio::register(&lua, s, &scene, &audio, &time, &camera).unwrap();
        let script = format!(
            r#"
            local clip = Sound.Bake({GUNSHOT}, "C2", "{path}", {{ duration = 0.12, seed = 9 }})
            return Audio.PlayAt(clip, 1.0, 0.0, -3.0, 0.9)
        "#
        );
        let played: bool = lua.load(&script).eval().unwrap();
        assert!(played, "the maestro accepted the baked one-shot");
        Ok(())
    })
    .unwrap();

    let maestro = audio.borrow();
    let event = maestro.events().last().expect("the play was logged");
    assert_eq!(event.kind, AudioEventKind::PlayAt);
    assert_eq!(
        event.clip, path,
        "the log names the file the agent just baked"
    );
    assert_eq!(event.position, [1.0, 0.0, -3.0]);
    std::fs::remove_file(path).ok();
}

#[test]
fn a_saved_patch_re_bakes_byte_for_byte() {
    // The workflow a project actually uses: author once, save the patch as `.json`,
    // and re-bake the asset from that file on any later run.
    let lua = Lua::new();
    let authored = tmp("rusty_sound_e2e_authored.wav");
    let rebaked = tmp("rusty_sound_e2e_rebaked.wav");
    let patch_json = tmp("rusty_sound_e2e_patch.json");

    rusty::api::sound::register(&lua).unwrap();
    lua.globals()
        .set("PATCH_PATH", patch_json.as_str())
        .unwrap();
    let script = format!(
        r#"
        local patch = {GUNSHOT}
        Sound.Bake(patch, "C2", "{authored}", {{ seed = 4 }})
        local file = io.open(PATCH_PATH, "w")
        file:write(Sound.ToJson(patch))
        file:close()
    "#
    );
    lua.load(&script).exec().unwrap();

    let saved = std::fs::read_to_string(&patch_json).expect("the patch was saved");
    let opts = rusty::soundgen::NoteOpts {
        seed: 4,
        ..Default::default()
    };
    let patch = rusty::soundgen::Patch::from_json(&saved).expect("the saved patch parses");
    rusty::soundgen::bake_note(&patch, 36.0, &opts, &rebaked).expect("re-bake");

    assert_eq!(
        std::fs::read(&authored).unwrap(),
        std::fs::read(&rebaked).unwrap(),
        "a saved patch must re-bake the identical asset"
    );
    for f in [authored, rebaked, patch_json] {
        std::fs::remove_file(f).ok();
    }
}
