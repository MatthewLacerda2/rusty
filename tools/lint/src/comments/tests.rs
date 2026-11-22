use super::{check, split, Kind, Line, CAP, HISTORY};
use crate::repo::scaffold::Repo;

fn comment(kind: Kind, n: usize) -> String {
    let marker = if kind == Kind::Lua { "--" } else { "//" };
    let words: Vec<&str> = std::iter::repeat_n("word", n).collect();
    let lines: Vec<String> = words
        .chunks(10)
        .map(|c| format!("{marker} {}\n", c.join(" ")))
        .collect();
    lines.concat()
}

fn over(kind: Kind, source: &str) -> Option<(usize, usize)> {
    check("x", kind, source).over
}

#[test]
fn one_word_over_the_cap_is_flagged_in_each_language_and_the_cap_itself_passes() {
    for kind in [Kind::Rust, Kind::Lua, Kind::Wgsl] {
        let cap = kind.cap();
        assert_eq!(over(kind, &comment(kind, cap)), None, "{kind:?} at its cap");
        let past = comment(kind, cap + 1);
        assert_eq!(over(kind, &past), Some((cap / 10 + 1, cap + 1)), "{kind:?}");
    }
}

#[test]
fn one_long_line_buys_nothing() {
    let long = format!("fn f() {{}} // {}\n", "word ".repeat(101));
    assert_eq!(over(Kind::Rust, &long), Some((1, 101)));
    assert_eq!(
        over(Kind::Wgsl, &format!("/* {} */\n", "w ".repeat(201))),
        Some((1, 201))
    );
    let lua = format!("--[[\n{}\n]]\nx = 1\n", "w ".repeat(101));
    assert_eq!(over(Kind::Lua, &lua), Some((2, 101)));
}

#[test]
fn the_words_of_a_doc_attribute_are_comment() {
    let attr = format!(
        "#![doc = \"{}\"]\n#[doc = r#\"{}\"#]\nfn f() {{}}\n",
        "w ".repeat(60),
        "w ".repeat(41)
    );
    assert_eq!(over(Kind::Rust, &attr), Some((2, 101)));
    let lines = split(Kind::Rust, "#[doc = \"a ] b\"] fn f() {}\n#[doc(hidden)]\n");
    assert_eq!(lines[0].code.trim(), "fn f() {}");
    assert!(lines[0].comment.contains("a ] b"));
    assert_eq!(
        lines[1].code, "#[doc(hidden)]",
        "`doc(hidden)` is no comment"
    );
}

#[test]
fn markers_and_a_safety_note_above_unsafe_are_free() {
    let free = format!(
        "// lint: allow(x) {w}\n// sorted: {w}\n//! stored-format: {w}\n\
         -- comment-cut: {w}\n// SAFETY: {w}\n// {w}\nunsafe {{ f() }}\n",
        w = "w ".repeat(60)
    );
    assert_eq!(over(Kind::Rust, &free), None);
    let not_above_unsafe = format!("// SAFETY: {}\nfn f() {{}}\n", "w ".repeat(101));
    assert!(over(Kind::Rust, &not_above_unsafe).is_some());
    let marker_mid_text = format!("// see sorted: {}\n", "w ".repeat(101));
    assert!(
        over(Kind::Rust, &marker_mid_text).is_some(),
        "only a comment opening with one"
    );
}

#[test]
fn markers_inside_string_literals_are_code() {
    let rust = format!(
        "let s = \"// {w}\"; let c = '\"'; let r = r#\"/* {w} */\"#;\n",
        w = "w ".repeat(101)
    );
    assert_eq!(over(Kind::Rust, &rust), None);
    let lua = format!("s = \"-- {w}\"\nt = [[\n-- {w}\n]]\n", w = "w ".repeat(101));
    assert_eq!(over(Kind::Lua, &lua), None);
    assert_eq!(
        split(Kind::Lua, "x = 'a' -- b\n")[0],
        Line {
            code: "x = 'a' ".into(),
            comment: "-- b".into()
        }
    );
}

