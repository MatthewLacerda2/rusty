use super::{check_dir, sources, MAX_FILES_PER_DIR, RULE};

/// `n` source file names, plus the owner and a few tests that never count.
fn names(n: usize) -> Vec<String> {
    let mut v: Vec<String> = (0..n).map(|i| format!("f{i}.rs")).collect();
    v.extend(["mod.rs", "tests.rs", "f0_tests.rs", "test_util.rs"].map(String::from));
    v
}

fn check(n: usize, owner: &str) -> Option<super::Finding> {
    let v = names(n);
    let files: Vec<&str> = v.iter().map(String::as_str).collect();
    check_dir("src/x", &files, Some(("src/x/mod.rs", owner)))
}

#[test]
fn tests_and_the_owner_never_count() {
    let v = names(3);
    let files: Vec<&str> = v.iter().map(String::as_str).collect();
    assert_eq!(sources("src/x", &files), vec!["f0.rs", "f1.rs", "f2.rs"]);
    assert!(sources("src/x/tests", &["a.rs"]).is_empty(), "under tests/");
}

#[test]
fn a_directory_over_the_cap_is_flagged_at_its_owner() {
    let f = check(MAX_FILES_PER_DIR + 1, "mod a;\n").expect("over the cap");
    assert_eq!((f.rule, f.path.as_str(), f.line), (RULE, "src/x/mod.rs", 1));
    assert!(f.message.contains("13 source files"), "{}", f.message);
}

#[test]
fn a_directory_at_the_cap_passes() {
    assert_eq!(check(MAX_FILES_PER_DIR, "mod a;\n"), None);
}

#[test]
fn a_registry_allow_with_a_reason_excuses_it() {
    let owner = "//! Docs.\n\n// lint: allow(dir-size) one file per Lua namespace\npub mod a;\n";
    assert_eq!(check(MAX_FILES_PER_DIR + 5, owner), None);
}

#[test]
fn an_allow_without_a_reason_is_left_to_the_shared_finding() {
    let owner = "// lint: allow(dir-size)\npub mod a;\n";
    assert_eq!(check(MAX_FILES_PER_DIR + 5, owner), None);
}

#[test]
fn an_allow_past_its_use_is_reported_on_its_line() {
    let owner = "pub mod a;\n// lint: allow(dir-size) flat registry\n";
    let f = check(2, owner).expect("stale allow");
    assert_eq!((f.path.as_str(), f.line), ("src/x/mod.rs", 2));
    assert!(f.message.contains("drop this allow"), "{}", f.message);
}
