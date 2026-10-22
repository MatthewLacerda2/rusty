use super::{code_lines, strip_comment, Open};

fn code(line: &str) -> String {
    strip_comment(line, &mut Open::None).into_owned()
}

#[test]
fn a_line_comment_is_cut() {
    assert_eq!(code("let x = 1; // crate::render"), "let x = 1; ");
}

#[test]
fn a_slash_slash_inside_a_string_is_kept() {
    assert_eq!(
        code(r#"let u = "rusty://x"; // (note"#),
        r#"let u = "rusty://x"; "#
    );
    assert_eq!(
        code(r#"open("http://x"); fs::rename(a, b);"#),
        r#"open("http://x"); fs::rename(a, b);"#
    );
}

#[test]
fn code_after_a_block_comment_on_the_same_line_is_kept() {
    assert_eq!(code("a /* x */ fs::rename(a, b);"), "a   fs::rename(a, b);");
    assert_eq!(code("a /* x // y */ b // c"), "a   b ");
    assert_eq!(
        code(r#"s = "/* not a comment */"; t"#),
        r#"s = "/* not a comment */"; t"#
    );
}

#[test]
fn a_block_comment_carries_across_lines_and_nests() {
    let lines = code_lines("a /* open\nfs::rename /* inner */ still\nend */ b\nc");
    assert_eq!(lines, ["a  ", " ", "  b", "c"]);
}

/// Lines of `source` that cut a line at its first `//` by hand: the copies #1199
/// removed, each of which misread `"http://x"` and ignored `/* */`.
fn hand_rolled(source: &str) -> Vec<usize> {
    let cut = [".split(\"//\")", ".find(\"//\")", ".split_once(\"//\")"];
    let lines: Vec<&str> = source.lines().collect();
    let mut open = Open::None;
    let mut found = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let code = strip_comment(l, &mut open);
        // A line that reads the comment itself says so on the line above.
        let excused = i > 0 && lines[i - 1].contains("comment-cut:");
        if !excused && cut.iter().any(|c| code.contains(c)) {
            found.push(i + 1);
        }
    }
    found
}

#[test]
fn a_hand_rolled_comment_cut_is_flagged_and_the_stripper_is_not() {
    assert_eq!(hand_rolled("let c = l.split(\"//\").next();\nok();"), [1]);
    assert_eq!(hand_rolled("let c = l.find(\"//\");"), [1]);
    assert!(hand_rolled("let c = strip_comment(l, &mut open);").is_empty());
    assert!(hand_rolled("// a note on .split(\"//\")").is_empty());
    let excused = "// comment-cut: reads the comment\nlet c = l.find(\"//\");";
    assert!(hand_rolled(excused).is_empty());
}

#[test]
fn no_lint_source_cuts_a_comment_by_hand() {
    fn walk(dir: &std::path::Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|e| e == "rs")
                && !path.ends_with("code_tests.rs")
            {
                let text = std::fs::read_to_string(&path).unwrap();
                for line in hand_rolled(&text) {
                    found.push(format!("{}:{line}", path.display()));
                }
            }
        }
    }
    let mut found = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut found,
    );
    assert!(
        found.is_empty(),
        "use crate::code::strip_comment: {found:?}"
    );
}
