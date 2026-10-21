use super::{check, DENY, RULE};

const FBX: &str = "src/asset/convert/fbx/texture/mod.rs";

#[test]
fn flags_a_converter_logging_a_failed_extract() {
    let src = "Err(e) => {\n    log::warn!(\"[Asset] FBX texture: {e}\");\n    None\n}\n";
    let found: Vec<_> = check(FBX, src).iter().map(|f| (f.line, f.rule)).collect();
    assert_eq!(found, [(2, RULE)]);
    assert_eq!(check(FBX, "warn!(\"[Asset] x\");\n").len(), 1);
    assert_eq!(check(FBX, "eprintln!(\"x\");\n").len(), 1);
    // A future converter's folder is a converter's too.
    assert_eq!(
        check("src/asset/convert/blend/run.rs", "log::info!(\"x\");\n").len(),
        1
    );
}

#[test]
fn passes_the_same_skip_returned_and_code_outside_a_converter() {
    let src = "skipped.push(Skip { what, why });\n// log::warn! would hide it\n";
    assert!(check(FBX, src).is_empty());
    assert!(check(FBX, "let dialog::warn = catalog::x;\n").is_empty());
    let warn = "log::warn!(\"[Asset] {skip}\");\n";
    assert!(check("src/asset/convert/mod.rs", warn).is_empty());
    assert!(check("src/asset/convert/stage/mod.rs", warn).is_empty());
    assert!(check("src/asset/convert/tests/skips.rs", warn).is_empty());
    assert!(check("src/asset/mod.rs", warn).is_empty());
}

#[test]
fn a_converter_root_carries_the_deny() {
    let root = "src/asset/convert/glb/mod.rs";
    let found: Vec<_> = check(root, "mod bin;\n").iter().map(|f| f.line).collect();
    assert_eq!(found, [1]);
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
