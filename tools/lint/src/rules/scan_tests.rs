use super::Scan;

#[test]
fn a_dotted_path_attribute_marks_the_file_the_walk_found() {
    let root = std::env::temp_dir().join(format!("lint_scan_dotted_{}", std::process::id()));
    let src = root.join("src");
    std::fs::create_dir_all(src.join("a")).unwrap();
    let file = |path: &str, text: &str| std::fs::write(src.join(path), text).unwrap();
    file(
        "a/mod.rs",
        "#[cfg(test)]\n#[path = \"./helper.rs\"]\nmod helper;\nmod live;\n",
    );
    file("a/helper.rs", "fn h() {}\nfn i() {}\n");
    file("a/live.rs", "fn l() {}\n");
    let scan = Scan::at(&root);
    std::fs::remove_dir_all(&root).ok();
    assert_eq!(scan.test_lines("src/a/helper.rs"), [true, true]);
    assert_eq!(scan.test_lines("src/a/live.rs"), [false]);
    assert_eq!(scan.test_lines("src/a/mod.rs"), [true, true, true, false]);
}
