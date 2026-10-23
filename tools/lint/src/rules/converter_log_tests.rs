use super::{check, DENY, RULE};

const FBX: &str = "src/asset/convert/fbx/texture/mod.rs";

/// The lines `check` flags in `src` at `path`.
fn lines(path: &str, src: &str) -> Vec<usize> {
    check(path, src).iter().map(|f| f.line).collect()
}

#[test]
fn flags_a_converter_logging_a_failed_extract() {
    let src = "Err(e) => {\n    log::warn!(\"[Asset] FBX texture: {e}\");\n    None\n}\n";
    let found: Vec<_> = check(FBX, src).iter().map(|f| (f.line, f.rule)).collect();
    assert_eq!(found, [(2, RULE)]);
    for call in ["warn!(\"x\");", "log!(Level::Warn, \"x\");"] {
        assert_eq!(lines(FBX, call), [1], "{call}");
    }
    // A future converter's folder is a converter's too.
    assert_eq!(
        lines("src/asset/convert/blend/run.rs", "log::info!(\"x\");"),
        [1]
    );
}

#[test]
fn flags_a_single_file_converter_and_its_missing_deny() {
    let blend = "src/asset/convert/blend.rs";
    assert_eq!(lines(blend, &format!("{DENY}\nlog::warn!(\"x\");\n")), [2]);
    assert_eq!(lines(blend, "fn convert() {}\n"), [1], "the root's deny");
    assert!(check(blend, &format!("{DENY}\nfn convert() {{}}\n")).is_empty());
}

#[test]
fn a_slash_slash_in_a_string_hides_nothing() {
    let src = "let uri = \"file://x\"; log::warn!(\"x\");\n";
    assert_eq!(lines(FBX, src), [1]);
    assert!(check(FBX, "let uri = \"file://x\"; // log::warn!\n").is_empty());
}

#[test]
fn a_deny_missing_a_lint_is_flagged() {
    let root = "src/asset/convert/glb/mod.rs";
    let old = "#![deny(clippy::let_underscore_must_use)]\nmod bin;\n";
    assert_eq!(lines(root, old), [1]);
    let unclosed = "#![deny(\n    clippy::let_underscore_must_use,\nmod bin;\n";
    assert_eq!(lines(root, unclosed), [1]);
}

#[test]
fn a_wrapped_or_commented_deny_passes() {
    let root = "src/asset/convert/glb/mod.rs";
    let wrapped = "#![deny(\n    clippy::let_underscore_must_use,\n    clippy::unused_result_ok,\n    \
                   clippy::print_stdout, // stdout belongs to the shell\n    clippy::print_stderr\n)]\n";
    assert!(check(root, wrapped).is_empty());
    assert!(check(root, &format!("{DENY} // why\nmod bin;\n")).is_empty());
}

#[test]
fn passes_the_same_skip_returned_and_code_outside_a_converter() {
    let src = "skipped.push(Skip { what, why });\n// log::warn! would hide it\n";
    assert!(check(FBX, src).is_empty());
    assert!(check(FBX, "let dialog::warn = catalog::x;\n").is_empty());
    assert!(check(FBX, "catalog!(x);\n").is_empty());
    let warn = "log::warn!(\"[Asset] {skip}\");\n";
    for path in [
        "src/asset/convert/mod.rs",
        "src/asset/convert/lock.rs",
        "src/asset/convert/stage/mod.rs",
        "src/asset/convert/texture_ref.rs",
        "src/asset/convert/tests/skips.rs",
        "src/asset/mod.rs",
    ] {
        assert!(check(path, warn).is_empty(), "{path}");
    }
}

#[test]
fn a_converter_root_carries_the_deny() {
    let root = "src/asset/convert/glb/mod.rs";
    assert_eq!(lines(root, "mod bin;\n"), [1]);
    assert!(check(root, &format!("//! glb\n{DENY}\nmod bin;\n")).is_empty());
    assert!(check("src/asset/convert/glb/bin.rs", "mod x;\n").is_empty());
}

#[test]
fn an_allow_with_a_reason_excuses_it() {
    let src =
        "// lint: allow(converter-log) a debugging aid, removed before merge\nlog::warn!(\"x\");\n";
    let found = check(FBX, src);
    assert!(crate::rules::allowed(src, found[0].line, RULE));
}
