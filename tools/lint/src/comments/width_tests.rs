use super::width::{check, max_width, WIDTH};
use super::Kind;
use crate::repo::scaffold::Repo;

fn lines(kind: Kind, source: &str) -> Vec<usize> {
    check("a", kind, source, 100)
        .iter()
        .map(|f| f.line)
        .collect()
}

fn prose(len: usize) -> String {
    let mut text = "// ".to_string();
    while text.len() < len {
        text.push_str("word ");
    }
    text.truncate(len - 1);
    text.push('x');
    text + "\n"
}

#[test]
fn a_comment_line_past_the_width_is_flagged_in_each_language() {
    let long = "w ".repeat(60);
    let wrapped = format!("{}\n{}\n", prose(99), prose(60));
    for (kind, source) in [
        (Kind::Rust, format!("fn f() {{}} // {long}\n")),
        (Kind::Rust, format!("/// {long}\n")),
        (Kind::Wgsl, format!("/* {long} */\n")),
        (Kind::Lua, format!("x = 1 -- {long}\n")),
    ] {
        assert_eq!(lines(kind, &source), [1], "{kind:?}: {source}");
        assert!(
            lines(kind, &wrapped).is_empty(),
            "{kind:?} within the width"
        );
    }
    assert_eq!(lines(Kind::Rust, &prose(100)), Vec::<usize>::new());
    assert_eq!(lines(Kind::Rust, &prose(101)), [1]);
}

#[test]
fn code_markers_and_one_unbreakable_token_pass() {
    let code = format!("let s = \"{}\";\n", "w ".repeat(60));
    let marker = format!("// lint: allow(subprocess) {}\n", "w ".repeat(60));
    let url = format!("/// See <https://example.com/{}>.\n", "a".repeat(100));
    let allowed = format!(
        "// lint: allow(comment-width) a table\n// {}\n",
        "w ".repeat(60)
    );
    for source in [code, marker, url, allowed] {
        assert!(lines(Kind::Rust, &source).is_empty(), "{source}");
    }
}

#[test]
fn a_token_that_fits_its_own_line_is_wrapped() {
    let source = format!(
        "{} https://example.com/{}\n",
        prose(80).trim_end(),
        "a".repeat(30)
    );
    assert_eq!(lines(Kind::Rust, &source), [1]);
}

#[test]
fn the_width_is_rustfmts_and_the_message_says_to_wrap() {
    let repo = Repo::new("comment-width", true);
    assert_eq!(max_width(&repo.0), 100, "rustfmt's default");
    repo.file("rustfmt.toml", "edition = \"2021\"\nmax_width = 80\n");
    assert_eq!(max_width(&repo.0), 80);
    repo.file("src/a.rs", &prose(90));
    let found = crate::size::verdict(&repo.0, &[]).findings;
    let found: Vec<_> = found.iter().filter(|f| f.contains(WIDTH)).collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("src/a.rs:1: [comment-width] comment line of 90 columns, width 80")
    );
    assert!(found[0].contains("wrap it at the width"));
}
