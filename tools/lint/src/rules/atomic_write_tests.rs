use super::{check, RULE};

#[test]
fn flags_a_hand_rolled_atomic_write() {
    let src = "std::fs::write(&tmp, b)?;\nstd::fs::rename(&tmp, path)?;\n";
    let found: Vec<_> = check("src/a.rs", src)
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    assert_eq!(found, [(2, RULE)]);
    assert_eq!(check("src/a/b.rs", "use std::fs::rename;\n").len(), 1);
}

#[test]
fn passes_the_helper_comments_and_non_engine_code() {
    let rename = "std::fs::rename(&temp, path)\n";
    assert!(check("src/core/fs/mod.rs", rename).is_empty());
    assert!(check("src/a.rs", "// std::fs::rename is banned here\n").is_empty());
    assert!(check("src/a.rs", "core::fs::write_atomic(p, b)?;\n").is_empty());
    assert!(check("tests/a.rs", rename).is_empty());
    assert!(check("tools/lint/src/a.rs", rename).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src =
        "// lint: allow(atomic-write) a staged folder moves in whole\nstd::fs::rename(a, b)?;\n";
    let found = check("src/a.rs", src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}

#[test]
fn reads_code_past_a_string_slash_slash_and_a_block_comment() {
    let url = "let u = \"http://x\"; std::fs::rename(&a, &b)?;\n";
    assert_eq!(check("src/a.rs", url).len(), 1);
    let block = "/* moved */ std::fs::rename(&a, &b)?;\n";
    assert_eq!(check("src/a.rs", block).len(), 1);
    assert!(check("src/a.rs", "/* std::fs::rename\n still */ x();\n").is_empty());
}

#[test]
fn a_test_gate_inside_a_block_comment_gates_nothing() {
    use crate::rules::test_code::test_lines;
    let src = "/*\n#[cfg(test)]\nmod tests {}\n*/\nfn live() {}\n";
    assert_eq!(test_lines("src/a.rs", src, false), [false; 5]);
}
