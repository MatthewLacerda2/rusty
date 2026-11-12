use std::path::PathBuf;

use mlua::{Lua, Table};

use super::refresh_table;
use crate::asset::audio::Refresh;

/// A path a script gets back is the path as the filesystem spells it: `/`
/// separates, and a `\` is part of a name (#1390).
#[test]
fn refresh_returns_paths_to_lua_as_spelled() {
    let report = Refresh {
        converted: vec![(
            PathBuf::from(r"a\x.mp3"),
            PathBuf::from(r"assets\sfx\x.wav"),
        )],
        skipped: vec![(PathBuf::from(r"assets\y.mp3"), "corrupt".into())],
    };
    let lua = Lua::new();
    let t = refresh_table(&lua, &report).unwrap();
    let converted: Table = t.get("converted").unwrap();
    assert_eq!(converted.get::<String>(1).unwrap(), r"assets\sfx\x.wav");
    let skipped: Table = t.get("skipped").unwrap();
    let first: Table = skipped.get(1).unwrap();
    assert_eq!(first.get::<String>("path").unwrap(), r"assets\y.mp3");
}
