use super::check;

fn flagged(line: &str) -> Option<String> {
    check(line).into_iter().next().map(|(_, m)| m)
}

#[test]
fn each_shape_of_history_is_flagged() {
    for (line, what) in [
        ("fixed in #123.", "#123"),
        ("see MatthewLacerda2/scorsese#1511 for it", "#1511"),
        ("https://github.com/o/r/issues/42", "/issues/42"),
        ("(https://github.com/o/r/pull/7)", "/pull/7"),
        ("on 2026-11-08 that cost a round", "2026-11-08"),
        ("#9", "#9"),
        ("a parenthesised (#12) ref", "#12"),
        ("[a link](#setup) then #34", "#34"),
    ] {
        let msg = flagged(line).unwrap_or_else(|| panic!("{line:?} passed"));
        assert!(msg.starts_with(&format!("`{what}` is history")), "{msg}");
        assert!(
            msg.contains("state the rule"),
            "the message says what to do"
        );
    }
}

#[test]
fn look_alikes_pass() {
    for line in [
        "a placeholder: `Closes #N`, `watch: #A goes before #B`",
        "a hex colour #33ccff, a short one #0f0, black #000000",
        "an entity &#123; in HTML",
        "an in-page link [setup](#1-setup), [step](#2)",
        "rustc 1.95.0, cargo-nextest 0.9.146, actions/checkout@v7",
        "a digit run 1234-56-78 and a long one 202611-08-12",
        "an id abc2026-11-12 glued to a word",
        "a Markdown heading:\n# Title\n## Sub",
        "a long dash — then #x and /issues/ alone",
    ] {
        assert_eq!(flagged(line), None, "{line:?} flagged");
    }
}

#[test]
fn one_finding_per_line_at_its_line() {
    let found = check("ok\n#1 and #2 on one line\nok\n2026-01-31\n");
    let lines: Vec<usize> = found.iter().map(|(l, _)| *l).collect();
    assert_eq!(lines, [2, 4]);
}
