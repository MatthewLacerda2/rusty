use super::{check_manifest, check_source, RULE};
use crate::rules::test_code::test_lines;

const MANIFEST: &str =
    "[package]\nname = \"rusty\"\n\n[lib]\nname = \"rusty\"\npath = \"src/lib.rs\"\n";

#[test]
fn flags_a_root_target_that_builds_a_harness() {
    let text = format!(
        "{MANIFEST}test = false\n\n[[bin]]\nname = \"play\"\npath = \"src/bin/play.rs\"\n\n\
         [[bin]]\nname = \"game\"\n# test = false\n\n[[test]]\nname = \"integration\"\n"
    );
    let found: Vec<_> = check_manifest(&text)
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    // The commented-out key does not count; the `[[test]]` target is not held.
    assert_eq!(found, [(9, RULE), (13, RULE)]);
    assert_eq!(
        check_manifest(MANIFEST).len(),
        1,
        "the last section is read too"
    );
}

#[test]
fn passes_targets_that_say_test_false() {
    let text = format!(
        "{MANIFEST}test = false\n\n[[bin]]\nname = \"play\"\ntest=false  # no harness\n\n\
         [[test]]\nname = \"integration\"\n"
    );
    assert!(check_manifest(&text).is_empty());
}

#[test]
fn flags_a_test_in_root_src() {
    let src = "fn main() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n";
    let lines = test_lines("src/bin/play.rs", src, false);
    let found = check_source("src/bin/play.rs", &lines).map(|f| (f.line, f.rule));
    assert_eq!(found, Some((3, RULE)));
}

#[test]
fn passes_tests_in_tests_and_member_crates() {
    let src = "#[cfg(test)]\nmod tests {}\n";
    let member = test_lines("crates/core/src/lib.rs", src, false);
    assert!(check_source("crates/core/src/lib.rs", &member).is_none());
    let suite = test_lines("tests/main.rs", src, false);
    assert!(check_source("tests/main.rs", &suite).is_none());
    let plain = test_lines("src/main.rs", "fn main() {}\n", false);
    assert!(check_source("src/main.rs", &plain).is_none());
}
