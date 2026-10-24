//! `require`: project, then engine, then `package.path`.

use super::{backslash_dir, dir, manager, out, warnings, write};

#[test]
fn require_finds_project_modules_first_without_warning() {
    let root = dir("project");
    write(
        &root.join(crate::core::paths::native("assets/scripts/util/math2.lua")),
        "return { v = 'project' }",
    );
    write(
        &root.join(crate::core::paths::native("assets/scripts/pkg/init.lua")),
        "return { v = 'init' }",
    );
    // A project copy of an engine helper wins over the bundled one.
    write(
        &root.join(crate::core::paths::native(
            "assets/scripts/ui/range_axis.lua",
        )),
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

/// A module that returns an error raised inside it, which its chunk name prefixes.
pub(super) const SOURCE: &str = "local _, e = pcall(function() error('here') end) return e";

#[test]
fn a_project_module_is_named_with_slashes_only() {
    let root = backslash_dir("slash_project");
    write(
        &root.join(crate::core::paths::native("assets/scripts/ui/axis.lua")),
        SOURCE,
    );
    let (m, _) = manager(&root);
    let named = out(&m, "local s, o = require('ui.axis'); out = s .. '|' .. o");
    assert!(!named.contains('\\'), "{named}");
    assert!(
        named.contains("a/b/assets/scripts/ui/axis.lua:1: here|"),
        "{named}"
    );
    assert!(named.ends_with("a/b/assets/scripts/ui/axis.lua"), "{named}");
}

#[test]
fn a_package_path_module_is_named_with_slashes_only() {
    let root = dir("slash_path_root");
    let lib = backslash_dir("slash_path_lib");
    write(&lib.join("shared.lua"), SOURCE);
    let (m, _) = manager(&root);
    let path = format!("{}/?.lua", lib.display());
    m.exec(&format!("package.path = [[{path}]]"))
        .expect("set path");
    let named = out(&m, "local s, o = require('shared'); out = s .. '|' .. o");
    assert!(!named.contains('\\'), "{named}");
    assert!(named.contains("a/b/shared.lua:1: here|"), "{named}");
    assert!(named.ends_with("a/b/shared.lua"), "{named}");
}

#[test]
fn the_outside_warning_prints_a_slash_path_without_a_verbatim_prefix() {
    let root = dir("slash_warn_root");
    let lib = backslash_dir("slash_warn_lib");
    write(&lib.join("shared.lua"), "return 1");
    let (m, console) = manager(&root);
    m.exec(&format!("package.path = [[{}/?.lua]]", lib.display()))
        .expect("set path");
    m.exec("require('shared')").expect("require");
    let warned = warnings(&console);
    let shown = warned[0].split(", outside").next().expect("path");
    assert!(!shown.contains('\\'), "no `\\` and no `\\\\?\\`: {shown}");
    assert!(shown.ends_with("a/b/shared.lua"), "{shown}");
}

#[test]
fn a_module_name_that_is_not_a_relative_path_is_refused_and_reads_nothing() {
    let root = dir("empty_segment");
    write(&root.join("assets/scripts/a/b.lua"), "return { v = 'ab' }");
    let outside = dir("empty_segment_outside");
    write(&outside.join("x.lua"), "ran = true; return 1");
    let (m, console) = manager(&root);
    let abs = format!("{}/x", outside.display());
    for name in [".util", "a..b", "a.", ".", "", "/util", abs.as_str()] {
        let err = m.exec(&format!("require([[{name}]])")).expect_err(name);
        assert!(err.contains(&format!("bad module name '{name}'")), "{err}");
    }
    assert_eq!(out(&m, "out = tostring(ran) .. require('a.b').v"), "nilab");
    assert!(warnings(&console).is_empty(), "{:?}", warnings(&console));
}
