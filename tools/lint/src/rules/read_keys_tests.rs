use super::{check, missing, MUST_CALL, RULE};

const CALL: &str = "fn main() {\n    rusty::dev::providers::read_keys();\n}\n";

#[test]
fn flags_a_call_outside_a_binary_root() {
    let found: Vec<_> = check("crates/dev/src/a.rs", CALL)
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    assert_eq!(found, [(2, RULE)]);
    assert_eq!(check("tests/a.rs", CALL).len(), 1);
    assert_eq!(check("src/bin/nested/a.rs", CALL).len(), 1);
}

#[test]
fn passes_binary_roots_the_definition_comments_and_the_lint_tool() {
    assert!(check("src/main.rs", CALL).is_empty());
    assert!(check("src/bin/session.rs", CALL).is_empty());
    assert!(check("tools/lint/src/a.rs", CALL).is_empty());
    let other = "pub fn read_keys() {}\n// read_keys();\n/// [`read_keys`]\n";
    assert!(check("crates/dev/src/a.rs", other).is_empty());
}

#[test]
fn flags_a_spending_binary_that_lost_its_opt_in() {
    let found = missing("src/bin/session.rs", "fn main() {}\n").expect("flagged");
    assert_eq!((found.line, found.rule), (1, RULE));
    assert!(missing("src/bin/session.rs", CALL).is_none());
    assert!(MUST_CALL.contains(&"src/main.rs"));
}

#[test]
fn an_allow_with_a_reason_excuses_a_call() {
    let src = "// lint: allow(read-keys) a test's child proving the opt-in\nread_keys();\n";
    let found = check("crates/dev/src/a.rs", src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
