use super::super::{check, File};
use super::test_file;
use crate::rules::test_code::test_lines;

fn found(all: &[(&str, &str)]) -> usize {
    let tests: Vec<Vec<bool>> = all.iter().map(|(p, s)| test_lines(p, s, false)).collect();
    let files: Vec<File> = all
        .iter()
        .zip(&tests)
        .map(|((path, source), test)| File { path, source, test })
        .collect();
    check(&files, &[], &|_, _| true).len()
}

#[test]
fn a_test_named_module_is_a_hook_not_a_test() {
    assert!(!test_file("crates/ai/src/perception/test_world.rs"));
    let module = "//! Test hook: the arena `dev`'s bench reads.\npub const PLAYER: u8 = 0;\n";
    let hook = ("crates/ai/src/perception/test_world.rs", module);
    let live = "fn f() { rusty_ai::perception::test_world::PLAYER; }\n";
    assert_eq!(
        found(&[hook, ("crates/dev/src/a.rs", live)]),
        1,
        "a live call into it"
    );
}

#[test]
fn benches_examples_and_tests_files_are_no_live_callers() {
    let hook = (
        "crates/lighting/src/occlusion/x.rs",
        "/// A test hook.\npub fn probe() {}\n",
    );
    let call = "fn f() { rusty_lighting::occlusion::probe(); }\n";
    for path in [
        "crates/dev/benches/a.rs",
        "crates/dev/examples/a.rs",
        "crates/dev/tests/a.rs",
        "crates/dev/src/tests.rs",
        "crates/dev/src/a_tests.rs",
    ] {
        assert!(test_file(path), "{path}");
        assert_eq!(found(&[hook, (path, call)]), 0, "{path}");
    }
}
