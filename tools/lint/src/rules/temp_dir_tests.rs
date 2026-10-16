use super::finding::allowed;
use super::temp_dir::{check_file, RULE};

const CALL: &str = "    let d = std::env::temp_dir().join(\"x\");\n";

#[test]
fn a_test_file_is_flagged_on_the_calls_line() {
    for path in ["src/a/tests.rs", "src/a/b_tests.rs", "tests/gpu/c.rs"] {
        let found = check_file(path, &format!("fn f() {{\n{CALL}}}\n"));
        assert_eq!(found.len(), 1, "{path}");
        assert_eq!((found[0].rule, found[0].line), (RULE, 2));
    }
}

#[test]
fn only_lines_after_cfg_test_count_in_a_source_file() {
    let src = format!("fn prod() {{\n{CALL}}}\n#[cfg(test)]\nmod tests {{\n{CALL}}}\n");
    let found = check_file("src/a/b.rs", &src);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].line, 6);
}

#[test]
fn comments_and_other_roots_pass() {
    let doc = "/// would reach for `std::env::temp_dir()`\n";
    assert!(check_file("tests/temp.rs", doc).is_empty());
    assert!(check_file("tools/lint/src/x_tests.rs", CALL).is_empty());
    assert!(
        check_file("src/bin/capture.rs", CALL).is_empty(),
        "not test code"
    );
}

#[test]
fn an_allow_with_a_reason_excuses_a_socket_path() {
    let src = format!("// lint: allow(test-temp-dir) a socket path stays short\n{CALL}");
    let found = check_file("tests/a.rs", &src);
    assert!(allowed(&src, found[0].line, RULE));
}
