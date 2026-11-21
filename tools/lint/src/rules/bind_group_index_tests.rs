use super::{check, RULE};

fn lines(path: &str, src: &str) -> Vec<(usize, &'static str)> {
    check(path, src, &[])
        .iter()
        .map(|f| (f.line, f.rule))
        .collect()
}

#[test]
fn flags_a_group_bound_at_a_number() {
    let src = "fn f(p: &mut P) {\n    p.set_bind_group(1, &g, &[]);\n}\n";
    assert_eq!(lines("crates/render/src/a.rs", src), [(2, RULE)]);
    let suffixed = "p.set_bind_group(0u32, &g, &[]);\n";
    assert_eq!(lines("crates/render/src/a.rs", suffixed), [(1, RULE)]);
    let wrapped = "p.set_bind_group(\n    3,\n    &g,\n    &[o],\n);\n";
    assert_eq!(lines("crates/render/src/a.rs", wrapped), [(1, RULE)]);
}

#[test]
fn passes_a_named_group_comments_strings_and_test_code() {
    let named = "p.set_bind_group(groups::DRAW, g, &o);\np.set_bind_group(\n    groups::MATERIAL,\n    m,\n    &[],\n);\n";
    assert!(lines("crates/render/src/a.rs", named).is_empty());
    let own = "const LUT: u32 = 0;\np.set_bind_group(LUT, &g, &[]);\n";
    assert!(lines("crates/render/src/a.rs", own).is_empty());
    let quoted = "// p.set_bind_group(0, g, &[]);\nlet s = \"set_bind_group(0, g)\";\n";
    assert!(lines("crates/render/src/a.rs", quoted).is_empty());
    let call = "p.set_bind_group(0, &g, &[]);\n";
    assert!(check("crates/render/src/a.rs", call, &[true]).is_empty());
    assert!(lines("tests/a.rs", call).is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src =
        "// lint: allow(bind-group-index) a fixture's one group\np.set_bind_group(0, &g, &[]);\n";
    let found = check("crates/render/src/a.rs", src, &[]);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
