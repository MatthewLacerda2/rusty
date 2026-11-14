use super::*;

#[test]
fn a_path_under_its_root_comes_out_slash_separated() {
    let path = "/tmp/rusty-tests/4192/rusty_busy_a/project/assets/scripts/busy_director.lua";
    assert_eq!(
        relative_to(path, "/tmp/rusty-tests/4192/rusty_busy_a").as_deref(),
        Some("project/assets/scripts/busy_director.lua")
    );
}

#[test]
fn two_runs_in_different_roots_store_the_same_path() {
    let at = |run: &str| {
        let path = format!("/tmp/ws/{run}/project/assets/scripts/busy_box.lua");
        relative_to(&path, &format!("/tmp/ws/{run}"))
    };
    assert_eq!(at("rusty_busy_a"), at("rusty_busy_b"));
}

#[test]
fn a_backslash_is_a_filename_character_not_a_separator() {
    assert_eq!(
        relative_to(r"/ws/models/a\b.png", "/ws/models").as_deref(),
        Some(r"a\b.png")
    );
    assert_eq!(relative_to(r"/ws\models/a.png", "/ws"), None, "one name");
}

#[test]
fn components_compare_case_sensitively() {
    assert_eq!(relative_to("/tmp/WS/x.lua", "/tmp/ws"), None);
}

#[test]
fn unix_paths_and_the_non_matches() {
    assert_eq!(
        relative_to("/tmp/ws/./project/x.lua", "/tmp/ws").as_deref(),
        Some("project/x.lua")
    );
    assert_eq!(relative_to("/tmp/wsx/x.lua", "/tmp/ws"), None, "a sibling");
    assert_eq!(relative_to("/tmp/ws", "/tmp/ws"), None, "the root itself");
    assert_eq!(relative_to("tmp/ws/x.lua", "/tmp/ws"), None, "relative");
    assert_eq!(relative_to("project/x.lua", ""), None, "no root");
}

#[test]
fn relativize_resolves_through_the_filesystem_and_leaves_the_rest() {
    let root = rusty_test_support::temp::dir().join("rusty_paths");
    std::fs::create_dir_all(root.join("project")).unwrap();
    let file = root.join("project").join("x.lua");
    std::fs::write(&file, "").unwrap();
    // A `..` hop is lexically outside the prefix; the canonical form is not.
    let hop = root
        .join("project")
        .join("..")
        .join("project")
        .join("x.lua");
    assert_eq!(relativize(&hop.to_string_lossy(), &root), "project/x.lua");
    assert_eq!(relativize("project/x.lua", &root), "project/x.lua");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn stored_trims_and_drops_a_leading_dot_but_keeps_a_backslash() {
    assert_eq!(stored(" ./assets/luts/a.cube \n"), "assets/luts/a.cube");
    assert_eq!(stored(r"./assets\a.cube"), r"assets\a.cube");
    assert_eq!(stored(".//./a.cube"), "a.cube");
    assert_eq!(stored("/abs/a.cube"), "/abs/a.cube");
    assert_eq!(stored("../a.cube"), "../a.cube");
}

#[test]
fn to_slash_keeps_a_backslash_as_part_of_a_name() {
    assert_eq!(
        to_slash(Path::new(r"assets\scripts/a.lua")),
        r"assets\scripts/a.lua"
    );
    assert_eq!(to_slash(Path::new("/tmp/x")), "/tmp/x");
}

#[test]
fn the_test_helpers_lua_path_spells_a_path_as_to_slash_does() {
    // `rusty_test_support::temp::lua_path` sits below `core` and repeats `to_slash`;
    // this holds the two in step.
    let name = "a dir/b.lua";
    assert_eq!(
        rusty_test_support::temp::lua_path(name),
        to_slash(&rusty_test_support::temp::dir().join(name))
    );
}

#[test]
fn extension_matching_ignores_case() {
    assert!(has_extension("assets/scripts/Enemy.LUA", "lua"));
    assert!(has_extension(Path::new("a/b.Lua"), "lua"));
    assert!(
        !has_extension("a/lua", "lua"),
        "a bare name has no extension"
    );
    assert!(!has_extension("a/b.luac", "lua"));
    assert_eq!(extension("Crate.GLB"), "glb");
    assert_eq!(extension("noext"), "");
}

#[test]
fn fold_drops_dots_and_folds_climbs_without_the_disk() {
    let fold = |p: &str| super::fold(Path::new(p));
    assert_eq!(
        fold("./a/b/../../../../m/s.png"),
        Path::new("../../m/s.png")
    );
    assert_eq!(fold("./cache/x/../../m/s.png"), Path::new("m/s.png"));
    assert_eq!(fold("/tmp/r/../../../home/s.png"), Path::new("/home/s.png"));
    assert_eq!(fold("a/.."), Path::new("."));
    assert_eq!(fold("a/./b"), Path::new("a/b"));
}
