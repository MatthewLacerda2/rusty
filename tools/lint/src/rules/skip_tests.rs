use super::{check, RULE};

#[test]
fn flags_ignore_and_cfg_attr_ignore() {
    let src = "#[test]\n#[ignore]\nfn a() {}\n#[cfg_attr(miri, ignore)]\nfn b() {}\n";
    let lines: Vec<_> = check("t.rs", src)
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    assert_eq!(lines, [(2, RULE), (4, RULE)]);
    assert_eq!(check("t.rs", "    #[ignore = \"slow\"]\n").len(), 1);
}

#[test]
fn passes_plain_tests_and_other_attributes() {
    let src = "#[test]\nfn ignore_me() {}\n#[cfg_attr(test, derive(Debug))]\nstruct S;\n";
    assert!(check("t.rs", src).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_the_skip() {
    let src = "#[test]\n// lint: allow(skip) a measurement, run by hand\n#[ignore]\nfn a() {}\n";
    let found = check("t.rs", src);
    assert_eq!(
        found.len(),
        1,
        "the rule reports; `rules::run` applies the allow"
    );
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
