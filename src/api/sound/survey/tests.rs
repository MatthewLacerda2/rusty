//! Tests for `Sound.Survey` (#380), driven the way a script drives it: a set of
//! documents in, the counts out — and nothing that grades them.

use mlua::{Lua, Table};

use crate::api::sound::bake_tests::{lua_with_sound, tmp};

/// A noise impact under a lowpass at `cutoff` Hz — the one-shot a shooter has
/// hundreds of.
fn impact(cutoff: u32) -> String {
    format!(
        r#"{{"source":{{"kind":"noise"}},"amp":{{"a":0.0,"d":0.08,"s":0.0,"r":0.05}},
            "filter":{{"kind":"lowpass","cutoff":{cutoff}}}}}"#
    )
}

/// A song with a karplus arp held through the bar and a hat written louder but
/// firing for a fifth of each beat — gain alone would crown the hat.
const CUE: &str = r#"{
    bpm = 96,
    tracks = {
        { name = "hat", gain = 0.9, patch = { source = { kind = "noise" },
          amp = { a = 0.0, d = 0.03, s = 0.0, r = 0.02 } } },
        { name = "arp", gain = 0.6, patch = { source = { kind = "karplus" },
          amp = { a = 0.0, d = 0.3, s = 0.4, r = 0.2 } } },
    },
    patterns = { bar = { beats = 4, notes = {
        { track = "hat", note = "C5", start = 0, dur = 0.2 },
        { track = "hat", note = "C5", start = 2, dur = 0.2 },
        { track = "arp", note = "E3", start = 0, dur = 2 },
        { track = "arp", note = "B3", start = 2, dur = 2 },
    } } },
    arrangement = { "bar" },
}"#;

fn survey<'lua>(lua: &'lua Lua, set: &str) -> Table<'lua> {
    lua.load(format!("return Sound.Survey({set})"))
        .eval()
        .expect("the set surveys")
}

fn write(name: &str, json: &str) -> String {
    let path = tmp(name);
    std::fs::write(&path, json).unwrap();
    path
}

#[test]
fn fourteen_impacts_read_as_one_kind_under_one_narrow_cutoff() {
    let paths: Vec<String> = [700, 900, 800]
        .iter()
        .map(|hz| write(&format!("rusty_survey_impact_{hz}.json"), &impact(*hz)))
        .collect();
    let lua = lua_with_sound();
    let report = survey(
        &lua,
        &format!("{{ \"{}\", \"{}\", \"{}\" }}", paths[0], paths[1], paths[2]),
    );
    let patches: Table = report.get("patches").unwrap();
    assert_eq!(patches.raw_len(), 3);
    let first: Table = patches.get(1).unwrap();
    assert_eq!(first.get::<_, String>("name").unwrap(), paths[0]);
    assert_eq!(first.get::<_, String>("source").unwrap(), "noise");
    assert_eq!(first.get::<_, String>("filter").unwrap(), "lowpass");
    assert_eq!(first.get::<_, f32>("cutoff").unwrap(), 700.0);
    assert_eq!(first.get::<_, f32>("sustain").unwrap(), 0.0);

    let rollup: Table = report.get("rollup").unwrap();
    let noise: Table = rollup.get::<_, Table>("sources").unwrap().get(1).unwrap();
    assert_eq!(noise.get::<_, String>("source").unwrap(), "noise");
    assert_eq!(noise.get::<_, usize>("patches").unwrap(), 3);
    let cutoff: Table = noise.get("cutoff").unwrap();
    assert_eq!(cutoff.get::<_, f32>("low").unwrap(), 700.0);
    assert_eq!(cutoff.get::<_, f32>("high").unwrap(), 900.0);
}

#[test]
fn the_loudest_track_is_gain_times_duty_not_the_highest_gain() {
    let lua = lua_with_sound();
    let report = survey(&lua, &format!("{{ {CUE}, Sound.SongToJson({CUE}) }}"));
    let songs: Table = report.get("songs").unwrap();
    assert_eq!(
        songs.raw_len(),
        2,
        "a table and a JSON string are both documents"
    );
    let cue: Table = songs.get(1).unwrap();
    assert_eq!(cue.get::<_, String>("name").unwrap(), "#1");
    assert_eq!(cue.get::<_, String>("loudest").unwrap(), "arp");
    let arp: Table = cue.get::<_, Table>("tracks").unwrap().get(2).unwrap();
    assert_eq!(arp.get::<_, usize>("notes").unwrap(), 2);
    assert_eq!(arp.get::<_, f32>("duty").unwrap(), 1.0);
    assert_eq!(
        arp.get::<_, f32>("median").unwrap(),
        59.0,
        "upper middle: B3"
    );

    let rollup: Table = report.get("rollup").unwrap();
    let karplus: Table = rollup
        .get::<_, Table>("sources")
        .unwrap()
        .sequence_values::<Table>()
        .map(Result::unwrap)
        .find(|row| row.get::<_, String>("source").unwrap() == "karplus")
        .unwrap();
    assert_eq!(karplus.get::<_, usize>("loudest").unwrap(), 2);
    let tempo: Table = rollup.get("tempo").unwrap();
    assert_eq!(tempo.get::<_, f32>("low").unwrap(), 96.0);
}

#[test]
fn a_set_of_one_has_no_rollup() {
    let lua = lua_with_sound();
    let report = survey(&lua, &format!("{{ {CUE} }}"));
    assert_eq!(report.get::<_, Table>("songs").unwrap().raw_len(), 1);
    assert!(report.get::<_, Option<Table>>("rollup").unwrap().is_none());
}

#[test]
fn what_cannot_be_read_is_skipped_rather_than_refusing_the_set() {
    let lua = lua_with_sound();
    let missing = tmp("rusty_survey_no_such_patch.json");
    let song = r#"{ bpm = 120, tracks = { { name = "lead", patch = "nowhere/lead.json" } },
        patterns = { p = { beats = 1, notes = { { track = "lead", note = 60, start = 0, dur = 1 } } } },
        arrangement = { "p" } }"#;
    let report = survey(
        &lua,
        &format!("{{ \"{missing}\", {{ source = 3 }}, {song} }}"),
    );
    let skipped: Table = report.get("skipped").unwrap();
    assert_eq!(skipped.raw_len(), 2);
    let first: Table = skipped.get(1).unwrap();
    assert_eq!(first.get::<_, String>("name").unwrap(), missing);
    assert!(first
        .get::<_, String>("error")
        .unwrap()
        .contains("cannot read"));
    let lead: Table = report
        .get::<_, Table>("songs")
        .unwrap()
        .get::<_, Table>(1)
        .unwrap()
        .get::<_, Table>("tracks")
        .unwrap()
        .get(1)
        .unwrap();
    assert!(lead.get::<_, Option<String>>("source").unwrap().is_none());
    assert_eq!(lead.get::<_, usize>("notes").unwrap(), 1);
}