#[test]
fn block_comments_nest_in_rust_and_wgsl() {
    for kind in [Kind::Rust, Kind::Wgsl] {
        let lines = split(kind, "a /* b /* c */ d */ e\nf");
        assert_eq!(
            lines[0].code.split_whitespace().collect::<Vec<_>>(),
            ["a", "e"]
        );
        assert!(lines[0].comment.contains('d'));
    }
}

fn history(kind: Kind, source: &str) -> Vec<String> {
    check(
        if kind == Kind::Lua { "x.lua" } else { "x.rs" },
        kind,
        source,
    )
    .history
    .into_iter()
    .map(|(_, what)| what)
    .collect()
}

#[test]
fn an_issue_or_pr_of_this_repository_is_flagged() {
    for (line, what) in [
        ("// fixed (#1695)", "#1695"),
        ("// since #453 it waits", "#453"),
        (
            "// https://github.com/MatthewLacerda2/rusty/issues/123",
            "/issues/123",
        ),
        ("// rusty#2001 and PR#404", "#2001"),
    ] {
        assert_eq!(history(Kind::Rust, line), [what], "{line}");
    }
    assert_eq!(history(Kind::Lua, "x = 1 -- see #1892"), ["#1892"]);
    assert_eq!(history(Kind::Wgsl, "/* #345 */"), ["#345"]);
}

#[test]
fn look_alikes_another_repository_and_an_allow_pass() {
    for line in [
        "// cascade #2, slot #12",
        "// upstream wgpu#245 and owner/repo#123, https://github.com/gfx-rs/wgpu/issues/4512",
        "// a colour #33ccff, an entity &#123;, a placeholder #N, a link [x](#123)",
        "// on 2026-11-08",
        "let s = \"#1234\";",
        "// lint: allow(comment-history) the upstream changelog's own numbering\nf(); // #999",
    ] {
        assert!(history(Kind::Rust, line).is_empty(), "{line}");
    }
    assert!(history(
        Kind::Lua,
        "-- lint: allow(comment-history) quoted\nx = 1 -- #999"
    )
    .is_empty());
}

#[test]
fn the_messages_say_what_to_do() {
    let source = format!("{}// #1234\n", comment(Kind::Rust, 101));
    let found = check("a.rs", Kind::Rust, &source).findings("a.rs", Kind::Rust);
    let shown: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert_eq!(
        found.iter().map(|f| f.rule).collect::<Vec<_>>(),
        [CAP, HISTORY]
    );
    assert!(shown[0].starts_with("a.rs:11: [comment-cap] 102 words of comment, cap 100: keep what"));
    assert!(shown[1].contains("`#1234` is history; say what the code does: `git blame`"));
}

#[test]
fn the_size_gate_fails_on_each_and_passes_a_clean_path_and_no_allow_excuses_the_cap() {
    let repo = Repo::new("comments", true);
    repo.file("src/a.rs", &comment(Kind::Rust, 101));
    repo.file("src/b.rs", "// fixed in #1234\n");
    repo.file("other/c.wgsl", "// ok\n");
    let allow = "// lint: allow(comment-cap) the format's grammar, quoted whole\n";
    repo.file("src/d.rs", &format!("{allow}{}", comment(Kind::Rust, 150)));
    let run = |paths: &[&str]| {
        let args: Vec<String> = paths.iter().map(ToString::to_string).collect();
        let found = crate::size::verdict(&repo.0, &args).findings;
        found
            .into_iter()
            .filter(|f| f.contains("[comment-"))
            .collect::<Vec<_>>()
    };
    let all = run(&[]);
    assert!(all[0].starts_with("src/a.rs:11: [comment-cap]"), "{all:?}");
    assert!(
        all[1].starts_with("src/b.rs:1: [comment-history]"),
        "{all:?}"
    );
    assert!(all[2].starts_with("src/d.rs:12: [comment-cap]"), "{all:?}");
    assert_eq!(all.len(), 3, "{all:?}");
    assert_eq!(run(&["src/a.rs"]).len(), 1);
    assert!(run(&["other"]).is_empty(), "a clean path passes");
}
