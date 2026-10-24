use super::RULE;
use crate::rules::{test_code::test_lines, Finding};

fn check_file(path: &str, src: &str) -> Vec<Finding> {
    super::check_file(path, src, &test_lines(path, src, false))
}

const BARE: &str = "fn f() {\n    let s = p.to_string_lossy();\n}\n";

#[test]
fn a_bare_to_string_lossy_in_lua_facing_code_is_flagged() {
    for path in ["src/api/assets.rs", "src/dev/lua_surface/sfx.rs"] {
        let found = check_file(path, BARE);
        assert_eq!(found.len(), 1, "{path}");
        assert_eq!((found[0].rule, found[0].line), (RULE, 2));
    }
}

#[test]
fn to_slash_other_modules_and_test_code_pass() {
    let slashed = "fn f() {\n    let s = to_slash(p);\n}\n";
    assert!(check_file("src/api/assets.rs", slashed).is_empty());
    assert!(
        check_file("src/editor/a.rs", BARE).is_empty(),
        "out of scope"
    );
    assert!(
        check_file("src/api/a_tests.rs", BARE).is_empty(),
        "test code"
    );
}
