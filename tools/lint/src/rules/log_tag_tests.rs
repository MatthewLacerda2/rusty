use super::{check, RULE};

#[test]
fn flags_an_untagged_warning_or_error() {
    let src = "log::warn!(\"font did not load\");\nlog::error!(\"{e}\");\n";
    let found: Vec<_> = check("src/a.rs", src)
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    assert_eq!(found, [(1, RULE), (2, RULE)]);
    let wrapped = "log::warn!(\n    \"surface shader {:?}: {e}\",\n    name\n);\n";
    assert_eq!(check("src/a.rs", wrapped).len(), 1);
    let targeted = "log::warn!(target: \"rusty::x\", \"no tag\");\n";
    assert_eq!(check("src/a.rs", targeted).len(), 1);
}

#[test]
fn passes_tagged_messages_other_levels_and_non_engine_code() {
    let src = "log::warn!(\"[Asset] skipped\");\nlog::error!(\n    \"[Audio] {e}\",\n);\n";
    assert!(check("src/a.rs", src).is_empty());
    let targeted = "log::warn!(target: \"wgpu_hal\", \"[Test] a quirk\");\n";
    assert!(check("src/a.rs", targeted).is_empty());
    assert!(check(
        "src/a.rs",
        "log::info!(\"progress\");\n// log::warn!(\"x\")\n"
    )
    .is_empty());
    assert!(check("tests/a.rs", "log::warn!(\"x\");\n").is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(log-tag) the message is built elsewhere\nlog::warn!(\"{msg}\");\n";
    let found = check("src/a.rs", src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
