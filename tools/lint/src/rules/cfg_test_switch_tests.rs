use super::{check_file, RULE};

fn check(path: &str, source: &str) -> Vec<usize> {
    let test = crate::rules::test_code::test_lines(path, source, false);
    check_file(path, source, &test)
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn flags_a_switch_on_the_crates_own_tests() {
    let src = "fn f() -> bool {\n    cfg!(test) || other()\n}\n";
    assert_eq!(check("crates/logging/src/lib.rs", src), [2]);
    assert_eq!(
        check(
            "src/dev/a.rs",
            "let k = cfg!(any(test, feature = \"x\"));\n"
        ),
        [1]
    );
    assert_eq!(check("src/a.rs", "if cfg!(not(test)) {}\n"), [1]);
}

#[test]
fn passes_gates_test_code_comments_and_other_predicates() {
    let src = "#[cfg(test)]\nmod tests;\n// `cfg!(test)` holds only here\nlet d = cfg!(debug_assertions);\nlet t = cfg!(feature = \"testing\");\n";
    assert!(check("src/a.rs", src).is_empty());
    let gated = "#[cfg(test)]\nmod tests {\n    fn f() -> bool { cfg!(test) }\n}\n";
    assert!(check("src/a.rs", gated).is_empty());
    assert!(check("src/a_tests.rs", "let t = cfg!(test);\n").is_empty());
    assert!(check("tools/lint/src/a.rs", "let t = cfg!(test);\n").is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(cfg-test-switch) only this crate's tests read it back\nlet t = cfg!(test);\n";
    let line = check("src/a.rs", src)[0];
    assert!(crate::rules::allowed(src, line, RULE));
}
