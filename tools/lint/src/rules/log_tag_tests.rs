use super::{check, RULE};

fn flagged(src: &str) -> usize {
    check("src/a.rs", src).len()
}

#[test]
fn flags_an_untagged_warning_or_error() {
    let src = "log::warn!(\"font did not load\");\nlog::error!(\"{e}\");\n";
    let found: Vec<_> = check("src/a.rs", src)
        .iter()
        .map(|f| (f.line, f.rule))
        .collect();
    assert_eq!(found, [(1, RULE), (2, RULE)]);
    let wrapped = "log::warn!(\n    \"surface shader {:?}: {e}\",\n    name\n);\n";
    assert_eq!(flagged(wrapped), 1);
    assert_eq!(
        flagged("log::warn!(target: \"rusty::x\", \"no tag\");\n"),
        1
    );
}

#[test]
fn passes_tagged_messages_other_levels_and_non_engine_code() {
    let src = "log::warn!(\"[Asset] skipped\");\nlog::error!(\n    \"[Audio] {e}\",\n);\n";
    assert_eq!(flagged(src), 0);
    assert_eq!(
        flagged("log::warn!(target: \"wgpu\", \"[Test] a quirk\");\n"),
        0
    );
    assert_eq!(
        flagged("log::info!(\"progress\");\n// log::warn!(\"x\")\n"),
        0
    );
    assert!(check("tests/a.rs", "log::warn!(\"x\");\n").is_empty());
}

#[test]
fn reads_an_imported_bare_macro() {
    assert_eq!(flagged("use log::warn;\nfn f() { warn!(\"x\"); }\n"), 1);
    assert_eq!(flagged("use log::{error, warn};\nerror!(\"{e}\");\n"), 1);
    assert_eq!(flagged("use log::warn;\nfn f() { warn!(\"[A] x\"); }\n"), 0);
    // Without an import from `log`, a bare `warn!` is someone else's macro.
    assert_eq!(flagged("fn f() { warn!(\"x\"); }\n"), 0);
    assert_eq!(flagged("use log::warn;\ntracing::warn!(\"x\");\n"), 0);
}

#[test]
fn reads_log_at_warn_or_error() {
    assert_eq!(flagged("log::log!(Level::Warn, \"x\");\n"), 1);
    assert_eq!(flagged("log::log!(log::Level::Error, \"{e}\");\n"), 1);
    assert_eq!(
        flagged("log::log!(target: \"t\", Level::Warn, \"x\");\n"),
        1
    );
    assert_eq!(flagged("log::log!(Level::Warn, \"[A] x\");\n"), 0);
    assert_eq!(flagged("log::log!(Level::Info, \"x\");\n"), 0);
    assert_eq!(flagged("log::log!(level, \"x\");\n"), 0);
}

#[test]
fn reads_a_space_before_the_parenthesis() {
    assert_eq!(flagged("log::warn! (\"x\");\n"), 1);
    assert_eq!(flagged("log::warn! (\"[A] x\");\n"), 0);
}

#[test]
fn reads_arguments_laid_out_over_several_lines() {
    let tagged = "log::warn!(\n    target: \"rusty::a\",\n    \"[A] {}\",\n    e\n);\n";
    assert_eq!(flagged(tagged), 0);
    let untagged = "log::warn!(\n    target: \"rusty::a\",\n    \"{}\",\n    e\n);\n";
    assert_eq!(flagged(untagged), 1);
    let level = "log::log!(\n    Level::Error,\n    \"[A] {e}\"\n);\n";
    assert_eq!(flagged(level), 0);
}

#[test]
fn reads_a_raw_string_message() {
    assert_eq!(flagged("log::warn!(r\"[A] C:\\path\");\n"), 0);
    assert_eq!(flagged("log::warn!(r#\"[A] \"quoted\"\"#);\n"), 0);
    assert_eq!(flagged("log::warn!(r\"untagged\");\n"), 1);
    assert_eq!(flagged("log::warn!(r#\"untagged\"#);\n"), 1);
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src = "// lint: allow(log-tag) the message is built elsewhere\nlog::warn!(\"{msg}\");\n";
    let found = check("src/a.rs", src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
