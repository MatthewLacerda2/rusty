use super::Tree;
use std::path::Path;

#[test]
fn a_tree_drops_the_files_only_a_gated_mod_declares_and_keeps_the_rest() {
    let root = std::env::temp_dir().join(format!("lint_tree_{}", std::process::id()));
    let src = root.join("crates/core/src");
    std::fs::create_dir_all(&src).unwrap();
    let file = |name: &str, text: &str| std::fs::write(src.join(name), text).unwrap();
    file("lib.rs", "#[cfg(test)]\nmod fixture;\nmod live;\n");
    file("fixture.rs", "use rusty_render::X;\n");
    file("live.rs", "use rusty_scene::Y;\n");
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
            ("live.rs", "use rusty_scene::Y;\n"),
        ]
        .map(|(f, t)| (f.to_owned(), t.to_owned()))
    );
}

fn name(src: &Path, file: &Path) -> String {
    file.strip_prefix(src).unwrap().display().to_string()
}

#[test]
fn a_file_belongs_to_its_crates_module_and_the_root_package_is_read_by_none() {
    let root = std::env::temp_dir().join(format!("lint_tree_owner_{}", std::process::id()));
    let lib = root.join("crates/core/src");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::create_dir_all(root.join("crates/test-support/src")).unwrap();
    std::fs::create_dir_all(root.join("src/bin")).unwrap();
    std::fs::write(lib.join("lib.rs"), "pub mod a;\n").unwrap();
    std::fs::write(root.join("src/lib.rs"), "use rusty_core::x;\n").unwrap();
    let tree = Tree::read(&root);
    let owners = (
        tree.owner(&lib.join("a.rs")),
        tree.owner(&root.join("crates/test-support/src/lib.rs")),
        tree.owner(&root.join("src/main.rs")),
    );
    let files: Vec<String> = tree.live_files().map(|(f, _)| name(&root, f)).collect();
    std::fs::remove_dir_all(&root).ok();
    assert_eq!(owners, (Some("core".into()), None, None));
    assert_eq!(files, ["crates/core/src/lib.rs"]);
}
