use super::{check, missing, readers_drifted, MUST_CALL, RULE};

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
    // Another binary is flagged too: only the three spenders opt in.
    assert_eq!(check("src/bin/play.rs", CALL).len(), 1);
    assert_eq!(check("src/bin/new_tool.rs", CALL).len(), 1);
}

#[test]
fn passes_binary_roots_the_definition_comments_and_the_lint_tool() {
    for (spender, _) in MUST_CALL {
        assert!(check(spender, CALL).is_empty(), "{spender}");
    }
    assert!(check("tools/lint/src/a.rs", CALL).is_empty());
    let other = "pub fn read_keys() {}\n// read_keys();\n/// [`read_keys`]\n";
    assert!(check("crates/dev/src/a.rs", other).is_empty());
}

#[test]
fn flags_a_spending_binary_that_lost_its_opt_in() {
    let found = missing("src/bin/session.rs", "fn main() {}\n").expect("flagged");
    assert_eq!((found.line, found.rule), (1, RULE));
    assert!(missing("src/bin/session.rs", CALL).is_none());
    // A call in a string literal or a comment is not an opt-in.
    let quoted = "fn main() {\n    let _ = \"read_keys()\"; // read_keys();\n}\n";
    assert!(missing("src/bin/session.rs", quoted).is_some());
    assert!(MUST_CALL.contains(&("src/main.rs", "rusty")));
}

#[test]
fn an_allow_with_a_reason_excuses_a_call() {
    let src = "// lint: allow(read-keys) a test's child proving the opt-in\nread_keys();\n";
    let found = check("crates/dev/src/a.rs", src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}

#[test]
fn the_engine_list_of_key_readers_matches_the_lint() {
    // Must pass: the same three names, in order.
    let same = "pub const KEY_READERS: &[&str] = &[\"rusty\", \"session\", \"session-mcp\"];\n";
    assert!(readers_drifted(same).is_none());
    // Must flag: a name dropped, a name added, or no list at all.
    let short = "pub const KEY_READERS: &[&str] = &[\"rusty\", \"session\"];\n";
    assert_eq!(readers_drifted(short).map(|f| f.line), Some(1));
    let long =
        "pub const KEY_READERS: &[&str] = &[\"rusty\", \"session\", \"session-mcp\", \"play\"];\n";
    assert!(readers_drifted(long).is_some());
    assert!(readers_drifted("").is_some());
}
