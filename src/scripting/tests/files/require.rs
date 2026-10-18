//! `require`: project, then engine, then `package.path`.

use super::{dir, manager, out, warnings, write};

#[test]
fn require_finds_project_modules_first_without_warning() {
    let root = dir("project");
    write(
        &root.join("assets/scripts/util/math2.lua"),
        "return { v = 'project' }",
    );
    write(
        &root.join("assets/scripts/pkg/init.lua"),
        "return { v = 'init' }",
    );
    // A project copy of an engine helper wins over the bundled one.
    write(
        &root.join("assets/scripts/ui/range_axis.lua"),
        "return { v = 'mine' }",
    );
    let (m, console) = manager(&root);
    let code = "out = require('util.math2').v .. require('pkg').v .. require('ui.range_axis').v";
    assert_eq!(out(&m, code), "projectinitmine");
    assert!(warnings(&console).is_empty(), "{:?}", warnings(&console));
}

#[test]
fn require_falls_back_to_the_engines_bundled_scripts() {
    let root = dir("engine");
    let (m, console) = manager(&root);
    assert_eq!(
        out(&m, "out = type(require('ui.range_axis').track)"),
        "function"
    );
    assert!(warnings(&console).is_empty());
}

#[test]
fn a_missing_module_names_the_project_and_engine_paths_searched() {
    let root = dir("missing");
    let (m, _) = manager(&root);
    let err = m
        .exec("require('no_such_module_1062')")
        .expect_err("missing module");
    assert!(
        err.contains("assets/scripts/no_such_module_1062.lua"),
        "{err}"
    );
    assert!(
        err.contains("engine/scripts/no_such_module_1062.lua"),
        "{err}"
    );
}

#[test]
fn an_outside_require_loads_and_warns_once_per_file() {
    let root = dir("outside_root");
    let lib = dir("outside_lib");
    write(&lib.join("shared.lua"), "return 7");
    let (m, console) = manager(&root);
    let path = format!("{}/?.lua", lib.display());
    m.exec(&format!("package.path = [[{path}]]"))
        .expect("set path");
    let code = "package.loaded.shared = nil; out = tostring(require('shared'))";
    assert_eq!(out(&m, code), "7");
    assert_eq!(out(&m, code), "7");
    let warned = warnings(&console);
    assert_eq!(warned.len(), 1, "once per file: {warned:?}");
    assert!(warned[0].contains("shared.lua") && warned[0].contains("outside the project"));
}

#[test]
fn native_modules_stay_off() {
    let (m, _) = manager(&dir("native"));
    assert_eq!(out(&m, "out = package.cpath"), "");
    assert_eq!(out(&m, "out = tostring(#package.searchers)"), "4");
}
