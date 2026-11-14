//! `Sound.Level(path, { sections = seconds })` (#1579): a file measured on a grid
//! of the caller's own, so a one-second capture reads as rows a scenario can compare.

use mlua::Table;

use super::fixture::lua_with_sound;
use super::level_tests::write_wav;

/// One second at 44.1 kHz, mono: loud for the first half, silent after.
fn half_loud() -> String {
    let mut samples = vec![8_000i16; 22_050];
    samples.extend(vec![0i16; 22_050]);
    write_wav("rusty_level_grid.wav", 1, &samples)
}

#[test]
fn sections_cut_a_file_on_the_grid_asked_for() {
    let lua = lua_with_sound();
    let path = half_loud();
    let level: Table = lua
        .load(format!(
            "return Sound.Level('{path}', {{ sections = 0.25 }})"
        ))
        .eval()
        .unwrap();
    let rows: Vec<Table> = level
        .get::<Table>("sections")
        .unwrap()
        .sequence_values()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows.len(), 4);
    let to: Vec<f64> = rows.iter().map(|r| r.get("to").unwrap()).collect();
    assert_eq!(to, vec![0.25, 0.5, 0.75, 1.0]);
    let silent: Vec<bool> = rows.iter().map(|r| r.get("silent").unwrap()).collect();
    assert_eq!(silent, vec![false, false, true, true]);
    assert!(
        rows[0].get::<Option<String>>("label").unwrap().is_none(),
        "grid rows are unlabelled"
    );
}

#[test]
fn without_sections_the_file_keeps_zimmers_grid() {
    let lua = lua_with_sound();
    let path = half_loud();
    let rows: Table = lua
        .load(format!("return Sound.Level('{path}').sections"))
        .eval()
        .unwrap();
    assert_eq!(
        rows.raw_len(),
        0,
        "one second is one 8-second row: none listed"
    );
}

#[test]
fn a_grid_that_is_not_a_positive_length_is_refused() {
    let lua = lua_with_sound();
    let path = half_loud();
    for bad in ["0", "-1", "0/0", "1e-9"] {
        let call = format!("return Sound.Level('{path}', {{ sections = {bad} }})");
        assert!(lua.load(call).exec().is_err(), "sections = {bad}");
    }
}
