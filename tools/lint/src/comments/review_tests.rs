use super::{check, split, Kind, STRICT};
use crate::repo::scaffold::Repo;

fn w(n: usize) -> String {
    "w ".repeat(n)
}

fn over(source: &str) -> bool {
    check("x.rs", Kind::Rust, source).over.is_some()
}

fn history(source: &str) -> usize {
    check("x.rs", Kind::Rust, source).history.len()
}

#[test]
fn a_lifetime_opens_no_literal() {
    let lines = split(
        Kind::Rust,
        "fn f(x: &'static str) -> &'a u8 { g(\"don't\") } // c\n",
    );
    assert_eq!(lines[0].comment.trim(), "// c");
    let after = format!(
        "struct S<'a>(&'a u8);\n#[doc = \"{}\"]\nfn f() {{}}\n",
        w(101)
    );
    assert!(
        over(&after),
        "a doc attribute after an unpaired lifetime is counted"
    );
    let chars = split(Kind::Rust, "let c = ['\\'', 'x', '\"']; // c\n");
    assert_eq!(chars[0].comment.trim(), "// c");
}

#[test]
fn a_reference_on_a_free_line_is_still_flagged() {
    assert_eq!(history("// sorted: by name, #1234\n"), 1);
    assert_eq!(history("// lint: allow(dir-size) see #1234\n"), 1);
    assert_eq!(history("// SAFETY: as #1234 says\nunsafe { f() }\n"), 1);
    assert_eq!(
        history("// lint: allow(comment-history) upstream #1234\n"),
        0
    );
}

#[test]
fn the_safety_note_is_its_own_run_directly_above_unsafe() {
    for code in [
        "unsafe { f() }",
        "unsafe impl Send for S {}",
        "pub unsafe fn f() {}",
        "x = unsafe{ f() };",
    ] {
        assert!(
            !over(&format!("// SAFETY: {}\n// {}\n{code}\n", w(60), w(60))),
            "{code}"
        );
    }
    let blank = format!("// SAFETY: {}\n\n// {}\nunsafe {{}}\n", w(60), w(60));
    assert!(over(&blank), "a blank line ends the note");
    let other = format!("// SAFETY: {}\n/// {}\nunsafe fn f() {{}}\n", w(60), w(60));
    assert!(over(&other), "a comment of another opener ends it");
    let named = format!("// SAFETY: {}\nlet unsafe_ptr = 1;\n", w(101));
    assert!(over(&named), "a word holding `unsafe` is no unsafe block");
}

#[test]
fn a_strict_run_over_a_folder_checks_the_agent_docs_inside_it() {
    let repo = Repo::new("comments-docs", true);
    repo.file("src/a.rs", "fn a() {}\n");
    repo.file("src/CLAUDE.md", "# Rules\n\nAs #12 decided.\n");
    let args = [STRICT, "src"].map(String::from);
    let found = crate::size::verdict(&repo.0, &args).findings;
    assert!(
        found.iter().any(|f| f.starts_with("src/CLAUDE.md")),
        "{found:?}"
    );
}
