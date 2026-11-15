use super::{check_manifest, check_source, RULE};
use crate::rules::test_code::test_lines;

const PACKAGE: &str = "[package]\nname = \"rusty\"\nautobins = false\n\n";

/// The header lines `check_manifest` flags in `text`.
fn flagged(text: &str) -> Vec<usize> {
    check_manifest(text).iter().map(|f| f.line).collect()
}

#[test]
fn flags_a_root_target_that_builds_a_harness() {
    let text = format!(
        "{PACKAGE}[lib]\npath = \"src/lib.rs\"\ntest = false\n\n[[bin]]\nname = \"play\"\n\n\
         [[bin]]\nname = \"game\"\n# test = false\n\n[[test]]\nname = \"integration\"\n"
    );
    // The commented-out key does not count; the `[[test]]` target is not held.
    assert_eq!(flagged(&text), [9, 12]);
    assert!(check_manifest(&text).iter().all(|f| f.rule == RULE));
    assert_eq!(flagged("[lib]\n"), [1], "the last section is read too");
}

#[test]
fn flags_a_package_that_finds_binaries_by_folder() {
    // `src/bin/bake.rs` with no `[[bin]]` would get a harness: the package must
    // turn cargo's own binary discovery off.
    let text = "[package]\nname = \"rusty\"\nautotests = false\n\n[lib]\ntest = false\n";
    assert_eq!(flagged(text), [1]);
    assert_eq!(flagged("[package]\nautobins = true\n"), [1]);
}

#[test]
fn passes_keys_however_they_are_spaced_or_commented() {
    let text = "[package]  # the root\nautobins\t=  false\n\n[lib]\ntest=false  # no harness\n\n\
                [[bin]]  # the editor\nname = \"rusty\"\ntest\t= false\n\n[[test]]\nname = \"i\"\n";
    assert!(flagged(text).is_empty());
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
