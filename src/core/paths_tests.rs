use super::*;

/// The path #783's Windows run embedded: the runner's temp dir, short-named.
const RUN: &str = r"C:\Users\RUNNER~1\AppData\Local\Temp\rusty-tests\4192\busy";

#[test]
fn a_windows_path_under_its_root_comes_out_slash_separated() {
    let path = format!(r"{RUN}\rusty_busy_a\project\assets\scripts\busy_director.lua");
    let root = format!(r"{RUN}\rusty_busy_a");
    assert_eq!(
        relative_to(&path, &root).as_deref(),
        Some("assets/scripts/busy_director.lua")
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
    let root = std::env::temp_dir().join(format!("rusty_paths_{}", std::process::id()));
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
