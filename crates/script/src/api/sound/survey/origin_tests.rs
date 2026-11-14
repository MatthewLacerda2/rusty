//! `Sound.Survey` reads a file as a bake reads it and an inline document as a
//! verb reads it (#1216): a miscased enum name is fine inline, an error in a file.

use mlua::Table;

use crate::api::sound::fixture::{lua_with_sound, tmp};

const MISCASED: &str = r#"{"source":{"kind":"NOISE"},"amp":{"a":0.0,"d":0.1,"s":0.0,"r":0.1}}"#;

#[test]
fn a_miscased_name_reads_inline_but_not_from_a_file() {
    let lua = lua_with_sound();
    let path = tmp("rusty_survey_miscased.json");
    std::fs::write(&path, MISCASED).unwrap();
    let report: Table = lua
        .load(format!("return Sound.Survey({{ '{MISCASED}', {path:?} }})"))
        .eval()
        .expect("the set surveys");
    let patches: Table = report.get("patches").unwrap();
    assert_eq!(patches.raw_len(), 1);
    let inline: Table = patches.get(1).unwrap();
    assert_eq!(inline.get::<String>("name").unwrap(), "#1");
    assert_eq!(inline.get::<String>("source").unwrap(), "noise");
    let skipped: Table = report.get::<Table>("skipped").unwrap().get(1).unwrap();
    assert_eq!(skipped.get::<String>("name").unwrap(), path);
    let error = skipped.get::<String>("error").unwrap();
    assert!(error.contains("unknown variant `NOISE`"), "{error}");
}
