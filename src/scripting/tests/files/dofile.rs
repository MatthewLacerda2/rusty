//! `dofile` / `loadfile`: relative paths from the project root.

use super::{dir, manager, out, warnings, write};

#[test]
fn dofile_and_loadfile_read_from_the_project_root() {
    let root = dir("dofile");
    write(&root.join("data/level.lua"), "return 3");
    let (m, console) = manager(&root);
    let code = "out = tostring(dofile('data/level.lua') + loadfile('data/level.lua')())";
    assert_eq!(out(&m, code), "6");
    assert!(warnings(&console).is_empty());
}

#[test]
fn an_outside_dofile_or_loadfile_still_runs_but_warns() {
    let root = dir("dofile_root");
    let lib = dir("dofile_lib");
    let file = lib.join("cfg.lua");
    write(&file, "return 5");
    let (m, console) = manager(&root);
    let code = format!(
        "out = tostring(dofile([[{0}]]) + loadfile([[{0}]])())",
        file.display()
    );
    assert_eq!(out(&m, &code), "10");
    let warned = warnings(&console);
    assert_eq!(warned.len(), 1, "one file, one warning: {warned:?}");
    assert!(warned[0].starts_with("[Lua] dofile read"), "{warned:?}");
}
