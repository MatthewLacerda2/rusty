use std::path::PathBuf;

use mlua::{Lua, Table};

use super::refresh_table;
use rusty_asset::audio::Refresh;

// On macOS and Linux `\` is part of a file name, not a separator, so it reaches Lua as is.
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
