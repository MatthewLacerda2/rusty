use super::subprocess::{check, RULE};

#[test]
fn flags_an_engine_command() {
    let src = "use std::process::Command;\nfn f() {}\n";
    let found: Vec<_> = check("src/a.rs", src)
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    assert_eq!(found, [(1, RULE)]);
    assert_eq!(
        check(
            "src/a.rs",
            "let c = std::process::Command::new(\"blender\");\n"
        )
        .len(),
        1
    );
    assert_eq!(
        check("src/a.rs", "use std::process::{Command, Stdio};\n").len(),
        1
    );
}

#[test]
fn passes_other_process_items_comments_and_non_engine_code() {
    let src = "let id = std::process::id();\nstd::process::exit(1);\n// std::process::Command\n";
    assert!(check("src/a.rs", src).is_empty());
    assert!(check("src/a.rs", "use std::process::CommandExt;\n").is_empty());
    let spawn = "let s = std::process::Command::new(exe);\n";
    assert!(check("tests/a.rs", spawn).is_empty());
    assert!(check("tools/lint/src/a.rs", spawn).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src =
        "// lint: allow(subprocess) asks git which commit this is\nuse std::process::Command;\n";
    let found = check("src/a.rs", src);
    assert!(super::finding::allowed(src, found[0].line, RULE));
}
