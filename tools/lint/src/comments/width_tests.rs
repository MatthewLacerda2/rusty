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
    prose_after("// ", len)
}

fn prose_after(opener: &str, len: usize) -> String {
    let mut text = opener.to_string();
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
    for (kind, source) in [
        (Kind::Rust, format!("fn f() {{}} // {long}\n")),
        (Kind::Rust, format!("/// {long}\n")),
        (Kind::Wgsl, format!("/* {long} */\n")),
        (Kind::Lua, format!("x = 1 -- {long}\n")),
    ] {
        let opener = if kind == Kind::Lua { "-- " } else { "// " };
        let wrapped = prose_after(opener, 99) + &prose_after(opener, 60);
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
    assert_eq!(max_width(&repo.0), Ok(100), "rustfmt's default");
    repo.file(
        "rustfmt.toml",
        "edition = \"2021\"\nmax_width = 120 # note\n",
    );
    assert_eq!(max_width(&repo.0), Ok(120));
    repo.file("rustfmt.toml", "max_width = 80\n");
    assert_eq!(max_width(&repo.0), Ok(80));
    repo.file("src/a.rs", &prose(90));
    let found = crate::size::verdict(&repo.0, &[]).findings;
    let found: Vec<_> = found.iter().filter(|f| f.contains(WIDTH)).collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("src/a.rs:1: [comment-width] comment line of 90 columns, width 80")
    );
    assert!(found[0].contains("wrap it at the width"));
}

#[test]
fn a_width_that_does_not_parse_is_a_finding_on_rustfmt_toml() {
    let repo = Repo::new("comment-width-bad", true);
    repo.file("rustfmt.toml", "edition = \"2021\"\nmax_width = wide\n");
    let bad = max_width(&repo.0).expect_err("not a number");
    assert_eq!((bad.path.as_str(), bad.line), ("rustfmt.toml", 2));
    let found = crate::size::verdict(&repo.0, &[]).findings;
    assert!(
        found
            .iter()
            .any(|f| f.starts_with("rustfmt.toml:2: [comment-width]")),
        "{found:?}"
    );
}
