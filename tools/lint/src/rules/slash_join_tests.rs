use super::RULE;
use crate::rules::{allowed, test_code::test_lines, Finding};

fn check_file(path: &str, src: &str) -> Vec<Finding> {
    super::check_file(path, src, &test_lines(path, src, false))
}

const JOIN: &str = "    let f = dir.join(\"models/b.fbx\");\n";

#[test]
fn a_joined_slash_literal_in_test_code_is_flagged() {
    for path in ["src/a/tests.rs", "src/a/b_tests.rs", "tests/c.rs"] {
        let found = check_file(path, &format!("fn f() {{\n{JOIN}}}\n"));
        assert_eq!(found.len(), 1, "{path}");
        assert_eq!((found[0].rule, found[0].line), (RULE, 2));
    }
    let later = "    let f = dir.join(\"a\").join(\"b/c\");\n";
    assert_eq!(
        check_file("tests/c.rs", later).len(),
        1,
        "any join on the line"
    );
}

#[test]
fn one_part_per_join_and_production_code_pass() {
    let parts = "    let f = dir.join(\"models\").join(\"b.fbx\");\n";
    assert!(check_file("tests/c.rs", parts).is_empty());
    let joined_var = "    let f = dir.join(name).join(\"x\");\n";
    assert!(check_file("tests/c.rs", joined_var).is_empty());
    let words = "    let s = [\"a\", \"b/c\"].join(\",\");\n";
    assert!(
        check_file("tests/c.rs", words).is_empty(),
        "a slash outside the join"
    );
    let slice = "    let rel = parts.join(\"/\");\n";
    assert!(
        check_file("tests/c.rs", slice).is_empty(),
        "a separator, not a path"
    );
    assert!(check_file("src/a/b.rs", JOIN).is_empty(), "not test code");
    assert!(check_file("tests/c.rs", "// dir.join(\"a/b\")\n").is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_a_slashed_path() {
    let src = format!("// lint: allow(slash-join) an asset path is `/`-separated\n{JOIN}");
    let found = check_file("tests/a.rs", &src);
    assert!(allowed(&src, found[0].line, RULE));
}
