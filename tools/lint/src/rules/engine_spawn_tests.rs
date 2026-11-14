use super::{check, HELPER, RULE};

#[test]
fn flags_a_spawn_that_skips_the_helper() {
    let src = "fn t() {\n    let c = Command::new(env!(\"CARGO_BIN_EXE_session\"));\n}\n";
    let found: Vec<_> = check("tests/a.rs", src)
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    assert_eq!(found, [(2, RULE)]);
    assert_eq!(check("crates/dev/tests/a.rs", src).len(), 1);
}

#[test]
fn passes_the_helper_comments_and_the_lint_tool() {
    let src = "let c = std::process::Command::new(env!(\"CARGO_BIN_EXE_play\"));\n";
    assert!(check(HELPER, src).is_empty());
    assert!(check("tools/lint/src/dead_pub/mod.rs", src).is_empty());
    let via = "let c = crate::spawn::engine_bin!(\"play\");\n// CARGO_BIN_EXE_play\n";
    assert!(check("tests/a.rs", via).is_empty());
}
