use super::*;

/// The path #783's Windows run embedded: the runner's temp dir, short-named.
const RUN: &str = r"C:\Users\RUNNER~1\AppData\Local\Temp\rusty-tests\4192\busy";

#[test]
fn a_windows_path_under_its_root_comes_out_slash_separated() {
    let path = format!(r"{RUN}\rusty_busy_a\project\assets\scripts\busy_director.lua");
    let root = format!(r"{RUN}\rusty_busy_a");
    assert_eq!(
        relative_to(&path, &root).as_deref(),
        Some("project/assets/scripts/busy_director.lua")
    );
}

#[test]
fn two_runs_in_different_roots_store_the_same_path() {
    let at = |run: &str| {
        let path = format!(r"{RUN}\{run}\project\assets\scripts\busy_box.lua");
        relative_to(&path, &format!(r"{RUN}\{run}"))
    };
    assert_eq!(at("rusty_busy_a"), at("rusty_busy_b"));
}

#[test]
fn the_drive_letter_and_mixed_separators_and_a_verbatim_prefix_still_match() {
    let path = r"\\?\c:\Users\RUNNER~1/ws\project\x.lua";
    assert_eq!(
        relative_to(path, r"C:\Users\RUNNER~1\ws\").as_deref(),
        Some("project/x.lua")
    );
}

#[test]
fn a_short_name_does_not_lexically_match_its_long_name() {
    let path = r"C:\Users\RUNNER~1\ws\x.lua";
    assert_eq!(relative_to(path, r"C:\Users\runneradmin\ws"), None);
}

#[test]
fn only_the_drive_letter_ignores_case() {
    assert_eq!(relative_to(r"C:\Users\Me\x.lua", r"C:\users\me"), None);
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
    let root = crate::test_temp::dir().join("rusty_paths");
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
fn the_verbatim_prefix_is_stripped_so_a_slash_join_reads_on_windows() {
    // #904: `engine_path` joined `/shaders/...` onto `\\?\D:\...`, which Windows reads
    // literally.
    let engine = strip_verbatim(r"\\?\D:\a\rusty\rusty\engine");
    assert_eq!(engine, r"D:\a\rusty\rusty\engine");
    assert_eq!(
        strip_verbatim(r"\\?\UNC\host\share\engine"),
        r"\\host\share\engine"
    );
    for plain in [r"D:\a\engine", r"\\host\share", "/home/u/engine", "engine"] {
        assert_eq!(strip_verbatim(plain), plain);
    }
}

#[test]
fn to_slash_turns_every_backslash_into_a_slash() {
    assert_eq!(
        to_slash(Path::new(r"assets\scripts/a.lua")),
        "assets/scripts/a.lua"
    );
    assert_eq!(to_slash(Path::new("/tmp/x")), "/tmp/x");
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

#[test]
fn a_slashed_path_comes_out_in_the_platforms_separator() {
    use std::path::MAIN_SEPARATOR_STR as SEP;
    assert_eq!(native_string("a/b/c"), ["a", "b", "c"].join(SEP));
    assert_eq!(native("a/b/c"), Path::new("a").join("b").join("c"));
    let rooted = Path::new("/").join("engine").join("target");
    assert_eq!(native("/engine/target"), rooted);
    assert_eq!(
        native("a//b/"),
        Path::new("a").join("b"),
        "empty parts drop"
    );
}
