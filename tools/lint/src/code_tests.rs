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
