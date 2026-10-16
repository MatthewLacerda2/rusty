use super::finding::allowed;
use super::prefix::{anchor, check_dir, groups, RULE};
use std::fs;

#[test]
fn three_siblings_sharing_a_word_are_a_group() {
    let files = [
        "nav_agent.rs",
        "nav_modifier.rs",
        "nav-obstacle.rs",
        "light.rs",
    ];
    assert_eq!(
        groups(&files),
        vec![("nav", vec!["nav-obstacle", "nav_agent", "nav_modifier"])]
    );
}

#[test]
fn a_bare_word_counts_with_its_prefixed_siblings() {
    let files = [
        "audio.rs",
        "audio_occlusion_tests.rs",
        "audio_reverb_tests.rs",
    ];
    assert_eq!(groups(&files).len(), 1);
}

#[test]
fn a_source_and_its_tests_sibling_count_once() {
    let files = [
        "probe_bake.rs",
        "probe_bake_tests.rs",
        "probe_bounce.rs",
        "mod.rs",
    ];
    assert!(groups(&files).is_empty(), "two stems, not three files");
}

#[test]
fn owner_files_never_count() {
    assert!(groups(&["mod.rs", "main.rs", "lib.rs", "main_loop.rs"]).is_empty());
}

#[test]
fn the_finding_anchors_on_the_first_declaring_mod_line() {
    let owner = "mod light;\n#[cfg(test)]\npub(crate) mod nav_b;\nmod nav_a;\n";
    assert_eq!(anchor(owner, &["nav_a.rs", "nav_b.rs"]), Some(3));
    assert_eq!(anchor(owner, &["gone.rs"]), None);
}

#[test]
fn a_directory_is_flagged_and_an_allow_above_its_mod_line_excuses_it() {
    let dir = std::env::temp_dir().join(format!("rusty-lint-934-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let owner = "mod light;\n// lint: allow(prefix-group) mirrors Unity's names\nmod nav_a;\nmod nav_b;\nmod nav_c;\n";
    fs::write(dir.join("mod.rs"), owner).unwrap();
    let dir_s = dir.to_string_lossy().replace('\\', "/");
    let found = check_dir(
        &dir_s,
        &["mod.rs", "light.rs", "nav_a.rs", "nav_b.rs", "nav_c.rs"],
    );
    fs::remove_dir_all(&dir).ok();
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].rule, found[0].line), (RULE, 3));
    assert_eq!(found[0].path, format!("{dir_s}/mod.rs"));
    assert!(allowed(owner, found[0].line, RULE));
}

#[test]
fn exempt_registries_pass() {
    let files = ["scene.rs", "scene_load.rs", "scene_prefab.rs"];
    assert!(check_dir("src/api", &files).is_empty());
    assert_eq!(
        check_dir("src/api/material", &files).len(),
        1,
        "not its subfolders"
    );
}
