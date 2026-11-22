use super::{check, prose, Kind, HISTORY};
use crate::repo::scaffold::Repo;

/// The rule's own inputs spell a reference `@N`, so this file names none.
fn hash(source: &str) -> String {
    source.replace('@', "#")
}

fn history(source: &str) -> Vec<String> {
    let found = check("x.rs", Kind::Rust, &hash(source)).history;
    found.into_iter().map(|(_, what)| what).collect()
}

fn prose_lines(path: &str, source: &str) -> Vec<usize> {
    prose::check(path, &hash(source))
        .iter()
        .map(|f| f.line)
        .collect()
}

#[test]
fn a_comment_line_a_string_writes_for_another_file_is_read() {
    let table = "const HEADER: &str = \"\\\n# The table, pinned (@1286).\n\";\n";
    assert_eq!(history(table), ["#1286"]);
    let lua = "let s = r#\"\nx = 1\n-- What it drew before @2092.\n\"#;\n";
    assert_eq!(history(lua), ["#2092"]);
    let shader = "let s = format!(\"// {n}.wgsl, a variant (@272).\\n\");\n";
    assert_eq!(history(shader), ["#272"]);
}

#[test]
fn a_string_that_is_no_comment_line_passes() {
    for source in [
        "assert!(ok, \"the cap's cell (@1704)\");\n",
        "let c = \"@808080\";\n",
        "let s = \"\\\n#[derive(Debug)] @1234\n\";\n",
    ] {
        assert!(history(source).is_empty(), "{source}");
    }
}

#[test]
fn an_issue_named_by_its_word_is_flagged_at_any_digit_count() {
    assert_eq!(history("//! Smoke (issue @3)."), ["#3"]);
    assert_eq!(history("// see PR @74"), ["#74"]);
    assert!(history("// cascade @2, the tissue @4").is_empty());
}

#[test]
fn python_markdown_and_tables_under_the_engine_are_read() {
    let py = "\"\"\"Regenerate the fixture (@545).\"\"\"\n";
    assert_eq!(prose_lines("crates/a/src/make.py", py), [1]);
    assert_eq!(prose_lines("engine/x/README.md", "# A\n\nSee @385.\n"), [3]);
    let txt = "cc 5b45 # shrinks to x = 1 (@1805)\n";
    assert_eq!(prose_lines("crates/a/proptest-regressions/t.txt", txt), [1]);
    assert_eq!(prose_lines("games/g/game.toml", "# from @900\n"), [1]);
}

#[test]
fn prose_elsewhere_an_agent_doc_and_an_allowed_line_pass() {
    assert!(prose_lines(".github/scripts/a.py", "# @2381\n").is_empty());
    assert!(prose_lines("crates/CLAUDE.md", "@2381\n").is_empty());
    assert!(prose_lines("crates/a/x.json", "\"@2381\"\n").is_empty());
    let allow = "# lint: allow(comment-history) upstream's numbering\nx = 1  # @2381\n";
    assert!(prose_lines("crates/a/b.py", allow).is_empty());
    for (path, colour) in [
        ("crates/a/README.md", "The sky is `@202020`.\n"),
        ("crates/a/b.py", "BG = \"@20202080\"  # @202020 behind\n"),
        ("games/g/game.toml", "clear = \"@808080\"\n"),
    ] {
        assert!(
            prose_lines(path, colour).is_empty(),
            "{colour}: a colour of only digits"
        );
    }
    assert_eq!(prose_lines("crates/a/b.md", "issue @202020\n"), [1]);
}

#[test]
fn an_allow_above_a_multi_line_string_excuses_its_later_lines() {
    let table = "// lint: allow(comment-history) the upstream table's own header\n\
                 const HEADER: &str = \"\\\nx\n# The table, pinned (@1286).\n\";\n";
    assert!(history(table).is_empty(), "{table}");
    let unallowed = "const A: &str = \"\\\nx\n# pinned (@1286).\n\";\n";
    assert_eq!(history(unallowed), ["#1286"]);
}

#[test]
fn the_size_gate_reads_a_kept_markdown_file() {
    let repo = Repo::new("comments-prose", true);
    repo.file("crates/a/fixtures/README.md", &hash("# Fixtures (@385)\n"));
    let found = crate::size::verdict(&repo.0, &[]).findings;
    let wanted = format!("crates/a/fixtures/README.md:1: [{HISTORY}]");
    assert!(found.iter().any(|f| f.starts_with(&wanted)), "{found:?}");
}
