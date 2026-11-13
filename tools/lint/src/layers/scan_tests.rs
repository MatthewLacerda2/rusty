use super::Tree;
use std::path::Path;

#[test]
fn a_tree_drops_the_files_only_a_gated_mod_declares_and_keeps_the_rest() {
    let root = std::env::temp_dir().join(format!("lint_tree_{}", std::process::id()));
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let file = |name: &str, text: &str| std::fs::write(src.join(name), text).unwrap();
    file("lib.rs", "#[cfg(test)]\nmod fixture;\nmod live;\n");
    file("fixture.rs", "use crate::render::X;\n");
    file("live.rs", "use crate::scene::Y;\n");
    let tree = Tree::read(&root);
    let live: Vec<(String, String)> = tree
        .live_files()
        .map(|(f, text)| (name(&src, f), text.to_owned()))
        .collect();
    let (fixture, lib) = (
        tree.is_test_only(&src.join("fixture.rs")),
        tree.is_test_only(&src.join("lib.rs")),
    );
    std::fs::remove_dir_all(&root).ok();
    assert!(fixture && !lib);
    assert_eq!(
        live,
        [
            ("lib.rs", "#[cfg(test)]\nmod fixture;\nmod live;\n"),
            ("live.rs", "use crate::scene::Y;\n"),
        ]
        .map(|(f, t)| (f.to_owned(), t.to_owned()))
    );
}

fn name(src: &Path, file: &Path) -> String {
    file.strip_prefix(src).unwrap().display().to_string()
}

#[test]
fn a_moved_module_is_read_at_its_module_path() {
    let root = std::env::temp_dir().join(format!("lint_tree_moved_{}", std::process::id()));
    let lib = root.join("crates/core/src");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::create_dir_all(root.join("src/scene")).unwrap();
    std::fs::write(lib.join("lib.rs"), "pub mod a;\n").unwrap();
    let tree = Tree::read(&root);
    let (core, scene, binary) = (
        tree.owner(&lib.join("a.rs")),
        tree.owner(&root.join("src/scene/mod.rs")),
        tree.owner(&root.join("src/main.rs")),
    );
    let in_crate = tree.crate_of(&lib.join("lib.rs"));
    std::fs::remove_dir_all(&root).ok();
    assert_eq!(
        (core.as_deref(), scene.as_deref(), binary),
        (Some("core"), Some("scene"), None)
    );
    assert_eq!(in_crate.as_deref(), Some("core"));
}
