use super::escapes::violations;

const ASSEMBLE: &str = "src/render/setup/mod.rs";

#[test]
fn the_sanctioned_escape_passes_once() {
    let one = "    #[expect(\n        clippy::too_many_arguments,\n        clippy::too_many_lines,\n        reason = \"flat\"\n    )]\n    fn assemble() {}\n";
    assert!(violations(ASSEMBLE, one).is_empty());
    let two = format!("{one}{one}");
    let v = violations(ASSEMBLE, &two);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(
        v[0].starts_with("ESCAPE src/render/setup/mod.rs:7 "),
        "{v:?}"
    );
}

#[test]
fn a_new_length_or_unwrap_escape_is_flagged() {
    let code = "#[expect(clippy::too_many_lines, reason = \"long\")]\nfn f() {}\n\
                #[cfg_attr(test, allow(clippy::unwrap_used, reason = \"x\"))]\nfn g() {}\n";
    let v = violations("src/app/mod.rs", code);
    assert_eq!(v.len(), 2, "{v:?}");
    assert!(v[0].contains("src/app/mod.rs:1 `clippy::too_many_lines`"));
    assert!(v[1].contains("src/app/mod.rs:3 `clippy::unwrap_used`"));
}

#[test]
fn a_deny_a_comment_and_other_lints_are_not_escapes() {
    let code = "#![deny(clippy::unwrap_used)]\n// #[allow(clippy::too_many_lines)]\n\
                #[expect(clippy::too_many_arguments, reason = \"r\")]\nfn f() {}\n";
    assert!(violations("src/app/mod.rs", code).is_empty());
}

#[test]
fn the_engine_tree_has_only_the_sanctioned_escape() {
    let src = include_str!("../../../../src/render/setup/mod.rs");
    assert!(violations(ASSEMBLE, src).is_empty());
}
